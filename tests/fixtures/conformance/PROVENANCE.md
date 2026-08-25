# Boundary/EOS conformance fixture provenance

This is a constructed fixture, not copied implementation data. It records the
Phase 1 normalized-input contract agreed for this repository:

- model tokens are exact byte strings or explicit EOS entries;
- a byte which closes an accepted lexeme is reconsumed from the DFA start;
- EOS flushes an accepted residual before parser EOF;
- EOS rejects an unfinished residual;
- every EOS vocabulary alias has identical semantics;
- NUL (`00`) and non-UTF-8 (`ff`) are ordinary bytes.

The parser language is exactly `ITEM CLOSE`. The lexer recognizes `i` as
`ITEM`, `c` as `CLOSE`, `u i` as `ITEM`, and the single bytes `00` and `ff` as
`ITEM`. Token 2 is `i c`, so its `c` byte is the corrected boundary witness.

Expected observations were derived by the independent interpreter in
`tests/conformance/oracle.rs`, which operates directly on its own raw byte-DFA
and LALR table representation. The paper is contextual material, not an
executable oracle. The conformance adapter separately constructs the same
normalized tables through `greatgramma-core` and compares public behavior.
