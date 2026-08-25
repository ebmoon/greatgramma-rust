from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest
import greatgramma.compiler as compiler_module

from greatgramma import (
    CompiledGrammar,
    CompileError,
    ConfigurationError,
    Terminal,
    TokenizerCompatibilityError,
    TokenizerManifest,
    compile,
)
from greatgramma import _native


class _FakeNativeGrammar:
    vocab_size = 3


def test_compile_passes_exact_bytes_eos_and_explicit_start_rule(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, Any] = {}

    def fake_compile(
        yacc: str,
        terminals: list[tuple[str, str, int]],
        tokens: list[bytes | None],
        ignored: list[str],
    ) -> _FakeNativeGrammar:
        captured.update(
            yacc=yacc,
            terminals=terminals,
            tokens=tokens,
            ignored=ignored,
        )
        return _FakeNativeGrammar()

    monkeypatch.setattr(_native, "_compile_yacc", fake_compile)
    manifest = TokenizerManifest(
        token_bytes=(b"i", b"", b"\x00\xff"),
        eos_token_ids=frozenset((1,)),
    )

    result = compile(
        "%token ITEM\n%%\nS: ITEM;\n",
        start_rule="S",
        terminals=[Terminal("ITEM", "i", 4)],
        tokenizer=manifest,
        ignored_terminals=("SPACE",),
    )

    assert isinstance(result, CompiledGrammar)
    assert str(captured["yacc"]).startswith("%start S\n")
    assert captured["terminals"] == [("ITEM", "i", 4)]
    assert captured["tokens"] == [b"i", None, b"\x00\xff"]
    assert captured["ignored"] == ["SPACE"]


def test_manifest_rejects_an_all_eos_vocabulary() -> None:
    with pytest.raises(TokenizerCompatibilityError) as raised:
        TokenizerManifest(
            token_bytes=(b"", b""),
            eos_token_ids=frozenset((0, 1)),
        )

    assert raised.value.code == "tokenizer_compatibility"


