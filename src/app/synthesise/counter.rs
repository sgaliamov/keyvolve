use rustc_hash::FxHashMap;
use std::io::BufRead;

/// Count all `a-z` digraph pairs from a buffered reader, skipping cross-whitespace pairs.
pub fn count_bigrams(reader: impl BufRead) -> FxHashMap<[char; 2], u64> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn digraphs_counts_pairs_and_breaks_on_whitespace() {
        // basic pairs, case folding, space as separator, ab ≠ ba
        let counts = count_bigrams(Cursor::new("ab BC ba ba aa"));
        assert_eq!(counts[&['a', 'b']], 1);
        assert_eq!(counts[&['b', 'c']], 1);
        assert_eq!(counts[&['b', 'a']], 2);
        assert_eq!(counts[&['a', 'a']], 1);
        // space breaks chain → no repeated cross-space pair
        assert!(!counts.contains_key(&['b', 'b']));
    }

    #[test]
    fn digraphs_boundary_and_punctuation_break_chain() {
        // line boundary
        let counts = count_bigrams(Cursor::new("ab\nbc"));
        assert_eq!(counts[&['a', 'b']], 1);
        assert_eq!(counts[&['b', 'c']], 1);
        assert!(!counts.contains_key(&['b', 'b']));

        // punctuation
        let counts = count_bigrams(Cursor::new("a.b"));
        assert!(counts.is_empty());

        // empty
        assert!(count_bigrams(Cursor::new("")).is_empty());
    }
}
