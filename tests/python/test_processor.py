from __future__ import annotations

import threading
from collections.abc import Callable
from dataclasses import dataclass
from typing import cast

import pytest

from greatgramma import (
    ConfigurationError,
    ConstraintViolationError,
    GreatGrammaLogitsProcessor,
    InternalError,
    SequenceDiscontinuityError,
)
from greatgramma import _native


@dataclass(frozen=True)
class Scores:
    shape: tuple[int, int]
    marker: str = "scores"


class FakeBatch:
    def __init__(self, initial: bytes) -> None:
        self.initial = initial
        self.initial_calls = 0
        self.advance_calls: list[list[int]] = []
        self.reject_token: int | None = None

    def initial_masks(self) -> bytes:
        self.initial_calls += 1
        return self.initial

    def advance_and_masks(self, tokens: list[int]) -> bytes:
        self.advance_calls.append(tokens.copy())
        if self.reject_token is not None and self.reject_token in tokens:
            row = tokens.index(self.reject_token)
            raise _native.NativeError(
                "constraint_violation",
                "rejected by test double",
                row,
                self.reject_token,
            )
        return self.initial


class FakeCompiled:
    def __init__(self, vocab_size: int, packed: bytes) -> None:
        self.vocab_size = vocab_size
        self.batch = FakeBatch(packed)
        self.new_batch_calls: list[tuple[int, int | None]] = []

    def _new_batch(self, rows: int, pad_token_id: int | None) -> FakeBatch:
        self.new_batch_calls.append((rows, pad_token_id))
        return self.batch


class LastColumnOnly:
    """Tensor-shaped input which fails if its complete history is copied."""

    def __init__(self, rows: tuple[tuple[int, ...], ...]) -> None:
        self.shape = (len(rows), len(rows[0]))
        self._last = [row[-1] for row in rows]
        self.full_tolist_calls = 0
        self.last_column_reads = 0

    def tolist(self) -> object:
        self.full_tolist_calls += 1
        raise AssertionError("the trusted append path copied the complete prefix")

    def __getitem__(self, key: object) -> object:
        assert key == (slice(None), -1)
        self.last_column_reads += 1

        class Column:
            def __init__(self, values: list[int]) -> None:
                self._values = values

            def tolist(self) -> list[int]:
                return self._values.copy()

        return Column(self._last)


def unpack(packed: bytes, rows: int, vocab_size: int) -> tuple[tuple[bool, ...], ...]:
    width = (vocab_size + 7) // 8
    return tuple(
        tuple(
            bool(packed[row * width + token // 8] & (1 << (token % 8)))
            for token in range(vocab_size)
        )
        for row in range(rows)
    )


def recording_masker(
    calls: list[tuple[bytes, int, int]],
) -> Callable[[object, bytes, int, int], object]:
    def apply(scores: object, packed: bytes, rows: int, vocab_size: int) -> object:
        calls.append((packed, rows, vocab_size))
        return (scores, unpack(packed, rows, vocab_size))

    return apply


def test_prompt_is_a_boundary_and_each_row_extends_once() -> None:
    calls: list[tuple[bytes, int, int]] = []
    compiled = FakeCompiled(5, bytes((0b0001_0101, 0b0000_0010)))
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[4, 4], [4, 3]],
        pad_token_id=0,
        _masker=recording_masker(calls),
    )

    first = processor([[4, 4], [4, 3]], Scores((2, 5)))
    second = processor([[4, 4, 2], [4, 3, 1]], Scores((2, 5)))

    assert compiled.new_batch_calls == [(2, 0)]
    assert compiled.batch.initial_calls == 1
    assert compiled.batch.advance_calls == [[2, 1]]
    assert calls == [
        (bytes((0b0001_0101, 0b0000_0010)), 2, 5),
        (bytes((0b0001_0101, 0b0000_0010)), 2, 5),
    ]
    first_result = cast(tuple[Scores, tuple[tuple[bool, ...], ...]], first)
    second_result = cast(tuple[Scores, tuple[tuple[bool, ...], ...]], second)
    assert first_result[0].marker == "scores"
    assert second_result[0].marker == "scores"


