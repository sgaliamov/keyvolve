mod evaluate;
mod layout_evaluator;
pub mod merge;
mod optimization;
mod run;
pub mod synthesise;

pub use evaluate::*;
pub use layout_evaluator::*;
pub use merge::*;
pub use optimization::*;
pub use run::*;

/// Placeholder char for empty/non-alpha genome slots.
pub const EMPTY_SLOT: char = '`';
