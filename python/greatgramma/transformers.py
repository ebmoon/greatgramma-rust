"""Strict fixed-row Transformers-compatible integration."""

from __future__ import annotations

import threading
from collections.abc import Callable, Sequence
from typing import Any, cast

from . import _native
from ._mask import torch_mask_scores
from .errors import (
    ConfigurationError,
    ConstraintViolationError,
    GreatGrammaError,
    InternalError,
    SequenceDiscontinuityError,
    from_native,
)


def _rows(value: object, *, label: str) -> tuple[tuple[int, ...], ...]:
    tolist = getattr(value, "tolist", None)
    raw: object = cast(Callable[[], object], tolist)() if callable(tolist) else value
    if not isinstance(raw, Sequence) or isinstance(raw, (str, bytes)) or not raw:
        raise ConfigurationError(
            f"{label} must be a nonempty two-dimensional integer array",
        )
    result: list[tuple[int, ...]] = []
    width: int | None = None
    for row_index, raw_row in enumerate(cast(Sequence[object], raw)):
        if not isinstance(raw_row, Sequence) or isinstance(raw_row, (str, bytes)):
            raise ConfigurationError(
                f"{label} row {row_index} is not an integer sequence",
                row=row_index,
            )
        values = tuple(cast(Sequence[object], raw_row))
        if not values or any(type(token) is not int for token in values):
            raise ConfigurationError(
                f"{label} row {row_index} must contain integers",
                row=row_index,
            )
        row = cast(tuple[int, ...], values)
        if width is None:
            width = len(row)
        elif len(row) != width:
            raise ConfigurationError(
                f"{label} rows must have one fixed padded width",
                row=row_index,
            )
        result.append(row)
    return tuple(result)


def _score_shape(scores: object) -> tuple[int, int]:
    shape = getattr(scores, "shape", None)
    if not isinstance(shape, Sequence):
        raise ConfigurationError(
            "scores must be a two-dimensional array",
        )
    values = tuple(cast(Sequence[object], shape))
    if len(values) != 2 or any(type(value) is not int for value in values):
        raise ConfigurationError(
            "scores must be a two-dimensional array",
        )
    return cast(int, values[0]), cast(int, values[1])


def _input_shape(input_ids: object) -> tuple[int, int]:
    shape = getattr(input_ids, "shape", None)
    if not isinstance(shape, Sequence):
        raise ConfigurationError(
            "input_ids must be a nonempty two-dimensional integer array",
        )
    values = tuple(cast(Sequence[object], shape))
    if (
        len(values) != 2
        or any(type(value) is not int for value in values)
        or cast(int, values[0]) <= 0
        or cast(int, values[1]) <= 0
    ):
        raise ConfigurationError(
            "input_ids must be a nonempty two-dimensional integer array",
        )
    return cast(int, values[0]), cast(int, values[1])


def _last_tokens(input_ids: object, rows: int) -> list[int]:
    try:
        column = cast(Any, input_ids)[:, -1]
        tolist = getattr(column, "tolist", None)
        raw: object = cast(Callable[[], object], tolist)() if callable(tolist) else column
    except Exception as error:
        raise ConfigurationError(
            "input_ids must expose its final token column",
        ) from error
    if not isinstance(raw, Sequence) or isinstance(raw, (str, bytes)):
        raise ConfigurationError(
            "input_ids final column must contain integers",
        )
    values = list(cast(Sequence[object], raw))
    if len(values) != rows or any(type(token) is not int for token in values):
        raise ConfigurationError(
            "input_ids final column must contain one integer per row",
        )
    return cast(list[int], values)


