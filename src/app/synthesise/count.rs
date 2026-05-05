use rustc_hash::FxHashMap;
use std::io::BufRead;

/// Count all `a-z` digraph pairs from a buffered reader, skipping cross-whitespace pairs.
pub fn count_digraphs(reader: impl BufRead) -> FxHashMap<[char; 2], u64> {
    let mut counts: FxHashMap<[char; 2], u64> = FxHashMap::default();
    let mut prev: Option<char> = None;

    for line in reader.lines().map_while(Result::ok) {
        for ch in line.chars() {
            if ch.is_ascii_alphabetic() {
                let lc = ch.to_ascii_lowercase();
                if let Some(p) = prev {
                    *counts.entry([p, lc]).or_insert(0) += 1;
                }
                prev = Some(lc);
            } else {
                // Whitespace or punctuation — break digraph chain.
                prev = None;
            }
        }
        // Line boundary = word boundary.
        prev = None;
    }

    counts
}
