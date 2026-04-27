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
    pub name: String,
}

impl Layout {
    // Constructor: Create Layout from line
    pub fn new(line: &str) -> Self {
        let keys = line_to_keys(line);
        let name = line.split(';').take(6).join(";");
        Layout { keys, name }
    }

    pub fn load(path: impl AsRef<Path>) -> Vec<Layout> {
        let path = path.as_ref();
        let Ok(file) = File::open(path) else {
            return Vec::new();
        };

        io::BufReader::new(file)
            .lines()
            .map(|x| Layout::new(&x.unwrap()))
            .collect_vec()
    }
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

        assert_eq!(layout.name, "zydpx;ralem;vbjuq;whtc_;fnosi;kg___");
    }
}
