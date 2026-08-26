import Aeneas

open Aeneas Aeneas.Std Result ControlFlow Error

/-! # External Rust types

The generated translation refers to Rust types that Aeneas does not model in
its standard library.  This module gives those names stable, reviewable Lean
definitions.  It is imported through the small generated-adjacent bridge
`GreatgrammaCore.TypesExternal`.
-/

/-- `TryReserveError` is deliberately observational: the verified core only
distinguishes allocation success from allocation failure and never inspects an
allocator-specific payload. -/
@[rust_type "alloc::collections::TryReserveError"]
inductive alloc.collections.TryReserveError where
  | modeled
deriving BEq, Repr
