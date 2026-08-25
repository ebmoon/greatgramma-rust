from __future__ import annotations

import json

from greatgramma import TokenizerManifest


def _bytelevel_piece(byte: int) -> str:
    direct = list(range(ord("!"), ord("~") + 1))
    direct += list(range(0xA1, 0xAD))
    direct += list(range(0xAE, 0x100))
    if byte in direct:
        return chr(byte)
    return chr(256 + sum(candidate not in direct for candidate in range(byte)))


def test_native_bytelevel_json_extraction_through_fast_tokenizer_facade() -> None:
    vocab = {_bytelevel_piece(byte): byte for byte in range(256)}
    vocab["🙂"] = 256
    vocab["<eos>"] = 257
    serialized = json.dumps(
        {
            "decoder": {"type": "ByteLevel"},
            "model": {"type": "BPE", "vocab": vocab},
            "added_tokens": [
                {"id": 257, "content": "<eos>", "special": True}
            ],
        }
    )

    class Backend:
        def to_str(self) -> str:
            return serialized

    class FastTokenizer:
        backend_tokenizer = Backend()

    manifest = TokenizerManifest.from_transformers(
        FastTokenizer(),
        eos_token_ids=(257,),
    )

    assert manifest.token_bytes[0] == b"\x00"
    assert manifest.token_bytes[255] == b"\xff"
    assert manifest.token_bytes[256] == "🙂".encode()
    assert manifest.token_bytes[257] == b""
    assert manifest.eos_token_ids == frozenset((257,))
