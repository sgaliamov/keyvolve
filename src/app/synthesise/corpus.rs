use rustc_hash::FxHashMap;

/// Max characters per output word. Boundary char is shared with next word
/// so all digraphs are preserved across splits.
const MAX_WORD_LEN: usize = 8;

/// Build short fake words from scaled digraph edges via Eulerian path decomposition.
/// Every input edge appears as exactly one consecutive char-pair in the output;
/// no extra edges are introduced.
pub fn build_corpus(edges: &[([char; 2], usize)]) -> Vec<String> {
    let mut adj = build_adj(edges);
    let mut words = Vec::new();
    loop {
        let Some(start) = next_start(&adj) else { break };
        let path = hierholzer(&mut adj, start);
        if path.len() > 1 {
            split_path(&path, &mut words);
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

/// Split a path into words of at most MAX_WORD_LEN chars.
/// The last char of each word is reused as the first char of the next,
/// so every consecutive pair in the original path appears in exactly one word.
fn split_path(path: &[char], out: &mut Vec<String>) {
    let mut i = 0;
    while i + 1 < path.len() {
        let end = (i + MAX_WORD_LEN).min(path.len());
        out.push(path[i..end].iter().collect());
        i = end - 1;
    }
}

/// Hierholzer's algorithm — extracts an Eulerian path/circuit from `start`.
fn hierholzer(adj: &mut FxHashMap<char, Vec<char>>, start: char) -> Vec<char> {
    let mut stack = vec![start];
    let mut path = Vec::new();
    while let Some(&top) = stack.last() {
        if adj.get(&top).is_some_and(|v| !v.is_empty()) {
            let next = adj.get_mut(&top).unwrap().pop().unwrap();
            stack.push(next);
        } else {
            path.push(stack.pop().unwrap());
        }
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::synthesise::digraph;
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
        let words = build_corpus(&input);
        // Join words with newlines so digraph::count resets between words (same as spaces).
        let text = words.join("\n");
        let counts = digraph::count(Cursor::new(text));

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

    #[test]
    fn words_are_short() {
        let input: Vec<([char; 2], usize)> = ('a'..='e')
            .flat_map(|a| ('a'..='e').map(move |b| ([a, b], 5)))
            .collect();
        let words = build_corpus(&input);
        for w in &words {
            assert!(w.len() <= MAX_WORD_LEN, "word too long: {}", w);
        }
    }
}
