//! Memory location operations

use crate::core::types::Exp;

/// Represents a memory range
#[derive(Debug, Clone, PartialEq)]
pub struct MemRange {
    pub pos: Exp,
    pub size: Exp,
}

impl MemRange {
    pub fn new(pos: Exp, size: Exp) -> Self {
        Self { pos, size }
    }
}
