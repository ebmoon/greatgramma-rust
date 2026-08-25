from __future__ import annotations

from collections.abc import Callable
from typing import Any, cast

import pytest

from greatgramma import InternalError
from greatgramma import _native
from greatgramma.errors import from_native


def test_native_panic_is_translated_when_test_hook_is_enabled() -> None:
    hook = getattr(_native, "_test_panic", None)
    if hook is None:
        pytest.skip("extension was built without panic-test-hook")

    call = cast(Callable[[], Any], hook)
    with pytest.raises(_native.NativeError) as raised:
        call()

    public = from_native(raised.value)
    assert isinstance(public, InternalError)
    assert public.code == "internal_error"
    assert "panicked" in str(public)
