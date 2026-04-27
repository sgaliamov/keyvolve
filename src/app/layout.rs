use std::{
    fs::File,
    io::{self, BufRead},
    path::Path,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;

pub type Keys = FxHashMap<char, u8>;

pub struct Layout {
    pub keys: Keys,
}

impl Layout {
    pub fn load(path: impl AsRef<Path>) -> Vec<Layout> {
        let path = path.as_ref();
        let Ok(file) = File::open(path) else {
            return Vec::new();
        };

        io::BufReader::new(file)
            .lines()
            .map(|x| {
                let line = x.unwrap();
                let keys = line_to_keys(&line);

                Layout { keys }
            })
            .collect_vec()
    }
}

fn line_to_keys(line: &str) -> Keys {
    let parts = line.split(';').collect_vec();
    let line = parts[0];
    let parts = line.split_whitespace().collect_vec();
    let left = parts
        .iter()
        .take(3)
        .flat_map(|part| part.chars())
        .enumerate()
        .map(|(p, c)| (c, p as u8));

    parts
        .iter()
        .skip(3)
        .flat_map(|part| part.chars().rev())
        .enumerate()
        .map(|(p, c)| (c, p as u8 + 15_u8))
        .merge(left)
        .filter(|(c, _)| c != &'_')
        .collect()
}