class GreatGrammaLogitsProcessor:
    """Strict fixed-row processor; direct use validates every complete prefix."""

    def __init__(
        self,
        compiled: Any,
        prompt: object,
        *,
        pad_token_id: int | None = None,
        _masker: Callable[[object, bytes, int, int], object] | None = None,
        _trusted_fixed_append: bool = False,
    ) -> None:
        prompt_rows = _rows(prompt, label="prompt")
        self._prompt = prompt_rows
        self._previous: tuple[tuple[int, ...], ...] | None = None
        self._vocab_size: int = compiled.vocab_size
        self._masker = _masker or torch_mask_scores
        self._trusted_fixed_append = _trusted_fixed_append
        self._previous_width: int | None = None
        self._score_vocab_size: int | None = None
        self._lock = threading.Lock()
        self._poisoned = False
        for row_index, row in enumerate(prompt_rows):
            if any(token < 0 or token >= self._vocab_size for token in row):
                raise ConfigurationError(
                    "prompt token ID is outside the compiled vocabulary",
                    row=row_index,
                )
        self._batch = compiled._new_batch(len(prompt_rows), pad_token_id)

    def __call__(self, input_ids: object, scores: object) -> object:
        if not self._lock.acquire(blocking=False):
            raise SequenceDiscontinuityError(
                "the logits processor cannot be used concurrently",
            )
        try:
            return self._call_locked(input_ids, scores)
        finally:
            self._lock.release()

    def _call_locked(self, input_ids: object, scores: object) -> object:
        if self._poisoned:
            raise InternalError(
                "the logits processor is poisoned after an earlier internal failure",
            )
        parsed_rows: tuple[tuple[int, ...], ...] | None
        if self._trusted_fixed_append and self._previous_width is not None:
            row_count, position = _input_shape(input_ids)
            parsed_rows = None
        else:
            parsed_rows = _rows(input_ids, label="input_ids")
            row_count = len(parsed_rows)
            position = len(parsed_rows[0])
        score_rows, score_vocab = _score_shape(scores)
        if row_count != len(self._prompt):
            raise SequenceDiscontinuityError(
                "the generation row count changed",
            )
        if score_rows != len(self._prompt) or score_vocab < self._vocab_size:
            raise ConfigurationError(
                "scores do not match the fixed batch or are narrower than the "
                "compiled vocabulary",
            )
        if self._score_vocab_size is not None and score_vocab != self._score_vocab_size:
            raise ConfigurationError(
                "scores vocabulary width changed during generation",
            )

        try:
            if self._previous_width is None:
                if parsed_rows != self._prompt:
                    raise SequenceDiscontinuityError(
                        "the first callback must exactly match the prompt snapshot",
                        position=position,
                    )
                packed = bytes(self._batch.initial_masks())
            elif self._trusted_fixed_append:
                if position != self._previous_width + 1:
                    raise SequenceDiscontinuityError(
                        "each fixed row must extend its prior prefix by exactly one token",
                        position=position,
                    )
                appended_tokens = _last_tokens(input_ids, row_count)
                for row_index, token in enumerate(appended_tokens):
                    if token < 0 or token >= self._vocab_size:
                        raise ConstraintViolationError(
                            "generated token ID is outside the compiled vocabulary",
                            row=row_index,
                            token=token,
                            position=position,
                        )
                packed = bytes(self._batch.advance_and_masks(appended_tokens))
            else:
                if parsed_rows is None or self._previous is None:
                    raise InternalError(
                        "the logits processor history invariant was violated",
                    )
                tokens: list[int] = []
                for row_index, (previous, current) in enumerate(
                    zip(self._previous, parsed_rows, strict=True)
                ):
                    if len(current) != len(previous) + 1 or current[:-1] != previous:
                        raise SequenceDiscontinuityError(
                            "each fixed row must extend its prior prefix by exactly one token",
                            row=row_index,
                            position=position,
                        )
                    if current[-1] < 0 or current[-1] >= self._vocab_size:
                        raise ConstraintViolationError(
                            "generated token ID is outside the compiled vocabulary",
                            row=row_index,
                            token=current[-1],
                            position=position,
                        )
                    tokens.append(current[-1])
                packed = bytes(self._batch.advance_and_masks(tokens))
        except _native.NativeError as error:
            public = from_native(error, position=position)
            if isinstance(public, InternalError):
                self._poisoned = True
            raise public from error

        try:
            result = self._masker(scores, packed, row_count, self._vocab_size)
        except GreatGrammaError:
            self._poisoned = True
            raise
        except Exception as error:
            self._poisoned = True
            raise InternalError(
                f"mask application failed: {error}",
                position=position,
            ) from error
        except BaseException:
            self._poisoned = True
            raise
        self._previous_width = position
        if not self._trusted_fixed_append:
            if parsed_rows is None:
                raise InternalError(
                    "the logits processor history invariant was violated",
                )
            self._previous = parsed_rows
        self._score_vocab_size = score_vocab
        return result


