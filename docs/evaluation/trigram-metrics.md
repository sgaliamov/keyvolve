# Same-finger skipgrams (SFS) - how it works

SFS counts trigrams where the 1st and 3rd keys use the same finger and the 2nd does not, and
`sfsRatio` penalizes their share. It covers the one trigram effect that bigram effort cannot
see.

## Why a separate metric

Pair effort from the keyboard JSON prices adjacent keys only. The 1 → 3 relationship (finger
fires, idles one keystroke, fires again) is never looked up. [rank](../modes/rank.md) can't
calibrate it either: it compares bigrams pairwise, and a typist can't separate "skip
reactivation" from "awkward keys" in one trial.

So SFS is a yes/no detector with a configured weight, like `rowSwitchRatio`. It flags the
pattern without sizing it per instance. Tune the weight from the
[breakdown table](penalty.md#breakdown-table-and-tuning).

## Match condition

For trigram `a b c`: SFS when `a` and `c` are on the same finger and `b` is not on that
finger.

- Same finger = same hand and same finger column; index-inner and index-outer count as one
  finger ([fingers](scoring.md#fingers)).
- `b` may be on either hand.
- Literal repeat counts (`aha`, `a` and `a` on one key).
- All three on one finger does not count: both hops are same-finger bigrams already priced
  by pair effort.
- Only trigrams inside words that survive [stats](../modes/stats.md) filtering
  (`minTrigramFrequency`).

## Redirects

A redirect (same hand, finger direction reverses, e.g. pinky → middle → ring) splits into
hops 1 → 2 and 2 → 3, both adjacent bigrams already priced by pair effort. A separate penalty
would need a reversal cost beyond the two hops; there is no evidence for one, and rank mode
couldn't measure it. No redirect metric.

## Config and output

| Where | What |
| ----- | ---- |
| `evaluator.sfsRatio` | Optional target, no default. Example: `{ type: max, value: 6, weight: 1 }`. |
| Metric | `sfs_ratio = 100 × sfs_count / P` ([penalty.md](penalty.md#press-patterns)) |
| Breakdown table | `sfs_ratio` row when the target is set |
| CSV | `sfs_ratio`, `sfs_count` columns |
