#!/usr/bin/env python3
"""Reconcile Aeneas translation metadata with the proof-obligation inventory."""

from __future__ import annotations

import argparse
import copy
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TRANSLATION = (
    ROOT / "proofs" / "Generated" / "GreatgrammaCore" / "translation.json"
)
DEFAULT_INVENTORY = ROOT / "proofs" / "proof-coverage.json"
REQUIRED_FUNCTION_FIELDS = {
    "def_id",
    "lean_name",
    "lean_file",
    "rust_name",
    "is_local",
    "is_opaque",
    "can_fail",
    "can_diverge",
    "is_rec",
    "reducible",
}
ALLOWED_STATUSES = {"unproved", "proved", "excluded"}
TRANSLATION_COLLECTIONS = (
    "functions",
    "types",
    "globals",
    "trait_decls",
    "trait_impls",
)
AENEAS_FLAG_NOTE = (
    "is_opaque, can_fail, and can_diverge are Aeneas translation properties; "
    "can_fail does not mean that the Rust API returns Result."
)


class InventoryError(ValueError):
    """Raised when an input cannot be treated as a coverage inventory."""


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


def validate_translation_files(metadata: Any, translation_dir: Path) -> list[str]:
    """Check that every metadata declaration names one checked-in generated file."""

    try:
        document = require_object(metadata, "translation metadata")
    except InventoryError as error:
        return [str(error)]

    errors: list[str] = []
    seen_lean_names: set[str] = set()
    for collection_name in TRANSLATION_COLLECTIONS:
        collection = document.get(collection_name)
        if not isinstance(collection, list):
            errors.append(
                f"translation metadata field {collection_name!r} must be an array"
            )
            continue
        for index, raw_entry in enumerate(collection):
            if not isinstance(raw_entry, dict):
                errors.append(f"{collection_name}[{index}] must be an object")
                continue
            lean_name = raw_entry.get("lean_name")
            if not isinstance(lean_name, str) or not lean_name:
                errors.append(f"{collection_name}[{index}].lean_name must be non-empty")
            elif lean_name in seen_lean_names:
                errors.append(f"duplicate Lean declaration in metadata: {lean_name}")
            else:
                seen_lean_names.add(lean_name)

            lean_file = raw_entry.get("lean_file")
            if not isinstance(lean_file, str) or not lean_file:
                errors.append(f"{collection_name}[{index}].lean_file must be non-empty")
                continue
            relative_path = Path(lean_file)
            if relative_path.is_absolute() or ".." in relative_path.parts:
                errors.append(
                    f"{collection_name}[{index}].lean_file escapes the generated root: "
                    f"{lean_file}"
                )
                continue
            if not (translation_dir / relative_path).is_file():
                errors.append(
                    f"{collection_name}[{index}] references missing generated file: "
                    f"{lean_file}"
                )
    return errors


def local_functions(metadata: Any) -> list[dict[str, Any]]:
    document = require_object(metadata, "translation metadata")
    functions = document.get("functions")
    if not isinstance(functions, list):
        raise InventoryError("translation metadata field 'functions' must be an array")

    result: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    for index, raw_function in enumerate(functions):
        function = require_object(raw_function, f"functions[{index}]")
        missing = REQUIRED_FUNCTION_FIELDS - function.keys()
        if missing:
            raise InventoryError(
                f"functions[{index}] lacks required fields: {', '.join(sorted(missing))}"
            )
        if not isinstance(function["is_local"], bool):
            raise InventoryError(f"functions[{index}].is_local must be boolean")
        if not function["is_local"]:
            continue
        lean_name = function["lean_name"]
        if not isinstance(lean_name, str) or not lean_name:
            raise InventoryError(f"functions[{index}].lean_name must be non-empty")
        if not isinstance(function["rust_name"], str) or not function["rust_name"]:
            raise InventoryError(f"functions[{index}].rust_name must be non-empty")
        if not isinstance(function["lean_file"], str) or not function["lean_file"]:
            raise InventoryError(f"functions[{index}].lean_file must be non-empty")
        if isinstance(function["def_id"], bool) or not isinstance(function["def_id"], int):
            raise InventoryError(f"functions[{index}].def_id must be an integer")
        for field in ("is_opaque", "can_fail", "can_diverge", "is_rec", "reducible"):
            if not isinstance(function[field], bool):
                raise InventoryError(f"functions[{index}].{field} must be boolean")
        if lean_name in seen_names:
            raise InventoryError(f"duplicate local Lean declaration in metadata: {lean_name}")
        seen_names.add(lean_name)
        result.append(function)
    return sorted(result, key=lambda function: function["lean_name"])


