use ed_balance::Context;
use std::collections::{HashMap, HashSet};

// A keyboard has 30 positions (0-29), which fits comfortably in a u8,
// keeping the heap footprint small when storing millions of genomes.
pub type Position = u8;

/// All read-only data that drives the genetic algorithm for a single run.
/// Keeping everything in one struct avoids threading many individual
/// parameters through every generator / mutator / scorer function.
pub struct Behavior {
    /// GA hyper-parameters (population size, generation count, …) shared
    /// with the `ed_balance` runtime.
    pub context: Context,
    /// Corpus words used to evaluate typing effort; the entire text is
    /// pre-split into words once at load time to avoid repeated allocations
    /// during scoring.
    pub words: Vec<String>,

    /// Keys whose positions must never be changed during evolution (e.g. a
    /// thumb key that is physically fixed on the hardware).
    /// char * position
    pub frozen_keys: FrozenKeys,

    /// Physical key slots that are absent or unusable on the target board.
    /// Does not include positions of frozen keys (those are already occupied).
    pub blocked_keys: HashSet<Position>,
    /// Nested map: `efforts[from][to]` = ergonomic cost of pressing `to`
    /// right after `from` on the same hand.  A separate map per source
    /// position lets callers do O(1) look-ups without arithmetic.
    pub efforts: Efforts,
    /// Extra cost applied whenever typing crosses hands; penalises layouts
    /// that force frequent alternation in high-effort positions.
    pub switch_penalty: f64,
    /// Extra cost for pressing the same physical key twice in a row (bigram
    /// like "ll"), which is uncomfortable on some finger placements.
    pub same_key_penalty: f64,
}

/// `efforts[position_a][position_b]` = normalised cost of the bigram a→b.
pub type Efforts = HashMap<Position, HashMap<Position, f64>>;

/// Keys that are pinned to a fixed position and excluded from mutation.
pub type FrozenKeys = HashMap<char, Position>;
