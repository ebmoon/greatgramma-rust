"""Packed-mask expansion with a lazy optional Torch dependency."""

from __future__ import annotations

import threading
from importlib import import_module
from typing import Any, cast

from .errors import ConfigurationError, NoValidTokenError

_BIT_VALUES: dict[object, Any] = {}
_TABLE_LOCK = threading.Lock()


def torch_mask_scores(scores: object, packed: bytes, rows: int, vocab_size: int) -> object:
    try:
        torch = cast(Any, import_module("torch"))
    except ImportError as error:
        raise ConfigurationError(
            "Torch is required to apply GreatGramma masks",
        ) from error

    if not isinstance(scores, torch.Tensor):
        raise ConfigurationError(
            "scores must be a Torch tensor",
        )
    tensor = cast(Any, scores)
    if (
        tensor.ndim != 2
        or int(tensor.shape[0]) != rows
        or int(tensor.shape[1]) < vocab_size
    ):
        raise ConfigurationError(
            "scores must have one row per sequence and at least the compiled "
            "vocabulary width",
        )
    if not tensor.is_floating_point():
        raise ConfigurationError(
            "scores must have a floating dtype",
        )

    device = tensor.device
    with _TABLE_LOCK:
        bit_values = _BIT_VALUES.get(device)
        if bit_values is None:
            bit_values = torch.tensor(
                (1, 2, 4, 8, 16, 32, 64, 128),
                dtype=torch.uint8,
                device=device,
            ).unsqueeze(0)
            _BIT_VALUES[device] = bit_values

    packed_tensor = torch.frombuffer(bytearray(packed), dtype=torch.uint8).to(device)
    allowed = packed_tensor.unsqueeze(1).bitwise_and(bit_values).ne(0)
    allowed = allowed.reshape(rows, -1)[:, :vocab_size]
    model_vocab_size = int(tensor.shape[1])
    if model_vocab_size > vocab_size:
        suffix = torch.zeros(
            (rows, model_vocab_size - vocab_size),
            dtype=torch.bool,
            device=device,
        )
        allowed = torch.cat((allowed, suffix), dim=1)
    masked = tensor.masked_fill(~allowed, float("-inf"))
    usable = (~torch.isneginf(masked) & ~torch.isnan(masked)).any(dim=1)
    if not bool(usable.all().item()):
        row = int((~usable).nonzero()[0].item())
        raise NoValidTokenError(
            "no grammar-allowed token retains a usable score",
            row=row,
        )
    return masked
