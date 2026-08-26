#!/usr/bin/env python3
"""Reconcile the Rust semantic public surface with Aeneas declarations."""

from __future__ import annotations

import argparse
import copy
import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CORE_SRC = ROOT / "crates" / "greatgramma-core" / "src"
LIB_RS = CORE_SRC / "lib.rs"
DEFAULT_TRANSLATION = ROOT / "proofs" / "Generated" / "GreatgrammaCore" / "translation.json"
DEFAULT_INVENTORY = ROOT / "proofs" / "public-api-coverage.json"
ALLOWED_STATUSES = {"unproved", "proved", "excluded"}
REQUIRED_SCOPE_EXCLUSIONS = {"public_fields_and_variants", "trait_implementations"}
AENEAS_FLAG_NOTE = (
    "is_opaque, can_fail, and can_diverge are Aeneas translation properties; "
    "can_fail does not mean that the Rust API returns Result."
)
RAW_STRING_START = re.compile(r'(?:b)?r(#+)?"')
CHAR_LITERAL = re.compile(r"(?:b)?'(?:\\.|[^\\'\n])'")
PUB_USE = re.compile(r"\bpub\s+use\s+([^;]+);", re.DOTALL)
PUB_VISIBILITY = re.compile(r"\bpub\b")
ITEM_MACRO = re.compile(r"\b([A-Za-z_][A-Za-z0-9_:]*)!\s*[({\[]")


class InventoryError(ValueError):
    """Raised when source, metadata, or inventory data is inconsistent."""


