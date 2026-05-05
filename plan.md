# Plan: Synthesise Mode

New `Mode::Synthesise`: reads large text → counts digraphs → saves frequency CSV → emits a compact fake-word corpus that preserves original digraph frequencies, ready for `LayoutEvaluator` without changes.

Key insight: treat each char as a node, each digraph `ab` as a directed edge. Build a directed multigraph (edges weighted by normalized count), then extract Eulerian paths per connected component via **Hierholzer's algorithm** — each path = one fake word. Consecutive char pairs in fake words = real digraphs at the correct frequency.

`ab` and `ba` are distinct edges. Only `a-z`; spaces separate fake words.

## Steps

1. Add `Mode::Synthesise` to `src/config.rs`. Add `output: Option<PathBuf>` and `target: Option<usize>` (edge normalization total) to `Config`.

2. Wire new mode in `src/app/run.rs` dispatch — load text, call `synthesise::run(cfg)`.

3. Create `src/app/synthesise/mod.rs`:
   - Chunk-read input via `BufReader`; accumulate `HashMap<(char, char), u64>` (lowercase `a-z` only, skip pairs crossing whitespace boundaries).
   - Write digraph CSV: `pair,count,frequency` sorted by frequency desc.

4. **Filter & normalize**: drop pairs below **0.1% relative frequency** (min precision floor). Scale remaining counts to `target` total edges (config param, default `100_000`). Round proportionally; redistribute rounding error to top pairs.

5. **Build multigraph & Eulerian paths**: insert each pair as N directed edges. Per weakly-connected component, balance in/out degrees by adding minimal reverse bridge edges. Run Hierholzer → one fake word per component.

6. Write fake words space-separated to output `.txt`.

## Further Considerations

1. **`target` default**: `100_000` edges → ~200–300 KB output. Expose as `"target": 100000` in `keyvolve.json`.
2. **Bridge edges from balancing**: added to fix in/out degree parity, slightly distort frequencies. Negligible at 100k scale.
3. **Chunking strategy**: line-batched `BufReader` avoids splitting multi-byte chars; sufficient unless files are >1 GB.
