# Same-finger skipgrams (SFS)

Bigram effort (`keyboard.json` pairs, calibrated by [rank mode](modes/rank.md)) prices each
two-key transition in isolation. SFS is the one trigram effect it cannot see, so it gets a
separate structural penalty. Redirects look similar but don't need one.

## The gap

SFS: keys 1 and 3 use the same finger, key 2 does not. The finger fires, idles one keystroke,
fires again.

Bigram scoring never queries the 1→3 relationship — `CorpusCounts.bigrams` holds adjacent
pairs only. Rank mode can't fill it either: it's pairwise by construction (Bradley–Terry over
bigram-vs-bigram choices), and a typist can't separate "skip reactivation" cost from "awkward
keys" cost in one trial.

## Penalty, not calibration

SFS is a yes/no geometric detector with a config-driven weight — same pattern as
`rowSwitchRatio` / `handSwitchRatio`. It flags the pattern; it does not size it per instance.
No calibration data exists to derive the weight, so tune it by feel from the breakdown table.

## Match condition

For trigram `(a, b, c)` with slots `ka`, `kb`, `kc`:

```text
same_finger(x, y) = same_hand(x, y) && logical_finger(x) == logical_finger(y)
is_sfs            = same_finger(ka, kc) && !same_finger(ka, kb)
```

- Hand check required: `logical_finger` is hand-agnostic.
- Both index columns map to one logical finger.
- All-same-finger trigrams excluded: both hops are already same-finger bigrams priced by the
  pairs table.
- Literal repeat (`ka == kc`, e.g. "aha") counts.
- Key 2 may be on either hand.

## Why redirects don't need a penalty

A redirect (same hand, finger direction reverses, e.g. pinky → middle → ring) decomposes into
hops 1→2 and 2→3 — both adjacent bigrams already calibrated in the pairs table. A separate
penalty would only be justified by a reversal cost *beyond* the sum of the hops; no evidence
for that, and rank mode couldn't measure it anyway.

## Implementation

- Corpus stats count trigrams (`26³ = 17,576` max entries).
- [`is_sfs`](../src/evaluator/mod.rs) runs per trigram in `score_corpus`; matches accumulate in
  `ScoreResult.sfs_count`.
- `sfs_ratio = sfs_count / total_presses`.
- Penalty: optional target `sfsRatio` (no default). Example in `keyvolve.yaml`:
  `sfsRatio: { type: max, value: 6, weight: 1 }`.
- Output: `sfs_ratio` in breakdown table; `sfs_ratio` and `sfs_count` CSV columns.
