use crate::models::KeyPos;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
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

    pub fn from_keys(keys: &[KeyPos]) -> Self {
        let keys = keys.iter().map(|kp| (kp.0, kp.1)).collect();
        Layout { keys }
    }

    pub fn name(&self) -> String {
        let mut keyed: Vec<(char, u8)> = self.keys.iter().map(|(&ch, &pos)| (ch, pos)).collect();
        keyed.sort_unstable_by_key(|&(_, pos)| pos);
        keyed.into_iter().map(|(ch, _)| ch).collect()
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

/// Detect persisted CSV header row.
fn is_header(line: &str) -> bool {
    line.starts_with("keys_1;keys_2;keys_3;keys_4;keys_5;keys_6;")
}

fn line_to_keys(line: &str) -> Keys {
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
        .filter(|(c, _)| c != &'_')
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
        assert_eq!(keys[&'c'], 16);
        assert_eq!(keys[&'w'], 19);
        assert_eq!(keys[&'g'], 28);
    }

    #[test]
    fn test_name() {
        let line = "zydpx;ralem;vbjuq;whtc_;fnosi;kg___;not used tail";
        let layout = Layout::new(line);

        assert_eq!(layout.name(), "zydpx;ralem;vbjuq;whtc_;fnosi;kg___");
    }

    #[test]
    fn test_chars_by_position() {
        let line = "zydpx;ralem;vbjuq;whtc_;fnosi;kg___;not used tail";
        let keys = line_to_keys(line);
        let chars = chars_by_position(&keys);

        assert_eq!(chars.len(), 26);
        assert_eq!(chars[0], 'z');
        assert_eq!(chars[4], 'x');
        assert_eq!(chars[14], 'q');
        assert_eq!(chars[16], 'c');
        assert_eq!(chars[19], 'w');
        assert_eq!(chars[28], 'g');
    }
}
