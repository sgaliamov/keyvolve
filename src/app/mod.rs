pub mod evaluate;
mod evaluator;
pub mod frequencies;
pub mod merge;
mod optimization;
mod output;
pub mod rank;
mod run;
pub mod synthesise;

pub use evaluate::*;
pub use evaluator::*;
pub use frequencies::*;
pub use merge::*;
pub use optimization::*;
pub use output::*;
pub use rank::*;
pub use run::*;

/// Placeholder char for empty/non-alpha genome slots.
pub const EMPTY_SLOT: char = '`';
