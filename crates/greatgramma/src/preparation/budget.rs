use std::{error::Error, fmt};

/// Coarse bounds for derived preparation data and the work used to build it.
///
/// Both limits are intentionally representation-independent. Optimized
/// backends may use different storage without changing this public contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationLimits {
    pub max_items: usize,
    pub max_work: usize,
}

impl Default for PreparationLimits {
    fn default() -> Self {
        Self {
            max_items: 64_000_000,
            max_work: 1_000_000_000,
        }
    }
}

/// Preparation failed before exposing a partial derived table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    TooLarge { required: usize, maximum: usize },
    AllocationFailure { requested: usize },
    InvariantViolation,
}

pub(crate) struct WorkBudget {
    used: usize,
    maximum: usize,
}

impl WorkBudget {
    pub(crate) const fn new(limits: PreparationLimits) -> Self {
        Self {
            used: 0,
            maximum: limits.max_work,
        }
    }

    pub(crate) fn charge(&mut self, amount: usize) -> Result<(), PreparationError> {
        self.used = check_limit(self.used.checked_add(amount), self.maximum)?;
        Ok(())
    }

    pub(crate) fn remaining(&self) -> usize {
        self.maximum.saturating_sub(self.used)
    }
}

pub(crate) fn check_items(
    required: Option<usize>,
    limits: PreparationLimits,
) -> Result<usize, PreparationError> {
    check_limit(required, limits.max_items)
}

pub(crate) fn prepared_vec<T>(capacity: usize) -> Result<Vec<T>, PreparationError> {
    reserved_vec(capacity).map_err(|requested| PreparationError::AllocationFailure { requested })
}

pub(crate) fn prepared_push<T>(values: &mut Vec<T>, value: T) -> Result<(), PreparationError> {
    reserve_one(values).map_err(|requested| PreparationError::AllocationFailure { requested })?;
    values.push(value);
    Ok(())
}

pub(crate) fn reserved_vec<T>(capacity: usize) -> Result<Vec<T>, usize> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| capacity)?;
    Ok(values)
}

pub(crate) fn reserve_one<T>(values: &mut Vec<T>) -> Result<(), usize> {
    if values.len() == values.capacity() {
        let requested = values.len().saturating_add(1);
        values.try_reserve(1).map_err(|_| requested)?;
    }
    Ok(())
}

fn check_limit(required: Option<usize>, maximum: usize) -> Result<usize, PreparationError> {
    let required = required.ok_or(PreparationError::TooLarge {
        required: usize::MAX,
        maximum,
    })?;
    if required > maximum {
        return Err(PreparationError::TooLarge { required, maximum });
    }
    Ok(required)
}

impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { required, maximum } => write!(
                f,
                "grammar preparation requires {required} items or work units, maximum is {maximum}"
            ),
            Self::AllocationFailure { requested } => {
                write!(
                    f,
                    "grammar preparation allocation failed for {requested} entries"
                )
            }
            Self::InvariantViolation => f.write_str("prepared grammar invariant violated"),
        }
    }
}

impl Error for PreparationError {}