def test_manifest_rejects_oversized_vocabulary_before_scanning_entries(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(compiler_module, "_MAX_VOCAB_SIZE", 2)

    with pytest.raises(TokenizerCompatibilityError) as raised:
        TokenizerManifest(
            token_bytes=(b"a", object(), b""),  # type: ignore[arg-type]
            eos_token_ids=frozenset((2,)),
        )

    assert raised.value.code == "tokenizer_compatibility"
    assert "at most 2" in str(raised.value)


def test_manifest_rejects_oversized_total_token_bytes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(compiler_module, "_MAX_TOKEN_BYTES", 3)

    with pytest.raises(TokenizerCompatibilityError) as raised:
        TokenizerManifest(
            token_bytes=(b"aa", b"bb", b""),
            eos_token_ids=frozenset((2,)),
        )

    assert raised.value.code == "tokenizer_compatibility"
    assert "at most 3 bytes" in str(raised.value)


def test_unsigned_terminal_priority_uses_a_stable_public_error() -> None:
    with pytest.raises(ConfigurationError) as raised:
        Terminal("ITEM", "i", -1)
    assert raised.value.code == "configuration_error"


def test_non_utf8_grammar_path_uses_the_compile_error_contract(tmp_path: Path) -> None:
    grammar = tmp_path / "grammar.y"
    grammar.write_bytes(b"\xff")

    with pytest.raises(CompileError) as raised:
        compile(
            grammar,
            start_rule="S",
            terminals=[Terminal("ITEM", "i")],
            tokenizer=TokenizerManifest((b"i", b""), frozenset((1,))),
        )

    assert raised.value.code == "compile_error"


def test_grammar_path_uses_one_bounded_binary_read(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    grammar = tmp_path / "grammar.y"
    grammar.write_text("ignored", encoding="utf-8")
    source_bytes = b"%token ITEM\n%%\nS: ITEM;\n"
    observed_sizes: list[int] = []

    class Reader:
        def __enter__(self) -> Reader:
            return self

        def __exit__(self, *_args: object) -> None:
            return None

        def read(self, size: int = -1) -> bytes:
            observed_sizes.append(size)
            return source_bytes

    def fake_open(
        _path: Path,
        mode: str = "r",
        *_args: object,
        **_kwargs: object,
    ) -> Reader:
        assert mode == "rb"
        return Reader()

    monkeypatch.setattr(Path, "open", fake_open)

    grammar_text: Any = getattr(compiler_module, "_grammar_text")
    max_source_bytes: int = getattr(compiler_module, "_MAX_SOURCE_BYTES")
    source, size = grammar_text(grammar)

    assert source == source_bytes.decode("utf-8")
    assert size == len(source_bytes)
    assert observed_sizes == [max_source_bytes + 1]


def test_from_transformers_fails_closed_without_decoding() -> None:
    class TokenizerDouble:
        def decode(self, _token: int) -> str:
            raise AssertionError("per-token decode must never be used")

    with pytest.raises(TokenizerCompatibilityError) as raised:
        TokenizerManifest.from_transformers(
            TokenizerDouble(),
            eos_token_ids=(2,),
        )

    assert raised.value.code == "tokenizer_compatibility"


def test_from_transformers_bounds_eos_collection_before_backend_serialization() -> None:
    class BackendDouble:
        def to_str(self) -> str:
            raise AssertionError("oversized EOS input must fail before serialization")

    class TokenizerDouble:
        backend_tokenizer = BackendDouble()

    with pytest.raises(TokenizerCompatibilityError) as raised:
        TokenizerManifest.from_transformers(
            TokenizerDouble(),
            eos_token_ids=range(1_000_001),
        )

    assert raised.value.code == "tokenizer_compatibility"
    assert "at most 1000000" in str(raised.value)


def test_from_transformers_bounds_utf8_json_before_native_conversion(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class BackendDouble:
        def to_str(self) -> str:
            return "\N{LATIN SMALL LETTER E WITH ACUTE}" * 3

    class TokenizerDouble:
        backend_tokenizer = BackendDouble()

    def fail_extract(_serialized: str, _eos_ids: list[int]) -> list[bytes]:
        pytest.fail("oversized JSON crossed the native boundary")

    monkeypatch.setattr(compiler_module, "_MAX_JSON_BYTES", 5)
    monkeypatch.setattr(_native, "_token_bytes_from_json", fail_extract)

    with pytest.raises(TokenizerCompatibilityError) as raised:
        TokenizerManifest.from_transformers(
            TokenizerDouble(),
            eos_token_ids=(1,),
        )

    assert raised.value.code == "tokenizer_compatibility"
    assert "at most 5 bytes" in str(raised.value)


def test_compile_bounds_aggregate_utf8_source_before_native_conversion(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_compile(
        _yacc: str,
        _terminals: list[tuple[str, str, int]],
        _tokens: list[bytes | None],
        _ignored: list[str],
    ) -> _FakeNativeGrammar:
        pytest.fail("oversized source crossed the native boundary")

    monkeypatch.setattr(compiler_module, "_MAX_SOURCE_BYTES", 38)
    monkeypatch.setattr(_native, "_compile_yacc", fail_compile)

    with pytest.raises(CompileError) as raised:
        compile(
            "%start S\n%token ITEM\n%%\nS: ITEM;\n",
            start_rule="S",
            terminals=[Terminal("ITEM", "\N{LATIN SMALL LETTER E WITH ACUTE}")],
            tokenizer=TokenizerManifest((b"i", b""), frozenset((1,))),
        )

    assert raised.value.code == "compile_error"
    assert "at most 38 bytes" in str(raised.value)


def test_compile_bounds_aggregate_regex_source_before_native_conversion(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_compile(
        _yacc: str,
        _terminals: list[tuple[str, str, int]],
        _tokens: list[bytes | None],
        _ignored: list[str],
    ) -> _FakeNativeGrammar:
        pytest.fail("oversized regex source crossed the native boundary")

    monkeypatch.setattr(compiler_module, "_MAX_REGEX_BYTES", 3)
    monkeypatch.setattr(_native, "_compile_yacc", fail_compile)

    with pytest.raises(CompileError) as raised:
        compile(
            "%start S\n%token ITEM\n%%\nS: ITEM;\n",
            start_rule="S",
            terminals=[Terminal("ITEM", "\N{LATIN SMALL LETTER E WITH ACUTE}" * 2)],
            tokenizer=TokenizerManifest((b"i", b""), frozenset((1,))),
        )

    assert raised.value.code == "compile_error"
    assert "at most 3 source bytes" in str(raised.value)


def test_from_transformers_uses_backend_json_without_per_token_decode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class BackendDouble:
        def to_str(self) -> str:
            return '{"decoder":{"type":"ByteLevel"}}'

    class TokenizerDouble:
        backend_tokenizer = BackendDouble()

        def decode(self, _token: int) -> str:
            raise AssertionError("per-token decode must never be used")

    captured: dict[str, object] = {}

    def fake_extract(serialized: str, eos_ids: list[int]) -> list[bytes]:
        captured.update(serialized=serialized, eos_ids=eos_ids)
        return [b"a", b""]

    monkeypatch.setattr(_native, "_token_bytes_from_json", fake_extract)
    manifest = TokenizerManifest.from_transformers(
        TokenizerDouble(),
        eos_token_ids=(1,),
    )

    assert manifest == TokenizerManifest((b"a", b""), frozenset((1,)))
    assert captured == {
        "serialized": '{"decoder":{"type":"ByteLevel"}}',
        "eos_ids": [1],
    }
