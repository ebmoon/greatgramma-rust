from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest


@pytest.mark.skipif(os.name == "nt", reason="requires a POSIX shell")
def test_package_check_resolves_python_command_before_passing_it_to_uv(
    tmp_path: Path,
) -> None:
    shell = shutil.which("sh")
    assert shell is not None

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    (fake_bin / "python").symlink_to(sys.executable)

    uv_arguments = tmp_path / "uv-arguments"
    fake_uv = fake_bin / "uv"
    fake_uv.write_text(
        '#!/bin/sh\nprintf \'%s\\n\' "$@" > "$UV_ARGUMENTS"\nexit 86\n',
        encoding="utf-8",
    )
    fake_uv.chmod(0o755)

    repo_root = Path(__file__).resolve().parents[2]
    env = os.environ.copy()
    env.update(
        {
            "PACKAGE_CHECK_DIR": str(tmp_path / "artifacts"),
            "PATH": f"{fake_bin}{os.pathsep}{env['PATH']}",
            "PYTHON": "python",
            "UV_ARGUMENTS": str(uv_arguments),
        }
    )
    result = subprocess.run(
        [shell, str(repo_root / "scripts" / "package-check.sh")],
        cwd=repo_root,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 86
    arguments = uv_arguments.read_text(encoding="utf-8").splitlines()
    python_argument = arguments[arguments.index("--python") + 1]
    assert python_argument != "python"
    assert Path(python_argument).is_absolute()
    assert Path(python_argument).resolve() == Path(sys.executable).resolve()
