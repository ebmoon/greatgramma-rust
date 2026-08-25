"""Typed exact-byte compiler facade."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Sequence, cast

from . import _native
from .errors import (
    CompileError,
    ConfigurationError,
    TokenizerCompatibilityError,
    from_native,
)

if TYPE_CHECKING:
    from .transformers import GreatGrammaLogitsProcessor

_MAX_VOCAB_SIZE = 1_000_000
_MAX_JSON_BYTES = 64 * 1024 * 1024
_MAX_TOKEN_BYTES = 1 << 30
_MAX_SOURCE_BYTES = 64 * 1024 * 1024
_MAX_REGEX_BYTES = 16 * 1024 * 1024
_MAX_TERMINAL_SPECS = 65_536
_MAX_IGNORED_TERMINALS = 65_536
_UTF8_CHUNK_CHARACTERS = 1024 * 1024


def _bounded_utf8_size(value: str, maximum: int) -> int | None:
    """Return the UTF-8 size, stopping once the caller's budget is exceeded."""

    total = 0
    for start in range(0, len(value), _UTF8_CHUNK_CHARACTERS):
        chunk = value[start : start + _UTF8_CHUNK_CHARACTERS]
        total += len(chunk.encode("utf-8"))
        if total > maximum:
            return None
    return total


def _source_part_size(value: str, remaining: int) -> int:
    try:
        size = _bounded_utf8_size(value, remaining)
    except UnicodeEncodeError as error:
        raise CompileError(
            f"grammar source is not valid UTF-8: {error}",
        ) from error
    if size is None:
        raise CompileError(
            f"aggregate grammar source must contain at most {_MAX_SOURCE_BYTES} bytes",
        )
    return size


def _terminal_pattern_size(
    value: str,
    source_remaining: int,
    regex_remaining: int,
) -> int:
    maximum = min(source_remaining, regex_remaining)
    try:
        size = _bounded_utf8_size(value, maximum)
    except UnicodeEncodeError as error:
        raise CompileError(
            f"terminal regex source is not valid UTF-8: {error}",
        ) from error
    if size is not None:
        return size
    if source_remaining <= regex_remaining:
        raise CompileError(
            f"aggregate grammar source must contain at most {_MAX_SOURCE_BYTES} bytes",
        )
    raise CompileError(
        f"terminal regexes must contain at most {_MAX_REGEX_BYTES} source bytes",
    )


@dataclass(frozen=True, slots=True)
class Terminal:
    name: str
    pattern: str
    priority: int = 0

    def __post_init__(self) -> None:
        runtime_name = cast(Any, self.name)
        runtime_pattern = cast(Any, self.pattern)
        if not isinstance(runtime_name, str) or not _RULE.fullmatch(runtime_name):
            raise ConfigurationError(
                "terminal name must be an ASCII grammar identifier",
            )
        if not isinstance(runtime_pattern, str):
            raise ConfigurationError(
                "terminal pattern must be a string",
            )
        if type(self.priority) is not int or not 0 <= self.priority <= 0xFFFF_FFFF:
            raise ConfigurationError(
                "terminal priority must be an unsigned 32-bit integer",
            )


