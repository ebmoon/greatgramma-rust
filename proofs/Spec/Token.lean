import Spec.Types

namespace Greatgramma.Spec

inductive TokenEntry where
  | bytes (value : List Byte)
  | eos
deriving DecidableEq, Repr

abbrev TokenTable := List TokenEntry
abbrev TokenHistory := List TokenId

def token? (tokens : TokenTable) (token : TokenId) : Option TokenEntry :=
  tokens[token.val]?

def ordinaryBytes? (tokens : TokenTable) (token : TokenId) : Option (List Byte) :=
  match token? tokens token with
  | some (.bytes bytes) => some bytes
  | _ => none

def isEos (tokens : TokenTable) (token : TokenId) : Prop :=
  token? tokens token = some .eos

def isOrdinary (tokens : TokenTable) (token : TokenId) : Prop :=
  ∃ bytes, token? tokens token = some (.bytes bytes)

/-!
This is exactly the token-shape information established by validation.  It
allows duplicate EOS IDs, but every ordinary token carries at least one byte.
-/
structure TokenTableWF (tokens : TokenTable) : Prop where
  nonempty : tokens ≠ []
  hasOrdinary : ∃ token bytes, token? tokens token = some (.bytes bytes)
  hasEos : ∃ token, token? tokens token = some .eos
  ordinaryNonempty :
    ∀ token bytes, token? tokens token = some (.bytes bytes) → bytes ≠ []

def decodeOrdinaryHistory? (tokens : TokenTable) : TokenHistory → Option (List Byte)
  | [] => some []
  | token :: rest => do
      let bytes ← ordinaryBytes? tokens token
      let suffix ← decodeOrdinaryHistory? tokens rest
      pure (bytes ++ suffix)

def HistoryDecodesTo (tokens : TokenTable) (history : TokenHistory)
    (bytes : List Byte) : Prop :=
  decodeOrdinaryHistory? tokens history = some bytes

def NormalizedHistory (tokens : TokenTable) (history : TokenHistory) : Prop :=
  ∃ bytes, HistoryDecodesTo tokens history bytes

@[simp] theorem decodeOrdinaryHistory_nil (tokens : TokenTable) :
    decodeOrdinaryHistory? tokens [] = some [] := rfl

@[simp] theorem decodeOrdinaryHistory_cons (tokens : TokenTable) (token : TokenId)
    (history : TokenHistory) :
    decodeOrdinaryHistory? tokens (token :: history) = (do
      let bytes ← ordinaryBytes? tokens token
      let suffix ← decodeOrdinaryHistory? tokens history
      pure (bytes ++ suffix)) := rfl

theorem eos_not_ordinary {tokens : TokenTable} {token : TokenId}
    (h : isEos tokens token) : ordinaryBytes? tokens token = none := by
  simp only [isEos, ordinaryBytes?, token?] at h ⊢
  split <;> rename_i entry hentry
  · cases entry <;> simp_all
  · simp

end Greatgramma.Spec