def _eos_ids(value: object) -> frozenset[int]:
    if type(value) is int:
        return frozenset((value,))
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        result: set[int] = set()
        for token in cast(Sequence[object], value):
            if type(token) is not int:
                raise ConfigurationError(
                    "generation EOS settings must contain only token IDs",
                )
            result.add(token)
        if result:
            return frozenset(result)
    raise ConfigurationError(
        "generation EOS settings must contain at least one token ID",
    )


def _config_value(config: object, name: str, default: object = None) -> object:
    return getattr(config, name, default)


def generate(
    compiled: Any,
    model: object,
    input_ids: object,
    *,
    generation_config: object,
    logits_processors: Sequence[object] = (),
    assistant_model: object | None = None,
) -> Any:
    prompt = _rows(input_ids, label="input_ids")
    model_config = getattr(model, "config", None)
    if model_config is None or bool(getattr(model_config, "is_encoder_decoder", False)):
        raise ConfigurationError(
            "the supported wrapper requires a decoder-only model",
        )
    if generation_config is None:
        raise ConfigurationError(
            "a resolved generation_config is required",
        )
    if _eos_ids(_config_value(generation_config, "eos_token_id")) != compiled.eos_token_ids:
        raise ConfigurationError(
            "generation EOS settings must exactly match the compiled EOS set",
        )
    pad_token_id = _config_value(generation_config, "pad_token_id")
    if pad_token_id is not None and type(pad_token_id) is not int:
        raise ConfigurationError(
            "generation pad_token_id must be an integer or None",
        )
    if len(prompt) > 1 and type(pad_token_id) is not int:
        raise ConfigurationError(
            "batches larger than one require a pad token ID",
        )
    if len(prompt) > 1 and pad_token_id in compiled.eos_token_ids:
        raise ConfigurationError(
            "batched generation cannot infer attention when the pad token is also an EOS token",
        )

    exact_ones = ("num_beams", "num_return_sequences", "num_beam_groups")
    for name in exact_ones:
        if _config_value(generation_config, name, 1) not in (None, 1):
            raise ConfigurationError(
                f"unsupported generation setting: {name}",
            )
    disabled = (
        "penalty_alpha",
        "prompt_lookup_num_tokens",
        "use_mtp",
        "assistant_early_exit",
        "token_healing",
        "continuous_batching",
        "continuous_batching_config",
        "constraints",
        "force_words_ids",
        "dola_layers",
    )
    for name in disabled:
        if _config_value(generation_config, name) not in (None, 0, False):
            raise ConfigurationError(
                f"unsupported generation setting: {name}",
            )
    if assistant_model is not None:
        raise ConfigurationError(
            "assisted generation is unsupported",
        )
    get_generation_mode = getattr(generation_config, "get_generation_mode", None)
    if callable(get_generation_mode):
        mode = cast(Callable[[], object], get_generation_mode)()
        mode_name = getattr(mode, "value", mode)
        if mode_name not in ("greedy_search", "sample"):
            raise ConfigurationError(
                f"unsupported generation mode: {mode_name}",
            )

    generate_method = getattr(model, "generate", None)
    if not callable(generate_method):
        raise ConfigurationError(
            "model does not expose generate()",
        )
    processor = compiled.logits_processor(
        input_ids,
        pad_token_id=pad_token_id if type(pad_token_id) is int else None,
        _trusted_fixed_append=True,
    )
    processors = list(logits_processors)
    processors.append(processor)
    return generate_method(
        input_ids=input_ids,
        generation_config=generation_config,
        logits_processor=processors,
    )