@dataclass(frozen=True, slots=True)
class TokenizerManifest:
    """Exact token-local bytes plus explicit EOS token IDs."""

    token_bytes: tuple[bytes, ...]
    eos_token_ids: frozenset[int]

    def __post_init__(self) -> None:
        if not self.token_bytes:
            raise TokenizerCompatibilityError(
                "the tokenizer manifest is empty",
            )
        if len(self.token_bytes) > _MAX_VOCAB_SIZE:
            raise TokenizerCompatibilityError(
                f"the tokenizer manifest must contain at most {_MAX_VOCAB_SIZE} entries",
            )
        if not self.eos_token_ids:
            raise TokenizerCompatibilityError(
                "at least one EOS token ID is required",
            )
        if len(self.eos_token_ids) > _MAX_VOCAB_SIZE:
            raise TokenizerCompatibilityError(
                f"EOS token IDs must contain at most {_MAX_VOCAB_SIZE} entries",
            )
        size = len(self.token_bytes)
        for token_id in self.eos_token_ids:
            if type(token_id) is not int or token_id < 0 or token_id >= size:
                raise TokenizerCompatibilityError(
                    "an EOS token ID is outside the manifest vocabulary",
                    token=token_id if type(token_id) is int else None,
                )
        if len(self.eos_token_ids) == size:
            raise TokenizerCompatibilityError(
                "the tokenizer manifest must contain an ordinary token",
            )
        token_bytes = 0
        for token_id, value in enumerate(self.token_bytes):
            runtime_value = cast(Any, value)
            if not isinstance(runtime_value, bytes):
                raise TokenizerCompatibilityError(
                    "every token entry must be exact bytes",
                    token=token_id,
                )
            if token_id not in self.eos_token_ids and not value:
                raise TokenizerCompatibilityError(
                    "ordinary token bytes cannot be empty",
                    token=token_id,
                )
            if token_id not in self.eos_token_ids:
                token_bytes += len(value)
                if token_bytes > _MAX_TOKEN_BYTES:
                    raise TokenizerCompatibilityError(
                        "ordinary token bytes must contain at most "
                        f"{_MAX_TOKEN_BYTES} bytes in aggregate",
                    )

    @classmethod
    def from_transformers(
        cls,
        tokenizer: object,
        *,
        eos_token_ids: Sequence[int],
    ) -> TokenizerManifest:
        ids: list[int] = []
        seen: set[int] = set()
        for token_id in eos_token_ids:
            if len(ids) >= _MAX_VOCAB_SIZE:
                raise TokenizerCompatibilityError(
                    f"EOS token IDs must contain at most {_MAX_VOCAB_SIZE} entries",
                )
            if type(token_id) is not int or not 0 <= token_id <= 0xFFFF_FFFF:
                raise TokenizerCompatibilityError(
                    "EOS token IDs must be unsigned 32-bit integers",
                )
            if token_id in seen:
                raise TokenizerCompatibilityError(
                    "EOS token IDs must not contain duplicates",
                )
            ids.append(token_id)
            seen.add(token_id)
        if not ids:
            raise TokenizerCompatibilityError(
                "at least one EOS token ID is required",
            )

        backend = getattr(tokenizer, "backend_tokenizer", None)
        to_str = getattr(backend, "to_str", None)
        if not callable(to_str):
            raise TokenizerCompatibilityError(
                "a fast tokenizer with backend_tokenizer.to_str() is required",
            )
        try:
            serialized = cast(Callable[[], object], to_str)()
        except Exception as error:
            raise TokenizerCompatibilityError(
                f"could not serialize the fast tokenizer backend: {error}",
            ) from error
        if not isinstance(serialized, str):
            raise TokenizerCompatibilityError(
                "backend_tokenizer.to_str() did not return tokenizer JSON",
            )
        try:
            json_bytes = _bounded_utf8_size(serialized, _MAX_JSON_BYTES)
        except UnicodeEncodeError as error:
            raise TokenizerCompatibilityError(
                f"tokenizer JSON is not valid UTF-8: {error}",
            ) from error
        if json_bytes is None:
            raise TokenizerCompatibilityError(
                f"tokenizer JSON must contain at most {_MAX_JSON_BYTES} bytes",
            )
        try:
            extractor = cast(
                Callable[[str, list[int]], list[bytes]],
                getattr(_native, "_token_bytes_from_json"),
            )
            token_bytes = extractor(serialized, ids)
        except _native.NativeError as error:
            raise from_native(error) from error
        return cls(tuple(token_bytes), frozenset(ids))


class CompiledGrammar:
    """One compiled grammar which can create one owned generation session."""

    def __init__(
        self,
        native: _native.NativeGrammar,
        eos_token_ids: frozenset[int],
    ) -> None:
        self._native = native
        self.eos_token_ids = eos_token_ids
        self.vocab_size = native.vocab_size

    def _new_batch(
        self,
        rows: int,
        pad_token_id: int | None,
    ) -> _native.NativeBatch:
        try:
            return self._native.new_batch(rows, pad_token_id)
        except _native.NativeError as error:
            raise from_native(error) from error

    def logits_processor(
        self,
        prompt: object,
        *,
        pad_token_id: int | None = None,
        _masker: Callable[[object, bytes, int, int], object] | None = None,
        _trusted_fixed_append: bool = False,
    ) -> GreatGrammaLogitsProcessor:
        from .transformers import GreatGrammaLogitsProcessor

        if pad_token_id is not None and (
            type(pad_token_id) is not int
            or pad_token_id < 0
            or pad_token_id >= self.vocab_size
        ):
            raise ConfigurationError(
                "pad_token_id must be inside the compiled vocabulary",
                token=pad_token_id if type(pad_token_id) is int else None,
            )

        return GreatGrammaLogitsProcessor(
            self,
            prompt,
            pad_token_id=pad_token_id,
            _masker=_masker,
            _trusted_fixed_append=_trusted_fixed_append,
        )

    def generate(
        self,
        model: object,
        input_ids: object,
        *,
        generation_config: object,
        logits_processors: Sequence[object] = (),
        assistant_model: object | None = None,
    ) -> Any:
        from .transformers import generate

        return generate(
            self,
            model,
            input_ids,
            generation_config=generation_config,
            logits_processors=logits_processors,
            assistant_model=assistant_model,
        )


