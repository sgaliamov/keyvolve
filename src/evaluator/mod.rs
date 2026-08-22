mod config;
pub mod corpus;
pub mod evaluator;
pub mod keys;
pub mod penalty;

pub use config::*;
pub use corpus::*;
pub use evaluator::*;

/// Placeholder char for empty/non-alpha genome slots.
pub const EMPTY_SLOT: char = '`';