def expected_entries(metadata: Any) -> list[dict[str, Any]]:
    functions = local_functions(metadata)
    by_def_id: dict[int, list[dict[str, Any]]] = defaultdict(list)
    for function in functions:
        by_def_id[function["def_id"]].append(function)

    parent_for_loop: dict[str, str] = {}
    children_for_parent: dict[str, list[str]] = defaultdict(list)
    for def_id, siblings in by_def_id.items():
        loops = [function for function in siblings if function["is_rec"]]
        if not loops:
            continue
        parents = [function for function in siblings if not function["is_rec"]]
        if len(parents) != 1:
            names = ", ".join(function["lean_name"] for function in siblings)
            raise InventoryError(
                f"loop def_id {def_id} needs exactly one non-recursive parent; found: {names}"
            )
        parent_name = parents[0]["lean_name"]
        for loop in loops:
            loop_name = loop["lean_name"]
            parent_for_loop[loop_name] = parent_name
            children_for_parent[parent_name].append(loop_name)

    entries: list[dict[str, Any]] = []
    for function in functions:
        lean_name = function["lean_name"]
        entry = {
            "lean_name": lean_name,
            "kind": "loop" if function["is_rec"] else "function",
            "is_opaque": function["is_opaque"],
            "can_fail": function["can_fail"],
            "can_diverge": function["can_diverge"],
        }
        if lean_name in parent_for_loop:
            entry["parent"] = parent_for_loop[lean_name]
        if lean_name in children_for_parent:
            entry["loops"] = sorted(children_for_parent[lean_name])
        entries.append(entry)
    return entries


def make_inventory(metadata: Any, existing: Any | None = None) -> dict[str, Any]:
    document = require_object(metadata, "translation metadata")
    prior_by_name: dict[str, dict[str, Any]] = {}
    if isinstance(existing, dict) and isinstance(existing.get("declarations"), dict):
        prior_by_name = existing["declarations"]

    declarations: dict[str, dict[str, Any]] = {}
    for expected in expected_entries(document):
        prior = prior_by_name.get(expected["lean_name"], {})
        entry = {key: value for key, value in expected.items() if key != "lean_name"}
        status = prior.get("status") if isinstance(prior, dict) else None
        entry["status"] = status if status in ALLOWED_STATUSES else "unproved"
        if entry["status"] == "proved" and isinstance(prior.get("theorem"), str):
            entry["theorem"] = prior["theorem"]
        if entry["status"] == "excluded" and isinstance(prior.get("justification"), str):
            entry["justification"] = prior["justification"]
        if entry["is_opaque"] and isinstance(prior.get("opacity_justification"), str):
            entry["opacity_justification"] = prior["opacity_justification"]
        declarations[expected["lean_name"]] = entry

    return {
        "schema_version": 1,
        "translation": {
            "crate": document.get("crate"),
            "aeneas_version": document.get("aeneas_version"),
            "charon_version": document.get("charon_version"),
        },
        "flag_semantics": AENEAS_FLAG_NOTE,
        "declarations": declarations,
    }


def validate_inventory(metadata: Any, inventory: Any) -> list[str]:
    errors: list[str] = []
    try:
        document = require_object(metadata, "translation metadata")
        expected = {
            entry["lean_name"]: {key: value for key, value in entry.items() if key != "lean_name"}
            for entry in expected_entries(document)
        }
        coverage_document = require_object(inventory, "proof coverage inventory")
    except InventoryError as error:
        return [str(error)]

    if coverage_document.get("schema_version") != 1:
        errors.append("proof coverage schema_version must be 1")

    expected_translation = {
        "crate": document.get("crate"),
        "aeneas_version": document.get("aeneas_version"),
        "charon_version": document.get("charon_version"),
    }
    if coverage_document.get("translation") != expected_translation:
        errors.append("translation pin summary does not match translation.json")
    if coverage_document.get("flag_semantics") != AENEAS_FLAG_NOTE:
        errors.append("flag_semantics does not describe the Aeneas effect flags")

    raw_declarations = coverage_document.get("declarations")
    if not isinstance(raw_declarations, dict):
        return errors + ["proof coverage field 'declarations' must be an object"]
    actual = raw_declarations
    for lean_name, entry in actual.items():
        if not isinstance(lean_name, str) or not lean_name:
            errors.append("declaration keys must be non-empty Lean names")
        if not isinstance(entry, dict):
            errors.append(f"declaration classification must be an object: {lean_name}")

    missing = sorted(expected.keys() - actual.keys())
    extra = sorted(actual.keys() - expected.keys())
    for lean_name in missing:
        errors.append(f"unclassified local declaration: {lean_name}")
    for lean_name in extra:
        errors.append(f"coverage entry has no local translation: {lean_name}")

    for lean_name in sorted(expected.keys() & actual.keys()):
        expected_entry = expected[lean_name]
        actual_entry = actual[lean_name]
        if not isinstance(actual_entry, dict):
            continue
        for field in expected_entry:
            if actual_entry.get(field) != expected_entry[field]:
                errors.append(f"metadata mismatch for {lean_name}: {field}")

        allowed_fields = set(expected_entry) | {
            "status",
            "theorem",
            "justification",
            "opacity_justification",
        }
        for field in sorted(actual_entry.keys() - allowed_fields):
            errors.append(f"unexpected classification field for {lean_name}: {field}")

        is_opaque = expected_entry["is_opaque"]
        opacity_justification = actual_entry.get("opacity_justification")
        if is_opaque:
            if not isinstance(opacity_justification, str) or len(opacity_justification.strip()) < 16:
                errors.append(
                    f"opaque local declaration needs a specific opacity_justification: {lean_name}"
                )
        elif opacity_justification is not None:
            errors.append(f"non-opaque declaration has opacity_justification: {lean_name}")

        status = actual_entry.get("status")
        theorem = actual_entry.get("theorem")
        justification = actual_entry.get("justification")
        if status not in ALLOWED_STATUSES:
            errors.append(f"invalid coverage status for {lean_name}: {status!r}")
        elif status == "unproved":
            if "theorem" in actual_entry or "justification" in actual_entry:
                errors.append(
                    f"unproved entry must omit theorem and justification: {lean_name}"
                )
        elif status == "proved":
            if not isinstance(theorem, str) or not theorem.startswith("Greatgramma."):
                errors.append(f"proved entry needs a stable Greatgramma theorem: {lean_name}")
            if "justification" in actual_entry:
                errors.append(f"proved entry must omit justification: {lean_name}")
        elif status == "excluded":
            if "theorem" in actual_entry:
                errors.append(f"excluded entry must omit theorem: {lean_name}")
            if not isinstance(justification, str) or len(justification.strip()) < 16:
                errors.append(f"excluded entry needs a specific justification: {lean_name}")

    return errors


