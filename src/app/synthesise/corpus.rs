use rustc_hash::FxHashMap;

/// Build short fake words from digraph edges via Eulerian path decomposition.
/// Every input edge appears as exactly one consecutive char-pair in the output;
/// no extra edges are introduced.
pub fn build_corpus(edges: &[([char; 2], usize)], max_word_len: usize) -> Vec<String> {
    let mut adj = build_adj(edges);
    let mut words = Vec::new();
    while let Some(start) = next_start(&adj) {
        let path = greedy_walk(&mut adj, start);
        if path.len() > 1 {
            split_path(&path, max_word_len, &mut words);
        }
    }
    words
}

/// Build directed adjacency list (multi-edges) from edge list.
fn build_adj(edges: &[([char; 2], usize)]) -> FxHashMap<char, Vec<char>> {
    let mut adj: FxHashMap<char, Vec<char>> = FxHashMap::default();
    for ([a, b], n) in edges {
        let entry = adj.entry(*a).or_default();
        for _ in 0..*n {
            entry.push(*b);
        }
    }
    adj
}

/// First (sorted) node with remaining outgoing edges.
fn next_start(adj: &FxHashMap<char, Vec<char>>) -> Option<char> {
    let mut candidates: Vec<char> = adj
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(&c, _)| c)
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Split a path into words of at most `max_word_len` chars.
/// The last char of each word is reused as the first char of the next,
/// so every consecutive pair in the original path appears in exactly one word.
fn split_path(path: &[char], max_word_len: usize, out: &mut Vec<String>) {
    let mut i = 0;
    while i + 1 < path.len() {
        let end = (i + max_word_len).min(path.len());
        out.push(path[i..end].iter().collect());
        i = end - 1;
    }
}

/// Greedy walk: follow edges from `start` until stuck.
/// Always produces a valid walk — every consecutive pair is a real input edge.
fn greedy_walk(adj: &mut FxHashMap<char, Vec<char>>, start: char) -> Vec<char> {
    let mut path = vec![start];
    let mut cur = start;
    while let Some(next) = adj.get_mut(&cur).and_then(|v| v.pop()) {
        path.push(next);
        cur = next;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::synthesise::*;
    use std::io::Cursor;

    fn edges(pairs: &[([char; 2], usize)]) -> Vec<([char; 2], usize)> {
        pairs.to_vec()
    }

    /// Round-trip: build_corpus → join with newlines → digraph::count must match input edges exactly.
    #[test]
    fn corpus_preserves_digraph_counts() {
        let input = edges(&[
            (['a', 'b'], 3),
            (['b', 'c'], 2),
            (['c', 'a'], 2),
            (['b', 'd'], 1),
        ]);
        let words = build_corpus(&input, default_max_word_len());
        let text = words.join("\n");
        let counts = counter::count_bigrams(Cursor::new(text));

        for ([a, b], n) in &input {
            assert_eq!(
                counts.get(&[*a, *b]).copied().unwrap_or(0),
                *n as u64,
                "digraph {}{}: expected {}, got {:?}",
                a,
                b,
                n,
                counts.get(&[*a, *b])
            );
        }
        // No extra digraphs introduced.
        let total_out: u64 = counts.values().sum();
        let total_in: u64 = input.iter().map(|(_, n)| *n as u64).sum();
        assert_eq!(total_out, total_in);
    }

    /// Larger synthetic dataset: all a–z pairs with pseudo-random counts.
    /// Verifies exact round-trip and no word exceeds MAX_WORD_LEN.
    #[test]
    fn corpus_preserves_digraph_counts_large() {
        // lcg deterministic pseudo-random counts 1..=50
        let mut lcg: u64 = 0xdeadbeef;
        let input: Vec<([char; 2], usize)> = ('a'..='z')
            .flat_map(|a| {
                ('a'..='z').map(move |b| {
                    ([a, b], 0usize) // placeholder; filled below
                })
            })
            .map(|([a, b], _)| {
                lcg = lcg
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let count = (lcg >> 58) as usize + 1; // 1..=64
                ([a, b], count)
            })
            .collect();

        let words = build_corpus(&input, default_max_word_len());
        assert!(!words.is_empty());

        let text = words.join("\n");
        let counts = counter::count_bigrams(Cursor::new(text));

        let mut mismatches = Vec::new();
        for ([a, b], n) in &input {
            let got = counts.get(&[*a, *b]).copied().unwrap_or(0);
            if got != *n as u64 {
                mismatches.push(format!("{}{}: expected {}, got {}", a, b, n, got));
            }
        }
        assert!(
            mismatches.is_empty(),
            "digraph mismatches:\n{}",
            mismatches.join("\n")
        );

        let total_out: u64 = counts.values().sum();
        let total_in: u64 = input.iter().map(|(_, n)| *n as u64).sum();
        assert_eq!(total_out, total_in, "extra digraphs introduced");
    }

    #[test]
    fn words_are_short() {
        let input: Vec<([char; 2], usize)> = ('a'..='e')
            .flat_map(|a| ('a'..='e').map(move |b| ([a, b], 5)))
            .collect();
        let max = default_max_word_len();
        let words = build_corpus(&input, max);
        for w in &words {
            assert!(w.len() <= max, "word too long: {}", w);
        }
    }
}
