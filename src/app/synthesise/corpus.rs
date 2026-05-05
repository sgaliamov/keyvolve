use rustc_hash::FxHashMap;

/// Build fake words from scaled digraph edges via Eulerian path decomposition.
pub fn build_corpus(edges: &[([char; 2], usize)]) -> Vec<String> {
    let mut adj = build_adj(edges);
    balance_degrees(&mut adj, edges);
    extract_paths(&mut adj)
}

/// Build directed adjacency list from edge list.
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

/// Add self-loop bridge edges to nodes where out-degree exceeds in-degree,
/// making each node Eulerian.
fn balance_degrees(adj: &mut FxHashMap<char, Vec<char>>, edges: &[([char; 2], usize)]) {
    let mut in_deg: FxHashMap<char, i64> = FxHashMap::default();
    let mut out_deg: FxHashMap<char, i64> = FxHashMap::default();
    for ([a, b], n) in edges {
        *out_deg.entry(*a).or_insert(0) += *n as i64;
        *in_deg.entry(*b).or_insert(0) += *n as i64;
    }
    let mut nodes: Vec<char> = adj.keys().cloned().collect();
    nodes.sort();
    for node in nodes {
        let deficit =
            out_deg.get(&node).copied().unwrap_or(0) - in_deg.get(&node).copied().unwrap_or(0);
        if deficit > 0 {
            let entry = adj.entry(node).or_default();
            for _ in 0..deficit {
                entry.push(node);
            }
        }
    }
}

/// Run Hierholzer per start node; collect non-trivial paths as fake words.
fn extract_paths(adj: &mut FxHashMap<char, Vec<char>>) -> Vec<String> {
    let starts: Vec<char> = {
        let mut s: Vec<char> = adj.keys().cloned().collect();
        s.sort();
        s
    };
    let mut result = Vec::new();
    for start in starts {
        if adj.get(&start).is_none_or(|v| v.is_empty()) {
            continue;
        }
        let path = hierholzer(adj, start);
        if path.len() > 1 {
            result.push(path.iter().collect());
        }
    }
    result
}

/// Hierholzer's algorithm — extracts an Eulerian path from `start`.
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
