pub mod evaluate;
pub mod frequencies;
mod layout_evaluator;
pub mod merge;
mod optimization;
mod output;
pub mod rank;
mod run;
pub mod synthesise;

pub use evaluate::*;
pub use frequencies::*;
pub use layout_evaluator::*;
pub use merge::*;
pub use optimization::*;
pub use output::*;
pub use rank::*;
pub use run::*;

/// Placeholder char for empty/non-alpha genome slots.
pub const EMPTY_SLOT: char = '`';