def load_json(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as error:
        raise InventoryError(f"missing file: {path}") from error
    except json.JSONDecodeError as error:
        raise InventoryError(f"invalid JSON in {path}: {error}") from error


def require_object(value: Any, description: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise InventoryError(f"{description} must be a JSON object")
    return value


def rust_code_mask(source: str) -> str:
    """Blank comments and literals while preserving byte positions and newlines."""

    result = list(source)
    index = 0
    block_depth = 0
    state = "code"
    raw_hashes = 0
    length = len(source)

    def blank(position: int) -> None:
        if result[position] != "\n":
            result[position] = " "

    while index < length:
        if state == "line_comment":
            blank(index)
            if source[index] == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if source.startswith("/*", index):
                blank(index)
                blank(index + 1)
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                blank(index)
                blank(index + 1)
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                blank(index)
                index += 1
            continue
        if state == "string":
            blank(index)
            if source[index] == "\\" and index + 1 < length:
                blank(index + 1)
                index += 2
            elif source[index] == '"':
                index += 1
                state = "code"
            else:
                index += 1
            continue
        if state == "raw_string":
            closing = '"' + ("#" * raw_hashes)
            if source.startswith(closing, index):
                for position in range(index, index + len(closing)):
                    blank(position)
                index += len(closing)
                state = "code"
            else:
                blank(index)
                index += 1
            continue

        if source.startswith("//", index):
            blank(index)
            blank(index + 1)
            index += 2
            state = "line_comment"
        elif source.startswith("/*", index):
            blank(index)
            blank(index + 1)
            index += 2
            block_depth = 1
            state = "block_comment"
        elif source[index] == '"':
            blank(index)
            index += 1
            state = "string"
        else:
            raw_match = RAW_STRING_START.match(source, index)
            if raw_match:
                token = raw_match.group(0)
                raw_hashes = len(raw_match.group(1) or "")
                for position in range(index, index + len(token)):
                    blank(position)
                index += len(token)
                state = "raw_string"
                continue
            char_match = CHAR_LITERAL.match(source, index)
            if char_match:
                for position in range(index, index + len(char_match.group(0))):
                    blank(position)
                index += len(char_match.group(0))
            else:
                index += 1

    if state in {"block_comment", "string", "raw_string"}:
        raise InventoryError(f"unterminated Rust lexical construct ({state})")
    return "".join(result)


def matching_brace(masked: str, open_index: int) -> int:
    if open_index >= len(masked) or masked[open_index] != "{":
        raise InventoryError("internal parser error: expected opening brace")
    depth = 0
    for index in range(open_index, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    raise InventoryError("unmatched opening brace in Rust source")


def top_level_matches(pattern: re.Pattern[str], masked: str) -> list[re.Match[str]]:
    """Return matches outside every brace-delimited item body."""

    matches: list[re.Match[str]] = []
    depth = 0
    cursor = 0
    for match in pattern.finditer(masked):
        for character in masked[cursor : match.start()]:
            if character == "{":
                depth += 1
            elif character == "}":
                depth -= 1
                if depth < 0:
                    raise InventoryError("unmatched closing brace in Rust source")
        if depth == 0:
            matches.append(match)
        cursor = match.start()
    return matches


def exported_symbols(lib_source: str) -> list[dict[str, str]]:
    masked = rust_code_mask(lib_source)
    statements = top_level_matches(PUB_USE, masked)
    statement_starts = {statement.start() for statement in statements}
    for visibility in top_level_matches(PUB_VISIBILITY, masked):
        suffix = masked[visibility.end() :]
        if suffix.lstrip().startswith("("):
            continue
        if visibility.start() not in statement_starts:
            line = lib_source.count("\n", 0, visibility.start()) + 1
            raise InventoryError(
                f"unsupported direct root public declaration in lib.rs at line {line}; "
                "re-export it with pub use or extend the public API checker"
            )

    exports: list[dict[str, str]] = []
    for statement in statements:
        target = lib_source[statement.start(1) : statement.end(1)].strip()
        grouped = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)\s*::\s*\{(.*)\}", target, re.DOTALL)
        if grouped:
            module = grouped.group(1)
            names = [part.strip() for part in grouped.group(2).split(",") if part.strip()]
        else:
            direct = re.fullmatch(
                r"([A-Za-z_][A-Za-z0-9_]*)\s*::\s*([A-Za-z_][A-Za-z0-9_]*)",
                target,
            )
            if not direct:
                raise InventoryError(f"unsupported pub use syntax in lib.rs: {target!r}")
            module, name = direct.groups()
            names = [name]
        for name in names:
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
                raise InventoryError(f"unsupported re-export name in lib.rs: {name!r}")
            exports.append({"module": module, "name": name})

    keys = [(entry["module"], entry["name"]) for entry in exports]
    if len(keys) != len(set(keys)):
        raise InventoryError("lib.rs contains duplicate public re-exports")
    return sorted(exports, key=lambda entry: (entry["module"], entry["name"]))


def direct_public_methods(masked: str, type_name: str) -> set[str]:
    for invocation in top_level_matches(ITEM_MACRO, masked):
        macro_name = invocation.group(1)
        if macro_name not in {"macro_rules", "define_id"}:
            raise InventoryError(
                f"unsupported top-level item macro {macro_name}! in module containing "
                f"public type {type_name}"
            )

    methods: set[str] = set()
    impl_pattern = re.compile(r"\bimpl\b(?P<header>[^{};]*)\{", re.DOTALL)
    method_pattern = re.compile(
        r"\bpub\s+(?:(?:const|async|unsafe)\s+)*fn\s+([A-Za-z_][A-Za-z0-9_]*)"
    )
    type_in_header = re.compile(rf"\b{re.escape(type_name)}\b")
    trait_for_type = re.compile(rf"\bfor\s+{re.escape(type_name)}\b")
    for impl_match in top_level_matches(impl_pattern, masked):
        header = impl_match.group("header").strip()
        if not type_in_header.search(header) or trait_for_type.search(header):
            continue
        open_index = impl_match.end() - 1
        close_index = matching_brace(masked, open_index)
        body = masked[open_index + 1 : close_index]
        item_macros = top_level_matches(ITEM_MACRO, body)
        if item_macros:
            macro_name = item_macros[0].group(1)
            raise InventoryError(
                f"unsupported item macro {macro_name}! in inherent impl for public type "
                f"{type_name}"
            )
        impl_methods: set[str] = set()
        depth = 0
        for index, character in enumerate(body):
            if character == "{":
                depth += 1
            elif character == "}":
                depth -= 1
            if depth == 0:
                candidate = method_pattern.match(body, index)
                if candidate:
                    impl_methods.add(candidate.group(1))
        if header != type_name and impl_methods:
            raise InventoryError(
                f"unsupported inherent impl syntax for public type {type_name}: "
                f"impl {header}"
            )
        methods.update(impl_methods)

    methods.update(macro_public_methods(masked, type_name))
    return methods


def macro_public_methods(masked_source: str, type_name: str) -> set[str]:
    """Expand the repository's define_id! public-method surface."""

    invocation = re.search(rf"\bdefine_id!\s*\(\s*{re.escape(type_name)}\s*,", masked_source)
    if not invocation:
        return set()
    macro = re.search(r"\bmacro_rules!\s+define_id\s*\{", masked_source)
    if not macro:
        raise InventoryError(f"{type_name} uses define_id! but its macro definition is missing")
    open_index = masked_source.find("{", macro.start(), macro.end())
    close_index = matching_brace(masked_source, open_index)
    body = masked_source[open_index + 1 : close_index]
    return set(
        re.findall(
            r"\bpub\s+(?:(?:const|async|unsafe)\s+)*fn\s+([A-Za-z_][A-Za-z0-9_]*)",
            body,
        )
    )


def translation_indexes(metadata: Any) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    document = require_object(metadata, "translation metadata")
    type_index: dict[str, dict[str, Any]] = {}
    function_index: dict[str, dict[str, Any]] = {}
    for collection_name, destination in (("types", type_index), ("functions", function_index)):
        collection = document.get(collection_name)
        if not isinstance(collection, list):
            raise InventoryError(f"translation metadata field {collection_name!r} must be an array")
        for index, raw_entry in enumerate(collection):
            entry = require_object(raw_entry, f"{collection_name}[{index}]")
            if not entry.get("is_local"):
                continue
            rust_name = entry.get("rust_name")
            if not isinstance(rust_name, str) or not rust_name:
                raise InventoryError(f"{collection_name}[{index}].rust_name must be non-empty")
            if rust_name in destination:
                # Loop encodings deliberately share a Rust name with their parent.
                if collection_name == "functions" and entry.get("is_rec"):
                    continue
                if collection_name == "functions" and destination[rust_name].get("is_rec"):
                    destination[rust_name] = entry
                    continue
                raise InventoryError(f"ambiguous local translation for {rust_name}")
            destination[rust_name] = entry
    return type_index, function_index


def translated_api(entry: dict[str, Any], kind: str) -> dict[str, Any]:
    lean_name = entry.get("lean_name")
    if not isinstance(lean_name, str) or not lean_name:
        raise InventoryError(f"translated {kind} has no Lean declaration name")
    result = {"lean": lean_name}
    if kind == "function":
        for field in ("is_opaque", "can_fail", "can_diverge"):
            if not isinstance(entry.get(field), bool):
                raise InventoryError(
                    f"translated function flag {field} is not boolean: {entry.get('rust_name')}"
                )
        result.update(
            {
                "is_opaque": entry.get("is_opaque"),
                "can_fail": entry.get("can_fail"),
                "can_diverge": entry.get("can_diverge"),
            }
        )
    return result


def expected_items(metadata: Any, lib_source: str) -> list[dict[str, Any]]:
    document = require_object(metadata, "translation metadata")
    crate = document.get("crate")
    if crate != "greatgramma_core":
        raise InventoryError(f"unexpected translated crate: {crate!r}")
    types, functions = translation_indexes(document)
    items: list[dict[str, Any]] = []
    module_masks: dict[str, str] = {}

    for export in exported_symbols(lib_source):
        module = export["module"]
        name = export["name"]
        source_path = f"{crate}::{module}::{name}"
        public_path = f"{crate}::{name}"
        type_translation = types.get(source_path)
        function_translation = functions.get(source_path)
        if (type_translation is None) == (function_translation is None):
            raise InventoryError(
                f"public re-export must have exactly one local type/function translation: {source_path}"
            )

        if type_translation is not None:
            if module not in module_masks:
                module_path = CORE_SRC / f"{module}.rs"
                try:
                    module_source = module_path.read_text(encoding="utf-8")
                except FileNotFoundError as error:
                    raise InventoryError(
                        f"missing module source for public export: {module_path}"
                    ) from error
                module_masks[module] = rust_code_mask(module_source)
            items.append(
                {
                    "rust_api": public_path,
                    "kind": "type",
                    "source": source_path,
                    **translated_api(type_translation, "type"),
                }
            )
            for method in sorted(direct_public_methods(module_masks[module], name)):
                method_source = f"{crate}::{module}::{{{source_path}}}::{method}"
                method_translation = functions.get(method_source)
                if method_translation is None:
                    raise InventoryError(
                        f"public method has no unique local translation: {public_path}::{method}"
                    )
                items.append(
                    {
                        "rust_api": f"{public_path}::{method}",
                        "kind": "method",
                        "source": method_source,
                        **translated_api(method_translation, "function"),
                    }
                )
        else:
            items.append(
                {
                    "rust_api": public_path,
                    "kind": "function",
                    "source": source_path,
                    **translated_api(function_translation, "function"),
                }
            )

    keys = [item["rust_api"] for item in items]
    if len(keys) != len(set(keys)):
        raise InventoryError("extracted Rust public API contains duplicate paths")
    return sorted(items, key=lambda item: item["rust_api"])


def default_scope_exclusions() -> list[dict[str, str]]:
    return [
        {
            "scope": "public_fields_and_variants",
            "justification": (
                "Public fields and enum variants are represented by their enclosing translated "
                "type entry; they are not separate callable proof boundaries."
            ),
        },
        {
            "scope": "trait_implementations",
            "justification": (
                "Derived and standard-trait operations are classified in proof-coverage.json; "
                "U1 does not duplicate them as independent semantic call boundaries."
            ),
        },
    ]


def make_inventory(metadata: Any, lib_source: str, existing: Any | None = None) -> dict[str, Any]:
    document = require_object(metadata, "translation metadata")
    prior_by_api: dict[str, dict[str, Any]] = {}
    if isinstance(existing, dict) and isinstance(existing.get("apis"), dict):
        prior_by_api = existing["apis"]

    apis: dict[str, dict[str, Any]] = {}
    for expected in expected_items(document, lib_source):
        prior = prior_by_api.get(expected["rust_api"], {})
        item = {key: value for key, value in expected.items() if key != "rust_api"}
        status = prior.get("status") if isinstance(prior, dict) else None
        item["status"] = status if status in ALLOWED_STATUSES else "unproved"
        if item["status"] == "proved" and isinstance(prior.get("theorem"), str):
            item["theorem"] = prior["theorem"]
        if item["status"] == "excluded" and isinstance(prior.get("justification"), str):
            item["justification"] = prior["justification"]
        if item.get("is_opaque") is True and isinstance(prior.get("opacity_justification"), str):
            item["opacity_justification"] = prior["opacity_justification"]
        apis[expected["rust_api"]] = item

    return {
        "schema_version": 1,
        "translation": {
            "crate": document.get("crate"),
            "aeneas_version": document.get("aeneas_version"),
            "charon_version": document.get("charon_version"),
        },
        "flag_semantics": AENEAS_FLAG_NOTE,
        "scope_exclusions": default_scope_exclusions(),
        "apis": apis,
    }


def validate_coverage_classification(item: dict[str, Any], errors: list[str]) -> None:
    rust_api = item.get("rust_api", "<unknown>")
    is_opaque = item.get("kind") in {"function", "method"} and item.get("is_opaque") is True
    opacity_justification = item.get("opacity_justification")
    if is_opaque:
        if not isinstance(opacity_justification, str) or len(opacity_justification.strip()) < 16:
            errors.append(f"opaque public API needs a specific opacity_justification: {rust_api}")
    elif opacity_justification is not None:
        errors.append(f"non-opaque public API has opacity_justification: {rust_api}")

    status = item.get("status")
    theorem = item.get("theorem")
    justification = item.get("justification")
    if status not in ALLOWED_STATUSES:
        errors.append(f"invalid coverage status for {rust_api}: {status!r}")
    elif status == "unproved":
        if "theorem" in item or "justification" in item:
            errors.append(f"unproved API must omit theorem and justification: {rust_api}")
    elif status == "proved":
        if not isinstance(theorem, str) or not theorem.startswith("Greatgramma."):
            errors.append(f"proved API needs a stable Greatgramma theorem: {rust_api}")
        if "justification" in item:
            errors.append(f"proved API must omit justification: {rust_api}")
    elif status == "excluded":
        if "theorem" in item:
            errors.append(f"excluded API must omit theorem: {rust_api}")
        if not isinstance(justification, str) or len(justification.strip()) < 16:
            errors.append(f"excluded API needs a specific justification: {rust_api}")


def validate_inventory(metadata: Any, lib_source: str, inventory: Any) -> list[str]:
    errors: list[str] = []
    try:
        document = require_object(metadata, "translation metadata")
        expected = {
            item["rust_api"]: {key: value for key, value in item.items() if key != "rust_api"}
            for item in expected_items(document, lib_source)
        }
        coverage_document = require_object(inventory, "public API coverage inventory")
    except InventoryError as error:
        return [str(error)]

    if coverage_document.get("schema_version") != 1:
        errors.append("public API coverage schema_version must be 1")
    expected_translation = {
        "crate": document.get("crate"),
        "aeneas_version": document.get("aeneas_version"),
        "charon_version": document.get("charon_version"),
    }
    if coverage_document.get("translation") != expected_translation:
        errors.append("translation pin summary does not match translation.json")
    if coverage_document.get("flag_semantics") != AENEAS_FLAG_NOTE:
        errors.append("flag_semantics does not describe the Aeneas effect flags")

    exclusions = coverage_document.get("scope_exclusions")
    seen_scopes: set[str] = set()
    if not isinstance(exclusions, list):
        errors.append("scope_exclusions must be an array")
    else:
        for index, exclusion in enumerate(exclusions):
            if not isinstance(exclusion, dict):
                errors.append(f"scope_exclusions[{index}] must be an object")
                continue
            scope = exclusion.get("scope")
            justification = exclusion.get("justification")
            if not isinstance(scope, str) or not scope:
                errors.append(f"scope_exclusions[{index}].scope must be non-empty")
                continue
            if scope in seen_scopes:
                errors.append(f"duplicate public API scope exclusion: {scope}")
            seen_scopes.add(scope)
            if not isinstance(justification, str) or len(justification.strip()) < 32:
                errors.append(f"scope exclusion needs a specific justification: {scope}")
        missing_scopes = REQUIRED_SCOPE_EXCLUSIONS - seen_scopes
        extra_scopes = seen_scopes - REQUIRED_SCOPE_EXCLUSIONS
        for scope in sorted(missing_scopes):
            errors.append(f"missing required public API scope exclusion: {scope}")
        for scope in sorted(extra_scopes):
            errors.append(f"unexpected public API scope exclusion: {scope}")

    raw_apis = coverage_document.get("apis")
    if not isinstance(raw_apis, dict):
        return errors + ["public API coverage field 'apis' must be an object"]
    actual = raw_apis
    for rust_api, item in actual.items():
        if not isinstance(rust_api, str) or not rust_api:
            errors.append("public API keys must be non-empty Rust paths")
        if not isinstance(item, dict):
            errors.append(f"public API classification must be an object: {rust_api}")

    missing = sorted(expected.keys() - actual.keys())
    extra = sorted(actual.keys() - expected.keys())
    for rust_api in missing:
        errors.append(f"unclassified Rust public API: {rust_api}")
    for rust_api in extra:
        errors.append(f"coverage entry has no Rust public API: {rust_api}")

    for rust_api in sorted(expected.keys() & actual.keys()):
        expected_item = expected[rust_api]
        actual_item = actual[rust_api]
        if not isinstance(actual_item, dict):
            continue
        for field in expected_item:
            if actual_item.get(field) != expected_item[field]:
                errors.append(f"metadata mismatch for {rust_api}: {field}")
        allowed_fields = set(expected_item) | {
            "status",
            "theorem",
            "justification",
            "opacity_justification",
        }
        for field in sorted(actual_item.keys() - allowed_fields):
            errors.append(f"unexpected classification field for {rust_api}: {field}")
        validate_coverage_classification({"rust_api": rust_api, **actual_item}, errors)
    return errors


def run_self_test(metadata: Any, lib_source: str, inventory: Any) -> None:
    baseline_errors = validate_inventory(metadata, lib_source, inventory)
    if baseline_errors:
        raise InventoryError("self-test needs a valid baseline: " + baseline_errors[0])

    cases: list[tuple[str, dict[str, Any], str]] = []
    missing = copy.deepcopy(inventory)
    removed_api = next(iter(missing["apis"]))
    del missing["apis"][removed_api]
    cases.append(("missing public item", missing, f"unclassified Rust public API: {removed_api}"))

    bad_translation = copy.deepcopy(inventory)
    first_api = next(iter(bad_translation["apis"]))
    bad_translation["apis"][first_api]["lean"] += ".missing"
    cases.append(("wrong Lean mapping", bad_translation, "metadata mismatch"))

    extra = copy.deepcopy(inventory)
    extra["apis"]["greatgramma_core::Unexpected"] = copy.deepcopy(
        extra["apis"][first_api]
    )
    cases.append(("extra public item", extra, "has no Rust public API"))

    missing_exclusion = copy.deepcopy(inventory)
    missing_exclusion["scope_exclusions"].pop()
    cases.append(("missing scope exclusion", missing_exclusion, "missing required"))

    for name, candidate, expected_fragment in cases:
        errors = validate_inventory(metadata, lib_source, candidate)
        if not any(expected_fragment in error for error in errors):
            raise InventoryError(
                f"negative self-test '{name}' did not produce {expected_fragment!r}: {errors}"
            )

    direct_root_source = lib_source + "\npub fn direct_root_probe() {}\n"
    direct_root_errors = validate_inventory(metadata, direct_root_source, inventory)
    if not any("unsupported direct root public declaration" in error for error in direct_root_errors):
        raise InventoryError(
            "negative self-test 'direct root export' did not fail: "
            f"{direct_root_errors}"
        )

    macro_fixtures = (
        "impl Probe { generate_methods!(); }",
        "generate_impl!(Probe);",
    )
    for macro_fixture in macro_fixtures:
        try:
            direct_public_methods(rust_code_mask(macro_fixture), "Probe")
        except InventoryError as error:
            if "unsupported" not in str(error) or "macro" not in str(error):
                raise InventoryError(
                    "negative self-test 'item macro' failed for the wrong reason: "
                    f"{error}"
                ) from error
        else:
            raise InventoryError(
                "negative self-test 'item macro' accepted a macro-generated public surface"
            )

    impl_fixtures = (
        "impl<T> Probe<T> { pub fn exposed() {} }",
        "impl Probe where Probe: Sized { pub fn exposed() {} }",
    )
    for impl_fixture in impl_fixtures:
        try:
            direct_public_methods(rust_code_mask(impl_fixture), "Probe")
        except InventoryError as error:
            if "unsupported inherent impl syntax" not in str(error):
                raise InventoryError(
                    "negative self-test 'inherent impl syntax' failed for the wrong reason: "
                    f"{error}"
                ) from error
        else:
            raise InventoryError(
                "negative self-test 'inherent impl syntax' accepted an untracked public method"
            )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--translation", type=Path, default=DEFAULT_TRANSLATION)
    parser.add_argument("--inventory", type=Path, default=DEFAULT_INVENTORY)
    parser.add_argument("--lib", type=Path, default=LIB_RS)
    parser.add_argument(
        "--write",
        action="store_true",
        help="write a deterministic inventory, preserving existing proof classifications",
    )
    parser.add_argument("--self-test", action="store_true", help="run negative fixtures too")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        metadata = load_json(args.translation)
        try:
            lib_source = args.lib.read_text(encoding="utf-8")
        except FileNotFoundError as error:
            raise InventoryError(f"missing Rust public root: {args.lib}") from error

        if args.write:
            existing = load_json(args.inventory) if args.inventory.exists() else None
            inventory = make_inventory(metadata, lib_source, existing)
            args.inventory.parent.mkdir(parents=True, exist_ok=True)
            args.inventory.write_text(
                json.dumps(inventory, indent=2, sort_keys=False) + "\n", encoding="utf-8"
            )
        else:
            inventory = load_json(args.inventory)

        errors = validate_inventory(metadata, lib_source, inventory)
        if errors:
            for error in errors:
                print(f"public API coverage error: {error}", file=sys.stderr)
            return 1
        if args.self_test:
            run_self_test(metadata, lib_source, inventory)

        items = inventory["apis"]
        counts = {
            kind: sum(item["kind"] == kind for item in items.values())
            for kind in ("type", "function", "method")
        }
        print(
            "public API coverage OK: "
            f"{len(items)} items ({counts['type']} types, {counts['function']} functions, "
            f"{counts['method']} methods)"
        )
        return 0
    except InventoryError as error:
        print(f"public API coverage error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
