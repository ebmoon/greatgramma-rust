#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

if ! command -v uv >/dev/null 2>&1; then
    echo "package check requires uv" >&2
    exit 2
fi

temporary=0
if [ -n "${PACKAGE_CHECK_DIR:-}" ]; then
    scratch=$PACKAGE_CHECK_DIR
    if [ -e "$scratch" ]; then
        echo "PACKAGE_CHECK_DIR already exists: $scratch" >&2
        exit 2
    fi
    mkdir -p "$scratch"
else
    scratch=$(mktemp -d "${TMPDIR:-/tmp}/greatgramma-package.XXXXXX")
    temporary=1
fi
scratch=$(CDPATH= cd -- "$scratch" && pwd)

cleanup() {
    status=$?
    trap - EXIT HUP INT TERM
    if [ "$temporary" -eq 1 ] && [ "$status" -eq 0 ] && [ "${KEEP_PACKAGE_CHECK:-0}" != 1 ]; then
        rm -rf -- "$scratch"
    else
        echo "package-check artifacts: $scratch" >&2
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

dist=$scratch/dist
venv=$scratch/venv
mkdir -p "$dist"

cd "$repo_root"
python=${PYTHON:-python3}
pinned_rust=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml | head -n 1)
if [ -z "$pinned_rust" ]; then
    echo "rust-toolchain.toml does not declare a pinned channel" >&2
    exit 2
fi
rust_toolchain=${RUSTUP_TOOLCHAIN:-$pinned_rust}
RUSTUP_TOOLCHAIN=$rust_toolchain uv build --sdist --out-dir "$dist" --python "$python"

sdist=$(find "$dist" -maxdepth 1 -type f -name '*.tar.gz' -print | head -n 1)
if [ -z "$sdist" ]; then
    echo "maturin did not produce an sdist" >&2
    exit 1
fi

RUSTUP_TOOLCHAIN=$rust_toolchain uv build --wheel --out-dir "$dist" --python "$python" "$sdist"
wheel=$(find "$dist" -maxdepth 1 -type f -name '*.whl' -print | head -n 1)
if [ -z "$wheel" ]; then
    echo "maturin did not produce a wheel from the sdist" >&2
    exit 1
fi

case $(basename -- "$wheel") in
    *-abi3-*.whl) ;;
    *)
        echo "wheel is not tagged for the abi3 stable ABI: $wheel" >&2
        exit 1
        ;;
esac

uv venv --python "$python" "$venv"
venv_python=$venv/bin/python
if [ ! -x "$venv_python" ]; then
    venv_python=$venv/Scripts/python.exe
fi
uv pip install --python "$venv_python" --no-deps "$wheel"

"$venv_python" -I -c '
import sys
import greatgramma
from greatgramma import _native

assert callable(greatgramma.compile)
assert _native.__name__ == "greatgramma._native"
assert "torch" not in sys.modules
assert "transformers" not in sys.modules
'

echo "package check passed: $(basename -- "$wheel"), $(basename -- "$sdist")"
