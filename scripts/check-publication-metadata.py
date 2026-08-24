#!/usr/bin/env python3
"""Fail closed if bootstrap packages become publishable before licensing."""

from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parent.parent


def project_section(pyproject: str) -> str:
    match = re.search(r"(?ms)^\[project\]\s*$\n(.*?)(?=^\[|\Z)", pyproject)
    if match is None:
        raise ValueError("pyproject.toml has no [project] section")
    return match.group(1)


def main() -> int:
    failures: list[str] = []
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)

    for package in metadata["packages"]:
        if package.get("publish") != []:
            failures.append(f"Rust package {package['name']} is publishable")
        if package.get("license") is not None or package.get("license_file") is not None:
            failures.append(f"Rust package {package['name']} claims a license")

    project = project_section((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    if re.search(r"(?m)^\s*license(?:-files)?\s*=", project):
        failures.append("Python project claims a license or license files")
    if '"Private :: Do Not Upload"' not in project:
        failures.append("Python project lacks the PyPI publication guard")

    if failures:
        print("publication metadata check failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print("publication metadata is fail-closed pending a project license")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
