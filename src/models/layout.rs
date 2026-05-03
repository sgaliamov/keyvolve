use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    fmt,
    fs::File,
    io::{self, BufRead},
    path::Path,
};

pub type Keys = FxHashMap<char, u8>;

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

    pub fn load(path: impl AsRef<Path>) -> Vec<Layout> {
        let path = path.as_ref();
        let Ok(file) = File::open(path) else {
            return Vec::new();
        };

        io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !is_header(line))
            .map(|line| Layout::new(line.trim()))
            .collect_vec()
    }
}

impl fmt::Display for Layout {
    /// Reconstruct semicolon-separated layout string (positions 0–14 left; 15–29 right, stored inner→outer per group).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut slots = ['_'; 30];
        for (&ch, &pos) in &self.keys {
            slots[pos as usize] = ch;
        }
        let left = slots[..15]
            .chunks(5)
            .map(|c| c.iter().collect::<String>())
            .join(";");
        let right = slots[15..]
            .chunks(5)
            .map(|c| c.iter().rev().collect::<String>())
            .join(";");
        write!(f, "{left};{right}")
    }
}

/// Detect persisted CSV header row.
fn is_header(line: &str) -> bool {
    line.starts_with("keys_1;keys_2;keys_3;keys_4;keys_5;keys_6;")
}

pub fn line_to_keys(line: &str) -> Keys {
    let parts = line.split(';');

    let left = parts
        .clone()
        .take(3)
        .flat_map(|part| part.chars())
        .enumerate()
        .map(|(p, c)| (c, p as u8))
        .collect_vec();
    let len = left.len();

    parts
        .skip(3)
        .take(3)
        .flat_map(|part| part.chars().rev())
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
        let line = "zydpx;ralem;vbjuq;whtc_;fnosi;kg___;not used tail";
        let keys = line_to_keys(line);

        assert_eq!(keys.len(), 26);
        assert_eq!(keys[&'z'], 0);
        assert_eq!(keys[&'x'], 4);
        assert_eq!(keys[&'q'], 14);
        assert_eq!(keys[&'w'], 19);
        assert_eq!(keys[&'c'], 16);
        assert_eq!(keys[&'g'], 28);
    }

    #[test]
    fn test_name() {
        let line = "zydpx;ralem;vbjuq;whtc_;fnosi;kg___;not used tail";
        let layout = Layout::new(line);

        assert_eq!(layout.to_string(), "zydpx;ralem;vbjuq;whtc_;fnosi;kg___");
    }
}