_RULE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def _grammar_text(grammar: str | os.PathLike[str]) -> tuple[str, int]:
    if isinstance(grammar, str):
        source = grammar
    else:
        path = Path(grammar)
        try:
            with path.open("rb") as stream:
                source_bytes = stream.read(_MAX_SOURCE_BYTES + 1)
            if len(source_bytes) > _MAX_SOURCE_BYTES:
                raise CompileError(
                    "aggregate grammar source must contain at most "
                    f"{_MAX_SOURCE_BYTES} bytes",
                )
            source = source_bytes.decode("utf-8")
        except (OSError, UnicodeError) as error:
            raise CompileError(
                f"could not read grammar path: {error}",
            ) from error
    return source, _source_part_size(source, _MAX_SOURCE_BYTES)


def _with_start_rule(source: str, start_rule: str) -> tuple[str, int]:
    runtime_start_rule = cast(Any, start_rule)
    if not isinstance(runtime_start_rule, str) or not _RULE.fullmatch(runtime_start_rule):
        raise ConfigurationError(
            "start_rule must be an ASCII grammar identifier",
        )
    declared: list[str] = []
    for line in source.splitlines():
        fields = line.strip().split()
        if fields and fields[0] == "%start":
            if len(fields) != 2:
                raise CompileError(
                    "the %start declaration must contain exactly one rule",
                )
            declared.append(fields[1])
    if len(declared) > 1:
        raise CompileError(
            "the grammar contains multiple %start declarations",
        )
    if declared and declared[0] != start_rule:
        raise ConfigurationError(
            f"grammar declares start rule {declared[0]!r}, not {start_rule!r}",
        )
    if declared:
        return source, 0
    prefix = f"%start {start_rule}\n"
    return prefix + source, len(prefix)


def compile(
    grammar: str | os.PathLike[str],
    *,
    start_rule: str,
    terminals: Sequence[Terminal],
    tokenizer: TokenizerManifest,
    ignored_terminals: Sequence[str] = (),
) -> CompiledGrammar:
    runtime_tokenizer = cast(Any, tokenizer)
    runtime_terminals = cast(Any, terminals)
    runtime_ignored = cast(Any, ignored_terminals)
    if not isinstance(runtime_tokenizer, TokenizerManifest):
        raise ConfigurationError(
            "tokenizer must be a TokenizerManifest",
        )
    if isinstance(runtime_terminals, (str, bytes)):
        raise ConfigurationError(
            "terminals must be a sequence of Terminal values",
        )
    if isinstance(runtime_ignored, (str, bytes)):
        raise ConfigurationError(
            "ignored_terminals must be a sequence of names",
        )
    source, source_bytes = _grammar_text(grammar)
    source, prefix_bytes = _with_start_rule(source, start_rule)
    source_bytes += prefix_bytes
    if source_bytes > _MAX_SOURCE_BYTES:
        raise CompileError(
            f"aggregate grammar source must contain at most {_MAX_SOURCE_BYTES} bytes",
        )

    terminal_specs: list[tuple[str, str, int]] = []
    regex_bytes = 0
    for terminal in terminals:
        if len(terminal_specs) >= _MAX_TERMINAL_SPECS:
            raise CompileError(
                f"grammar must contain at most {_MAX_TERMINAL_SPECS} terminal specifications",
            )
        if not isinstance(cast(Any, terminal), Terminal):
            raise ConfigurationError(
                "terminals must contain only Terminal values",
            )
        source_bytes += _source_part_size(
            terminal.name,
            _MAX_SOURCE_BYTES - source_bytes,
        )
        pattern_bytes = _terminal_pattern_size(
            terminal.pattern,
            _MAX_SOURCE_BYTES - source_bytes,
            _MAX_REGEX_BYTES - regex_bytes,
        )
        source_bytes += pattern_bytes
        regex_bytes += pattern_bytes
        terminal_specs.append((terminal.name, terminal.pattern, terminal.priority))

    ignored_specs: list[str] = []
    for name in ignored_terminals:
        if len(ignored_specs) >= _MAX_IGNORED_TERMINALS:
            raise CompileError(
                f"grammar must contain at most {_MAX_IGNORED_TERMINALS} ignored terminals",
            )
        if not isinstance(cast(Any, name), str) or not _RULE.fullmatch(name):
            raise ConfigurationError(
                "ignored terminal names must be ASCII grammar identifiers",
            )
        source_bytes += _source_part_size(name, _MAX_SOURCE_BYTES - source_bytes)
        ignored_specs.append(name)

    tokens: list[bytes | None] = [
        None if token_id in tokenizer.eos_token_ids else value
        for token_id, value in enumerate(tokenizer.token_bytes)
    ]
    try:
        native_compile = cast(
            Callable[
                [str, list[tuple[str, str, int]], list[bytes | None], list[str]],
                _native.NativeGrammar,
            ],
            getattr(_native, "_compile_yacc"),
        )
        native = native_compile(
            source,
            terminal_specs,
            tokens,
            ignored_specs,
        )
    except _native.NativeError as error:
        raise from_native(error) from error
    return CompiledGrammar(native, tokenizer.eos_token_ids)
