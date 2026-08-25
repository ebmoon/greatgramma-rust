# Error contract

GreatGramma fails closed: malformed compilation inputs, unsupported tokenizer
semantics, discontinuous generation state, illegal tokens, and internal
invariants produce errors rather than an unconstrained mask.

## Stable Python errors

For calls satisfying the published Python argument types, all GreatGramma
domain errors derive from `GreatGrammaError`, which derives from `RuntimeError`.
The `code` attribute is the stable machine-readable contract. The optional
`row`, `token`, and `position` attributes add context when it is known. Error
message text is diagnostic and may change. Ordinary Python protocol errors from
objects that do not satisfy the published argument types are not stable error
codes.

| Code | Python class | Meaning |
|---|---|---|
| `compile_error` | `CompileError` | The grammar, terminal specification, tokenizer table, limits, validation, or preparation could not produce a supported compiled grammar. |
| `tokenizer_compatibility` | `TokenizerCompatibilityError` | A token manifest or attempted tokenizer conversion does not satisfy the exact token-byte contract. |
| `configuration_error` | `ConfigurationError` | Terminal/start-rule input, prompt, score shape, padding, EOS, or generation settings are outside the supported integration profile. |
| `sequence_discontinuity` | `SequenceDiscontinuityError` | Rows changed identity/count, did not extend by exactly one token, a processor was reused or called concurrently, or another fixed-session invariant was broken. |
| `constraint_violation` | `ConstraintViolationError` | A committed token is outside the compiled vocabulary or is not legal in the current grammar state. |
| `no_valid_token` | `NoValidTokenError` | A live row has no legal next token. This is an error, never a request to decode unconstrained. |
| `sequence_completed` | `SequenceCompletedError` | A caller advanced an accepted row with a non-padding token, requested a mask for an accepted unpadded row, or all rows had completed. |
| `internal_error` | `InternalError` | A panic was caught at the native boundary, an invariant or allocation failed, native error metadata was malformed, or score-mask application failed unexpectedly. |

The exception tuple raised by `greatgramma._native.NativeError` is a private ABI.
Applications must catch the public exceptions exported from `greatgramma`, not
interpret the native tuple. Unknown or malformed native codes are converted to
`InternalError`.

`position` is the current fixed row width observed by the processor. It is a
token position, not a byte offset into the grammar or token text.

## State after an error

Compilation errors produce no reusable compiled object. Constraint,
completion, configuration, and sequence errors reject the operation without
silently relaxing the grammar. The core batch-transition methods compute every
row successor and successor mask before committing them. The native
advance-and-mask path uses that atomic operation, so a constraint or dead-end
failure neither partially advances successful rows nor commits the failing
successor.

An internal native failure poisons that native batch. A failure while applying
the Python score mask poisons the `GreatGrammaLogitsProcessor`. Later calls then
raise `internal_error`; create a newly compiled grammar and a new processor
rather than attempting to resume. Each `CompiledGrammar` owns one native
generation session, so it cannot be used to create a replacement processor
after that session has been taken.

## Rust error layers

The Rust APIs retain more detailed typed errors:

- `greatgramma_compile::CompileError` covers grammar parsing, restricted regex
  compilation, tokenizer ingestion, table construction, validation, resource
  limits, and preparation.
- `greatgramma_core::ValidationError` rejects malformed or over-limit normalized
  tables before they become `ValidatedGrammar`.
- `greatgramma_core::PreparationError` rejects over-limit or inconsistent
  precomputation before it becomes `PreparedGrammar`.
- `greatgramma_core::EngineError` covers runtime row/token misuse, dead ends,
  completion, allocation failure, and corrupt prepared data.

These Rust enum variants are source-level APIs in version 0.1.0; the Python
codes above are the compatibility boundary intended for applications.

## Operational response

Treat `compile_error` and `tokenizer_compatibility` as deployment/configuration
failures. Treat `configuration_error` and `sequence_discontinuity` as adapter
integration failures. Treat `constraint_violation`, `no_valid_token`, and
`sequence_completed` as request/session failures. Treat `internal_error` as a
non-recoverable session failure and retain its causal exception for diagnosis.
