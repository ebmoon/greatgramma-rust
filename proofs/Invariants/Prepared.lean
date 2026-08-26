import Invariants.Validated

open Aeneas Aeneas.Std

namespace Greatgramma.Invariants

/-!
Compatibility interface for derived tables.  U5 and U6 will prove the stronger
extensional representation clauses; U3 fixes the dimensions and ownership
conditions that every later theorem may rely on.
-/

structure PreparedWF
    (prepared : GreatgrammaCore.engine.PreparedGrammar) : Prop where
  grammar : ValidatedWF prepared.grammar
  tokenSourceRows :
    prepared.spanner.token_table.rows.val.length =
      prepared.grammar.lexer.state_count.val + 1
  tokenColumns :
    ∀ row ∈ prepared.spanner.token_table.rows.val,
      row.val.length = prepared.grammar.token_count.val
  inverseSourceRows :
    prepared.spanner.inverse.val.length =
      prepared.grammar.lexer.state_count.val + 1
  parserStateRows :
    prepared.parser.rows.val.length =
      prepared.grammar.lalr.dimensions.state_count.val
  parserSequenceColumns :
    ∀ row ∈ prepared.parser.rows.val,
      row.val.length = prepared.spanner.sequences.sequences.val.length
  maskBytes :
    prepared.mask_bytes.val = (prepared.grammar.token_count.val + 7) / 8

end Greatgramma.Invariants
