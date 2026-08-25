from __future__ import annotations

import pytest
from collections.abc import Iterator
from typing import Any, cast

from greatgramma import NoValidTokenError
from greatgramma._mask import torch_mask_scores

torch = pytest.importorskip("torch")


class _NonIterableBytes(bytes):
    def __iter__(self) -> Iterator[int]:
        raise AssertionError("packed masks must be consumed through the buffer protocol")


@pytest.mark.parametrize("dtype", (torch.float16, torch.bfloat16, torch.float32, torch.float64))
def test_torch_mask_is_out_of_place_and_preserves_cpu_float_dtype(dtype: object) -> None:
    scores = torch.tensor([[1.0, 2.0], [3.0, 4.0]], dtype=dtype)
    result = torch_mask_scores(scores, _NonIterableBytes((0b01, 0b10)), 2, 2)
    runtime_result = cast(Any, result)

    assert result is not scores
    assert runtime_result.dtype == scores.dtype
    assert runtime_result.tolist() == [[1.0, float("-inf")], [float("-inf"), 4.0]]
    assert scores.tolist() == [[1.0, 2.0], [3.0, 4.0]]


def test_torch_mask_rejects_an_exhausted_row() -> None:
    exhausted = torch.tensor([[0.0, float("-inf")]], dtype=torch.float32)

    with pytest.raises(NoValidTokenError) as raised:
        torch_mask_scores(exhausted, bytes((0b10,)), 1, 2)
    assert raised.value.code == "no_valid_token"
    assert raised.value.row == 0


@pytest.mark.parametrize("dtype", (torch.float16, torch.bfloat16, torch.float32))
def test_torch_mask_preserves_supported_cuda_dtype_when_available(dtype: object) -> None:
    if not torch.cuda.is_available():
        pytest.skip("CUDA is not available")
    scores = torch.tensor([[1.0, 2.0]], dtype=dtype, device="cuda")

    result = cast(Any, torch_mask_scores(scores, b"\x01", 1, 2))

    assert result.device.type == "cuda"
    assert result.dtype == dtype
    assert result.tolist() == [[1.0, float("-inf")]]
