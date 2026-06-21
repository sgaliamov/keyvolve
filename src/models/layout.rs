use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    fmt,
    fs::File,
    io::{self, BufRead},
    path::Path,
};

pub type Keys = FxHashMap<char, u8>;

#[derive(Clone)]
pub struct Layout {
    pub keys: Keys,
}

impl Layout {
    // Constructor: Create Layout from line
    pub fn new(line: &str) -> Self {
        let keys = line_to_keys(line);
        Layout { keys }
    }

    pub fn from_keys(keys: &[char]) -> Self {
        let keys = keys
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_alphabetic())
            .map(|(i, &c)| (c, i as u8))
            .collect();
        Layout { keys }
    }

    /// Hand-swapped twin: every key reflected left↔right. Involution.
    /// Fitness is hand-symmetric, so a layout and its mirror score identically.
    pub fn mirrored(&self) -> Layout {
        let keys = self
            .keys
            .iter()
            .map(|(&c, &p)| (c, mirror_slot(p as usize) as u8))
            .collect();
        Layout { keys }
    }

    /// `true` when `a` sits on the left hand (slot 0–14); `false` if on the right
    /// or absent. Drives canonicalization to the `a`-left orientation on save.
    pub fn a_is_left(&self) -> bool {
        self.keys.get(&'a').is_some_and(|&p| p < 15)
    }

    pub fn load(path: impl AsRef<Path>) -> Vec<Layout> {
        let path = path.as_ref();
        let Ok(file) = File::open(path) else {
            return Vec::new();
        };

        let mut seen = rustc_hash::FxHashSet::default();
        io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !is_header(line))
            .filter(|line| seen.insert(line.splitn(7, ',').take(6).collect::<String>()))
            .map(|line| Layout::new(line.trim()))
            .collect_vec()
    }

    /// 30-slot character array; `_` marks an empty slot. Index = physical key position.
    fn slots(&self) -> [char; 30] {
        let mut slots = ['_'; 30];
        for (&ch, &pos) in &self.keys {
            slots[pos as usize] = ch;
        }
        slots
    }
}

impl fmt::Display for Layout {
    /// Reconstruct comma-separated layout string (positions 0–14 left; 15–29 right, stored inner→outer per group).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slots = self.slots();
        let left = slots[..15]
            .chunks(5)
            .map(|c| c.iter().collect::<String>())
            .join(", ");
        let right = slots[15..]
            .chunks(5)
            .map(|c| c.iter().collect::<String>())
            .join(", ");
        write!(f, "{left}, {right}")
    }
}

/// Detect persisted CSV header row.
fn is_header(line: &str) -> bool {
    line.starts_with("keys_1,")
}

/// Hand-swap reflection of a slot index (0–29). Involution: `mirror_slot(mirror_slot(i)) == i`.
/// Left col k (slots 0–14) ↔ right col 4-k (slots 15–29), same row.
fn mirror_slot(i: usize) -> usize {
    if i < 15 {
        (i / 5) * 5 + (4 - i % 5) + 15
    } else {
        let r = i - 15;
        (r / 5) * 5 + (4 - r % 5)
    }
}

pub fn line_to_keys(line: &str) -> Keys {
    let parts = line.split(',');
    let left = parts
        .clone()
        .take(3)
        .flat_map(|part| part.trim().chars())
        .enumerate()
        .map(|(p, c)| (c, p as u8))
        .collect_vec();
    let len = left.len();

    parts
        .skip(3)
        .take(3)
        .flat_map(|part| part.trim().chars())
        .enumerate()
        .map(|(p, c)| (c, (p + len) as u8))
        .merge(left)
        .filter(|(c, _)| c.is_alphabetic())
        .collect()
}

#[cfg(test)]
mod layout_test {
    use super::*;

    #[test]
    fn test_line_to_keys_basic() {
        let line = "zydpx, ralem, vbjuq, whtc_, fnosi, kg___, not used tail";
        let keys = line_to_keys(line);

        assert_eq!(keys.len(), 26);
        assert_eq!(keys[&'z'], 0);
        assert_eq!(keys[&'x'], 4);
        assert_eq!(keys[&'q'], 14);
        assert_eq!(keys[&'w'], 15);
        assert_eq!(keys[&'c'], 18);
        assert_eq!(keys[&'g'], 26);
    }

    #[test]
    fn test_name() {
        let line = "zydpx, ralem, vbjuq, whtc_, fnosi,kg___,not used tail";
        let layout = Layout::new(line);

        assert_eq!(
            layout.to_string(),
            "zydpx, ralem, vbjuq, whtc_, fnosi, kg___"
        );
    }

    #[test]
    fn mirrored_is_an_involution() {
        let layout = Layout::new("zydpx, ralem, vbjuq, whtc_, fnosi, kg___");

        assert_eq!(layout.mirrored().mirrored().to_string(), layout.to_string());
    }

    #[test]
    fn mirrored_swaps_a_hand() {
        // `a` at slot 6 (left); mirroring moves it to the right hand.
        let layout = Layout::new("zydpx, ralem, vbjuq, whtc_, fnosi, kg___");

        assert!(layout.a_is_left());
        assert!(!layout.mirrored().a_is_left());
    }
}