def run_self_test(metadata: Any, inventory: Any, translation_dir: Path) -> None:
    baseline_errors = validate_inventory(metadata, inventory)
    if baseline_errors:
        raise InventoryError("self-test needs a valid baseline: " + baseline_errors[0])

    loop_name = next(
        name
        for name, entry in inventory["declarations"].items()
        if entry["kind"] == "loop"
    )
    cases: list[tuple[str, dict[str, Any], Any, str]] = []

    missing_loop = copy.deepcopy(inventory)
    del missing_loop["declarations"][loop_name]
    cases.append(("missing loop", missing_loop, metadata, f"unclassified local declaration: {loop_name}"))

    bad_parent = copy.deepcopy(inventory)
    bad_parent["declarations"][loop_name]["parent"] = "GreatgrammaCore.missing"
    cases.append(("bad loop parent", bad_parent, metadata, "metadata mismatch"))

    extra = copy.deepcopy(inventory)
    first_name = next(iter(extra["declarations"]))
    extra["declarations"]["GreatgrammaCore.unexpected"] = copy.deepcopy(
        extra["declarations"][first_name]
    )
    cases.append(("extra entry", extra, metadata, "has no local translation"))

    opaque_metadata = copy.deepcopy(metadata)
    opaque_name = next(iter(inventory["declarations"]))
    for function in opaque_metadata["functions"]:
        if function.get("lean_name") == opaque_name:
            function["is_opaque"] = True
            break
    cases.append(("unexpected opacity", inventory, opaque_metadata, "metadata mismatch"))

    invalid_status = copy.deepcopy(inventory)
    invalid_status["declarations"][opaque_name]["status"] = "documented"
    cases.append(("invalid status", invalid_status, metadata, "invalid coverage status"))

    for name, candidate, candidate_metadata, expected_fragment in cases:
        errors = validate_inventory(candidate_metadata, candidate)
        if not any(expected_fragment in error for error in errors):
            raise InventoryError(
                f"negative self-test '{name}' did not produce {expected_fragment!r}: {errors}"
            )

    missing_file_metadata = copy.deepcopy(metadata)
    missing_file_metadata["functions"][0]["lean_file"] = "MissingGeneratedModule.lean"
    missing_file_errors = validate_translation_files(
        missing_file_metadata, translation_dir
    )
    if not any("references missing generated file" in error for error in missing_file_errors):
        raise InventoryError(
            "negative self-test 'missing generated module' did not fail: "
            f"{missing_file_errors}"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--translation", type=Path, default=DEFAULT_TRANSLATION)
    parser.add_argument("--inventory", type=Path, default=DEFAULT_INVENTORY)
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
        metadata_file_errors = validate_translation_files(metadata, args.translation.parent)
        if metadata_file_errors:
            for error in metadata_file_errors:
                print(f"proof coverage error: {error}", file=sys.stderr)
            return 1
        if args.write:
            existing = load_json(args.inventory) if args.inventory.exists() else None
            inventory = make_inventory(metadata, existing)
            args.inventory.parent.mkdir(parents=True, exist_ok=True)
            args.inventory.write_text(
                json.dumps(inventory, indent=2, sort_keys=False) + "\n", encoding="utf-8"
            )
        else:
            inventory = load_json(args.inventory)

        errors = validate_inventory(metadata, inventory)
        if errors:
            for error in errors:
                print(f"proof coverage error: {error}", file=sys.stderr)
            return 1
        if args.self_test:
            run_self_test(metadata, inventory, args.translation.parent)

        entries = inventory["declarations"]
        loops = sum(entry["kind"] == "loop" for entry in entries.values())
        opaque = sum(entry["is_opaque"] for entry in entries.values())
        print(
            "proof coverage OK: "
            f"{len(entries)} local declarations ({loops} loops, {opaque} opaque)"
        )
        return 0
    except InventoryError as error:
        print(f"proof coverage error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
