macro_rules! define_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            /// Creates an ID whose table membership is checked by its consuming API.
            #[must_use]
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            /// Returns the underlying integer without treating it as a table index.
            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

define_id!(
    TokenId,
    "Identifies an entry in the normalized token table."
);
define_id!(
    DfaStateId,
    "Identifies a state in the normalized lexer DFA."
);
define_id!(
    TerminalId,
    "Identifies a terminal in the normalized LALR table."
);
define_id!(
    ParserStateId,
    "Identifies a state in the normalized LALR table."
);
define_id!(
    NonterminalId,
    "Identifies a nonterminal in the normalized LALR table."
);
define_id!(
    ProductionId,
    "Identifies a production in the normalized LALR table."
);
define_id!(
    SequenceId,
    "Identifies an interned realizable terminal sequence head."
);