def test_trusted_fixed_append_reads_only_shape_and_last_column() -> None:
    compiled = FakeCompiled(8, b"\xff\xff")
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[1], [2]],
        pad_token_id=0,
        _masker=lambda scores, _packed, _rows, _vocab: scores,
        _trusted_fixed_append=True,
    )
    processor([[1], [2]], Scores((2, 8)))
    extended = LastColumnOnly(((1, 3), (2, 4)))

    processor(extended, Scores((2, 8)))

    assert extended.full_tolist_calls == 0
    assert extended.last_column_reads == 1
    assert compiled.batch.advance_calls == [[3, 4]]


def test_trusted_fixed_append_keeps_width_snapshot_after_native_rejection() -> None:
    compiled = FakeCompiled(8, b"\xff")
    compiled.batch.reject_token = 7
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[1]],
        _masker=lambda scores, _packed, _rows, _vocab: scores,
        _trusted_fixed_append=True,
    )
    processor([[1]], Scores((1, 8)))

    with pytest.raises(ConstraintViolationError):
        processor(LastColumnOnly(((1, 7),)), Scores((1, 8)))

    compiled.batch.reject_token = None
    processor(LastColumnOnly(((1, 2),)), Scores((1, 8)))
    assert compiled.batch.advance_calls == [[7], [2]]


@pytest.mark.parametrize(
    "current",
    (
        [[1, 2]],
        [[1, 2, 3, 4]],
        [[9, 2, 3]],
        [[1, 2, 3], [1, 2, 3]],
    ),
)
def test_sequence_discontinuities_fail_before_native_advance(
    current: list[list[int]],
) -> None:
    compiled = FakeCompiled(8, b"\xff")
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[1, 2]],
        _masker=lambda scores, _packed, _rows, _vocab: scores,
    )
    processor([[1, 2]], Scores((1, 8)))

    with pytest.raises(SequenceDiscontinuityError):
        processor(current, Scores((len(current), 8)))

    assert compiled.batch.advance_calls == []


def test_native_rejection_does_not_advance_python_snapshot() -> None:
    compiled = FakeCompiled(8, b"\xff")
    compiled.batch.reject_token = 7
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[1]],
        _masker=lambda scores, _packed, _rows, _vocab: scores,
    )
    processor([[1]], Scores((1, 8)))

    with pytest.raises(ConstraintViolationError) as rejected:
        processor([[1, 7]], Scores((1, 8)))
    assert rejected.value.row == 0
    assert rejected.value.token == 7

    compiled.batch.reject_token = None
    processor([[1, 2]], Scores((1, 8)))
    assert compiled.batch.advance_calls == [[7], [2]]


def test_unsigned_token_validation_uses_stable_public_errors() -> None:
    invalid_prompt = FakeCompiled(8, b"\xff")
    with pytest.raises(ConfigurationError) as prompt_error:
        GreatGrammaLogitsProcessor(
            invalid_prompt,
            [[-1]],
            _masker=lambda scores, _packed, _rows, _vocab: scores,
        )
    assert prompt_error.value.code == "configuration_error"
    assert invalid_prompt.new_batch_calls == []

    compiled = FakeCompiled(8, b"\xff")
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[1]],
        _masker=lambda scores, _packed, _rows, _vocab: scores,
    )
    processor([[1]], Scores((1, 8)))
    with pytest.raises(ConstraintViolationError) as token_error:
        processor([[1, -1]], Scores((1, 8)))
    assert token_error.value.code == "constraint_violation"
    assert token_error.value.token == -1
    assert compiled.batch.advance_calls == []


