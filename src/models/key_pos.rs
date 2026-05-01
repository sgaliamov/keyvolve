use darwin::{Gene, Genome};
use serde::Deserialize;

use crate::models::ScoreResult;

/// Key position: (char, position index)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct KeyPos(pub char, pub u8);

impl Gene for KeyPos {
    fn to_f64(self) -> f64 {
        ((self.0 as u16) << 8 | self.1 as u16) as f64
    }
}

pub type KeysGenome = Genome<KeyPos>;

/// Individual in the GA population.
pub type KeysIndividual = darwin::Individual<KeyPos, ScoreResult>;

/// GA context for layout optimization.
pub type GaContext<'a> = darwin::Context<'a, KeyPos, crate::LayoutEvaluator, ScoreResult>;
