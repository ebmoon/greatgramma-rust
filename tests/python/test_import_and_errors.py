from __future__ import annotations

import subprocess
import sys

from greatgramma import InternalError
from greatgramma import _native
from greatgramma.errors import from_native


def test_base_import_does_not_import_optional_frameworks() -> None:
    script = """
import sys
import greatgramma
assert "torch" not in sys.modules
assert "transformers" not in sys.modules
assert greatgramma.GreatGrammaLogitsProcessor.__name__ == "GreatGrammaLogitsProcessor"
"""
    subprocess.run([sys.executable, "-c", script], check=True)


def test_unknown_native_error_code_is_hidden_as_internal_error() -> None:
    native = _native.NativeError("future_private_code", "bad metadata", 2, 7)

    public = from_native(native, position=11)

    assert isinstance(public, InternalError)
    assert public.code == "internal_error"
    assert public.row == 2
    assert public.token == 7
    assert public.position == 11


def test_malformed_native_error_is_hidden_as_internal_error() -> None:
    public = from_native(_native.NativeError("not the tuple ABI"), position=3)

    assert isinstance(public, InternalError)
    assert public.code == "internal_error"
    assert public.position == 3
