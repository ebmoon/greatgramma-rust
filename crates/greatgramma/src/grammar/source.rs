/// One named lexer terminal compiled as an anchored byte regular expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSpec {
    pub(crate) name: String,
    pub(crate) pattern: String,
    pub(crate) priority: u32,
}

impl TerminalSpec {
    #[must_use]
    pub fn new(name: impl Into<String>, pattern: impl Into<String>, priority: u32) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            priority,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    #[must_use]
    pub const fn priority(&self) -> u32 {
        self.priority
    }
}

/// Owned compiler input. The grammar surface is action-free original Yacc.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceGrammar {
    pub(crate) yacc: String,
    pub(crate) terminals: Vec<TerminalSpec>,
    pub(crate) ignored_terminals: Vec<String>,
}

impl SourceGrammar {
    #[must_use]
    pub fn new(yacc: impl Into<String>, terminals: Vec<TerminalSpec>) -> Self {
        Self {
            yacc: yacc.into(),
            terminals,
            ignored_terminals: Vec::new(),
        }
    }

    /// Marks lexer terminals as parser no-ops. They must be declared but not
    /// referenced by a grammar production.
    #[must_use]
    pub fn with_ignored_terminals(mut self, terminals: Vec<String>) -> Self {
        self.ignored_terminals = terminals;
        self
    }

    #[must_use]
    pub fn yacc(&self) -> &str {
        &self.yacc
    }

    #[must_use]
    pub fn terminals(&self) -> &[TerminalSpec] {
        &self.terminals
    }

    #[must_use]
    pub fn ignored_terminals(&self) -> &[String] {
        &self.ignored_terminals
    }
}
