#!/usr/bin/env python3
"""Validate source-only Lean Blueprint coverage without third-party modules."""

from __future__ import annotations

import argparse
from collections import Counter
from pathlib import Path
import re
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = REPO_ROOT / "blueprint" / "src" / "content.tex"

NODE_KINDS = (
    "definition",
    "lemma",
    "proposition",
    "theorem",
    "corollary",
    "assumption",
)
NODE_RE = re.compile(
    r"\\begin\{(?P<kind>" + "|".join(NODE_KINDS) + r")\}"
    r"(?:\s*\[[^\]]*\])?"
    r"(?P<body>.*?)"
    r"\\end\{(?P=kind)\}",
    re.DOTALL,
)
LABEL_RE = re.compile(r"\\label\s*\{([^{}]+)\}")
USES_RE = re.compile(r"\\uses\s*\{([^{}]*)\}", re.DOTALL)
STATUS_RE = re.compile(r"\\bpstatus\s*\{([^{}]+)\}")
LEAN_RE = re.compile(r"\\lean\s*\{([^{}]*)\}", re.DOTALL)
INCLUDE_RE = re.compile(r"\\(?:input|include)\s*\{([^{}]+)\}")
VALID_LABEL_RE = re.compile(r"^[a-z][a-z0-9_-]*:[a-z][a-z0-9_-]*$")

VALID_STATES = {"planned", "linked", "proved", "assumption"}
STABLE_LEAN_PREFIXES = (
    "Greatgramma.Spec.",
    "Greatgramma.Invariants.",
    "Greatgramma.Refinement.",
    "Greatgramma.Theorems.",
)

REQUIRED_LABELS = {
    # U1--U10 roadmap coverage.
    "def:generated-proof-boundary",
    "prop:feasibility-gate",
    "thm:validation-boundary",
    "thm:lexer-token-refinement",
    "thm:spanner-refinement",
    "thm:lalr-preprocess-refinement",
    "thm:core-refines-finite-head",
    "thm:batch-runtime-correct",
    "thm:finite-head-language-correct",
    "prop:verification-claim-gate",
    # Major specification and language seams.
    "def:finite-head-checker",
    "prop:realizability-adapter",
    "thm:intended-grammar-corollary",
    # Three public roots.
    "thm:core-language-correct",
    # AS1--AS9.
    "asm:token-bytes-correct",
    "asm:lexer-dfa-correct",
    "asm:lalr-operational-wf",
    "asm:reduction-rank-correct",
    "asm:lalr-language-correct",
    "asm:continuation-realizable",
    "asm:parser-productive",
    "asm:allocator-contract",
    "asm:trusted-computing-base",
}


def strip_comments(source: str) -> str:
    """Remove unescaped TeX comments while preserving line boundaries."""

    stripped: list[str] = []
    for line in source.splitlines(keepends=True):
        comment_at: int | None = None
        for index, char in enumerate(line):
            if char != "%":
                continue
            backslashes = 0
            cursor = index - 1
            while cursor >= 0 and line[cursor] == "\\":
                backslashes += 1
                cursor -= 1
            if backslashes % 2 == 0:
                comment_at = index
                break
        if comment_at is None:
            stripped.append(line)
        elif line.endswith("\n"):
            stripped.append(line[:comment_at] + "\n")
        else:
            stripped.append(line[:comment_at])
    return "".join(stripped)


def command_count(body: str, command: str) -> int:
    return len(re.findall(r"\\" + re.escape(command) + r"(?![A-Za-z@])", body))


def split_csv(value: str) -> list[str]:
    return [part.strip() for part in value.split(",") if part.strip()]