@pytest.mark.parametrize("vocab_size", (1, 7, 8, 9, 15, 16, 17))
def test_packed_mask_abi_at_every_byte_boundary(vocab_size: int) -> None:
    width = (vocab_size + 7) // 8
    row0 = bytearray(width)
    row1 = bytearray(width)
    row0[0] |= 1
    row0[(vocab_size - 1) // 8] |= 1 << ((vocab_size - 1) % 8)
    row1[(vocab_size // 2) // 8] |= 1 << ((vocab_size // 2) % 8)
    packed = bytes(row0 + row1)
    captured: list[tuple[bytes, int, int]] = []
    processor = GreatGrammaLogitsProcessor(
        FakeCompiled(vocab_size, packed),
        [[0], [0]],
        pad_token_id=0,
        _masker=recording_masker(captured),
    )

    result = processor([[0], [0]], Scores((2, vocab_size)))
    allowed = cast(
        tuple[Scores, tuple[tuple[bool, ...], ...]],
        result,
    )[1]

    assert captured == [(packed, 2, vocab_size)]
    assert len(packed) == 2 * width
    assert allowed[0][0]
    assert allowed[0][-1]
    assert allowed[1][vocab_size // 2]
    if vocab_size % 8:
        assert packed[width - 1] >> (vocab_size % 8) == 0
        assert packed[-1] >> (vocab_size % 8) == 0


def test_wrong_score_shape_fails_before_native_state_changes() -> None:
    compiled = FakeCompiled(9, b"\x01\x00")
    processor = GreatGrammaLogitsProcessor(
        compiled,
        [[1]],
        _masker=lambda scores, _packed, _rows, _vocab: scores,
    )

    with pytest.raises(ConfigurationError):
        processor([[1]], Scores((1, 8)))

    assert compiled.batch.initial_calls == 0


def test_concurrent_use_is_rejected() -> None:
    entered = threading.Event()
    release = threading.Event()
    failures: list[BaseException] = []

    def blocking_masker(
        scores: object,
        _packed: bytes,
        _rows: int,
        _vocab: int,
    ) -> object:
        entered.set()
        assert release.wait(timeout=5)
        return scores

    processor = GreatGrammaLogitsProcessor(
        FakeCompiled(8, b"\xff"),
        [[1]],
        _masker=blocking_masker,
    )

    def invoke() -> None:
        try:
            processor([[1]], Scores((1, 8)))
        except BaseException as error:
            failures.append(error)

    thread = threading.Thread(target=invoke)
    thread.start()
    assert entered.wait(timeout=5)
    try:
        with pytest.raises(SequenceDiscontinuityError):
            processor([[1]], Scores((1, 8)))
    finally:
        release.set()
        thread.join(timeout=5)

    assert not thread.is_alive()
    assert failures == []


def test_mask_failure_poisons_the_processor() -> None:
    def fail_mask(
        _scores: object,
        _packed: bytes,
        _rows: int,
        _vocab: int,
    ) -> object:
        raise ValueError("test failure")

    processor = GreatGrammaLogitsProcessor(
        FakeCompiled(8, b"\xff"),
        [[1]],
        _masker=fail_mask,
    )

    with pytest.raises(InternalError):
        processor([[1]], Scores((1, 8)))
    with pytest.raises(InternalError, match="poisoned"):
        processor([[1]], Scores((1, 8)))


def test_base_exception_after_native_advance_poisons_the_processor() -> None:
    calls = 0

    def interrupt_second_mask(
        scores: object,
        _packed: bytes,
        _rows: int,
        _vocab: int,
    ) -> object:
        nonlocal calls
        calls += 1
        if calls == 2:
            raise KeyboardInterrupt
        return scores

    processor = GreatGrammaLogitsProcessor(
        FakeCompiled(8, b"\xff"),
        [[1]],
        _masker=interrupt_second_mask,
    )
    processor([[1]], Scores((1, 8)))

    with pytest.raises(KeyboardInterrupt):
        processor([[1, 2]], Scores((1, 8)))
    with pytest.raises(InternalError, match="poisoned"):
        processor([[1, 2]], Scores((1, 8)))