def validate_source(source: str, *, require_roadmap: bool = True) -> list[str]:
    source = strip_comments(source)
    errors: list[str] = []

    for included_path in INCLUDE_RE.findall(source):
        errors.append(
            "Blueprint roadmap content must be self-contained; "
            f"nested input/include is unsupported: {included_path.strip()!r}"
        )

    nodes = list(NODE_RE.finditer(source))
    node_labels = [
        value.strip()
        for node in nodes
        for value in LABEL_RE.findall(node.group("body"))
    ]
    node_label_counts = Counter(node_labels)

    all_labels = [value.strip() for value in LABEL_RE.findall(source)]
    label_counts = Counter(all_labels)
    for label, count in sorted(label_counts.items()):
        if count > 1:
            errors.append(f"duplicate label {label!r} occurs {count} times")
        if not VALID_LABEL_RE.fullmatch(label):
            errors.append(f"label {label!r} is not a stable semantic label")

    for raw_targets in USES_RE.findall(source):
        targets = split_csv(raw_targets)
        if not targets:
            errors.append("an empty \\uses{} marker is not allowed")
        for target in targets:
            if target not in node_label_counts:
                errors.append(f"\\uses target {target!r} has no matching roadmap node")

    if not nodes:
        errors.append("no theorem-like Blueprint nodes found")

    for node_index, match in enumerate(nodes, start=1):
        kind = match.group("kind")
        body = match.group("body")
        labels = [value.strip() for value in LABEL_RE.findall(body)]
        node_name = labels[0] if len(labels) == 1 else f"node #{node_index}"

        if len(labels) != 1:
            errors.append(
                f"{node_name} ({kind}) must contain exactly one \\label; found {len(labels)}"
            )

        statuses = [value.strip() for value in STATUS_RE.findall(body)]
        if len(statuses) != 1:
            errors.append(
                f"{node_name} must contain exactly one \\bpstatus; found {len(statuses)}"
            )
            continue

        status = statuses[0]
        if status not in VALID_STATES:
            errors.append(f"{node_name} has unknown Blueprint status {status!r}")
            continue

        notready_count = command_count(body, "notready")
        leanok_count = command_count(body, "leanok")
        mathlibok_count = command_count(body, "mathlibok")
        lean_markers = LEAN_RE.findall(body)
        lean_decls = [decl for marker in lean_markers for decl in split_csv(marker)]

        if mathlibok_count:
            errors.append(f"{node_name} must not use \\mathlibok as a status shortcut")

        if kind == "assumption" and status != "assumption":
            errors.append(f"{node_name} uses the assumption environment but status is {status!r}")
        if status == "assumption" and kind != "assumption":
            errors.append(f"{node_name} has assumption status outside an assumption environment")

        if status == "planned":
            if notready_count != 1:
                errors.append(f"{node_name} planned state requires exactly one \\notready")
            if lean_markers:
                errors.append(f"{node_name} planned state must not contain \\lean")
            if leanok_count:
                errors.append(f"{node_name} planned state must not contain \\leanok")
        elif status == "linked":
            if notready_count:
                errors.append(f"{node_name} linked state must not contain \\notready")
            if len(lean_markers) != 1 or len(lean_decls) != 1:
                errors.append(f"{node_name} linked state requires exactly one Lean declaration link")
            if leanok_count:
                errors.append(f"{node_name} linked state must not contain \\leanok")
        elif status == "proved":
            if notready_count:
                errors.append(f"{node_name} proved state must not contain \\notready")
            if len(lean_markers) != 1 or len(lean_decls) != 1:
                errors.append(f"{node_name} proved state requires exactly one Lean declaration link")
            if leanok_count != 1:
                errors.append(f"{node_name} proved state requires exactly one \\leanok")
        else:  # assumption
            if notready_count != 1:
                errors.append(f"{node_name} assumption state requires exactly one \\notready")
            if len(lean_markers) > 1 or (lean_markers and len(lean_decls) != 1):
                errors.append(f"{node_name} assumption state permits at most one Lean declaration link")
            if leanok_count:
                errors.append(f"{node_name} assumption state must never contain \\leanok")

        for declaration in lean_decls:
            if not declaration.startswith(STABLE_LEAN_PREFIXES):
                allowed = ", ".join(prefix.removesuffix(".") for prefix in STABLE_LEAN_PREFIXES)
                errors.append(
                    f"{node_name} links unstable Lean declaration {declaration!r}; "
                    f"expected a handwritten declaration below one of: {allowed}"
                )

    if require_roadmap:
        missing = sorted(REQUIRED_LABELS.difference(node_label_counts))
        if missing:
            errors.append("roadmap is missing required labels: " + ", ".join(missing))

    return errors


def run_self_tests() -> None:
    valid = r"""
\begin{definition}
  \label{def:planned}
  \bpstatus{planned}\notready
\end{definition}
\begin{theorem}
  \label{thm:linked}
  \bpstatus{linked}\lean{Greatgramma.Refinement.Linked}
  \uses{def:planned}
\end{theorem}
\begin{theorem}
  \label{thm:proved}
  \bpstatus{proved}\lean{Greatgramma.Theorems.Proved}\leanok
  \uses{thm:linked}
\end{theorem}
\begin{assumption}
  \label{asm:conditional}
  \bpstatus{assumption}\notready
\end{assumption}
"""
    valid_errors = validate_source(valid, require_roadmap=False)
    if valid_errors:
        raise AssertionError("valid four-state fixture failed: " + "; ".join(valid_errors))

    dangling = r"""
\begin{theorem}
  \label{thm:dangling}
  \bpstatus{planned}\notready
  \uses{def:missing}
\end{theorem}
"""
    dangling_errors = validate_source(dangling, require_roadmap=False)
    if not any("has no matching roadmap node" in error for error in dangling_errors):
        raise AssertionError("dangling-edge fixture was accepted")

    invalid_state = r"""
\begin{definition}
  \label{def:invalid-state}
  \bpstatus{planned}\notready
  \lean{Greatgramma.Spec.TooEarly}
\end{definition}
"""
    state_errors = validate_source(invalid_state, require_roadmap=False)
    if not any("planned state must not contain \\lean" in error for error in state_errors):
        raise AssertionError("invalid planned-plus-link fixture was accepted")

    included = r"""
\begin{definition}
  \label{def:included}
  \bpstatus{planned}\notready
\end{definition}
\input{unchecked-chapter}
"""
    included_errors = validate_source(included, require_roadmap=False)
    if not any("must be self-contained" in error for error in included_errors):
        raise AssertionError("nested-input fixture was accepted")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "source",
        nargs="?",
        type=Path,
        default=DEFAULT_SOURCE,
        help=f"Blueprint content source (default: {DEFAULT_SOURCE.relative_to(REPO_ROOT)})",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run built-in positive and negative checker fixtures before validation",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_tests()
        print("Blueprint source checker self-tests: ok")

    try:
        source = args.source.read_text(encoding="utf-8")
    except OSError as error:
        print(f"error: cannot read {args.source}: {error}", file=sys.stderr)
        return 2

    errors = validate_source(source)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    try:
        display_path = args.source.resolve().relative_to(REPO_ROOT)
    except ValueError:
        display_path = args.source
    print(
        f"Blueprint source check passed: {display_path} "
        f"({len(list(NODE_RE.finditer(strip_comments(source))))} nodes)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
