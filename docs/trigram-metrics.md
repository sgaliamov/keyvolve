# Trigram effects — problem and implementation plan

Bigram effort (the `keyboard.json` pairs table, calibrated by rank mode) prices any two-key
transition in isolation. It cannot see effects that only exist across **three** consecutive
presses. This document explains why, lays out the concrete plan for the one trigram effect
we're adding a penalty for — same-finger skipgrams (SFS) — and explains why the other classic
trigram metric, redirects, turns out **not** to need one.

## The gap

Rank mode calibrates 210 ordered pairs (15 left-hand keys × 14, mirrored to the right) by
asking a human "which of these two bigrams is easier." Every possible `(from, to)` transition
between two different keys already has a number. That's not the gap.

The gap: **that number is calibrated, and later looked up, only in the context of a bare
two-key transition** — starting fresh, no memory of what happened before.

**Same-finger skipgrams (SFS)** are the concrete case that falls into this gap: keys 1 and 3
use the same finger, key 2 (any hand, any finger, just not the same one) sits between them.
The finger fires, sits idle for exactly one other keystroke, then fires again. Whether that's
harder than a normal transition is a question about the *gap between two activations of one
finger* — pairwise effort was never asked that question, because rank mode only ever compares
two-key sequences typed fresh.

Corpus stats make this concrete too: `CorpusCounts` only stores adjacent-letter frequencies
(`bigrams: (char, char) → count`). The skip relationship between position 1 and 3 of a
trigram is never even queried during scoring — the data to ask the question doesn't exist yet.

Redirects look like a similar trigram-only case at first glance. They aren't — see "Why
redirects don't need a separate penalty" below.

## How rank mode works

Full mechanics: [rank-mode.md](/c:/Users/Admin/projects/keyvolve/docs/rank-mode.md). The
short version, relevant to why it can't stretch to trigrams: rank mode repeatedly asks a human
to type option 1 (e.g. `TE`), type option 2 (e.g. `TD`), and pick the easier one. Answers feed
a Bradley–Terry fit — the same rating model used for chess/game ladders — producing one skill
rating per bigram. That rating becomes the calibrated effort in `keyboard.json` after tiering.

The protocol is pairwise **by construction**: every question is "bigram A vs bigram B," and
Bradley–Terry itself is defined over pairwise comparisons between individual items. There's no
natural extension to "bigram A vs bigram B, given trigram context C" — that would need a
different comparison unit (trigram vs trigram) and a much larger rating space (26³ trigrams
instead of 210 bigrams), with no existing math in rank mode built to fit it. Practically, it's
also physiologically unreliable: a human can't cleanly isolate "harder because of the
skip-reactivation" from "harder because these are just awkward keys" in one typed trial — the
two effects blend into a single felt sensation.

## Design decision: penalty, not calibration

Keep the pairwise effort table exactly as-is — don't try to extend rank mode to rank triples
(see above for why that protocol doesn't stretch to trigrams).

Instead: add a small, **uncalibrated, structural** penalty — the same pattern already used for
`rowSwitchRatio`/`handSwitchRatio`. Those aren't calibrated through rank mode either; they're
formulas over the layout's slot geometry, with a config-driven weight the user tunes by feel.
SFS gets the same treatment: a geometric detector (same finger, one key between, different
finger in between) plus a small default weight — a nudge, not a claim of measured cost.

This also matches the project's own existing convention for low-confidence signals:
`rowSwitchImbalance` and `homeRowBalance` both ship at `weight: 0.01`, `streakImbalance` at
`weight: 0.1` — soft tie-breakers, not primary drivers. SFS penalty should start in that same
range, not compete with `handSwitchRatio`-tier weights.

## Emulating SFS without calibration

Rank mode can't produce a real number for "cost of reactivating this finger after one
intervening keystroke" — that question is outside its protocol (see above). So the SFS
penalty is an **emulation**, not a measurement: a geometric detector standing in for a
hypothesis, not a calibrated fact. The hypothesis itself comes from motor-control research on
finger independence — the same papers already cited in the post draft — which suggests
same-finger reactivation under time pressure measurably degrades.

The detector only answers a yes/no question (does this trigram match the SFS pattern) with a
fixed, config-driven weight. Unlike per-pair effort — which tries to represent *how much*
harder a specific transition is, calibrated from real human comparisons — the SFS detector
doesn't attempt to size the effect per instance, only to flag it and apply a uniform nudge.
That's the whole reason the weight starts small and gets tuned by feel from the breakdown
table, instead of being derived from any calibration session: there's nothing to derive it
from.

## SFS match condition

For trigram `(a, b, c)` under layout `keys`:

```text
ka = slot(a), kb = slot(b), kc = slot(c)
same_hand(x, y)   = (x < 15) == (y < 15)
same_finger(x, y) = same_hand(x, y) && logical_finger(x) == logical_finger(y)

is_sfs = same_finger(ka, kc) && !same_finger(ka, kb)
```

- `same_finger(ka, kc)` — positions 1 and 3 land on the same physical finger (same hand *and*
  same column-group; `logical_finger` alone is hand-agnostic, must gate on hand too — this is
  exactly the same combined check `score_bigram` already does inline for `same_finger`).
- `!same_finger(ka, kb)` — excludes the case where all three are the same finger. That case is
  already a same-finger bigram on `(a, b)` and `(b, c)` both — already priced by the pairwise
  effort table (rank mode explicitly ranks same-finger bigrams as among the worst). Counting
  it again here would double-charge an already-priced pattern.
- `ka == kc` (literal repeat, e.g. "aha") still counts as SFS — same finger reactivating is the
  point, not whether the key itself repeats.
- Key 2 can be on **either hand** — the "one keystroke to recover" argument doesn't care which
  hand produced the intervening press.

## Why redirects don't need a separate penalty

A redirect is three consecutive same-hand presses where finger-order direction reverses (e.g.
pinky → middle → ring: inward then outward). At first glance this looks like the same kind of
trigram-only gap as SFS. It isn't — the difference is which key relationships actually get
priced.

SFS needs a relationship — position 1 to position 3 — that bigram scoring **never queries at
all**. `CorpusCounts` only tracks adjacent letter pairs; the skip relationship has zero data
behind it structurally, not just an imprecise number.

A redirect, by contrast, decomposes entirely into two relationships that **are** already
queried and priced: hop 1→2 and hop 2→3. Both are ordinary adjacent bigrams, both get looked
up in the pairs table, both were individually calibrated by a human in rank mode. Whatever
makes a redirect trigram uncomfortable, the two hops that make it up already carry their own
independently calibrated cost — a redirect through two already-awkward, expensive transitions
already scores high on raw effort *for that reason*, without needing to know anything about
"reversal" as a separate concept.

The only way redirects would need their own penalty is if reversing direction carries a cost
**beyond** the sum of its two hops — a genuine interaction effect, not just "both hops happen
to be expensive." That's a much shakier claim than the SFS case (where the missing
relationship is structurally invisible, not just unproven), and it couldn't be measured any
more easily than SFS could (same rank-mode limitation above). Absent a specific, demonstrated
reason to believe such an interaction effect exists and matters, decomposing into already-priced
pairs is the right call — no new metric, no new corpus data, nothing to build.

## Data plan: trigram frequency stats

Note for the future: the synthetic-corpus mode remains optional and is kept as a convenience for exploratory work. The primary path for real scoring remains the cached corpus statistics built by the build-stats pipeline, because the evaluator and optimizer are designed to consume normalized corpus frequencies rather than ad hoc generated text.

Character alphabet is 26 letters, so the full trigram space is `26³ = 17,576` entries — trivial
to store and count, no risk of reintroducing the "must scan raw text at scoring time" cost the
project already avoided for bigrams.

### `CorpusStatsCounter` / `CorpusStats` ([counter.rs](/c:/Users/Admin/projects/keyvolve/src/modes/synthesise/counter.rs))

Add, mirroring the existing bigram fields exactly:

```rust
// CorpusStatsCounter
trigram_counts: FxHashMap<[char; 3], u64>,
total_trigrams: u64,

// CorpusStats
#[serde(with = "trigram_map_serde", default)]
pub trigrams: FxHashMap<[char; 3], f64>,
```

`add_word` needs a 3-wide sliding window instead of the current 2-wide (`prev`) — track
`prev2, prev1` and emit `[prev2, prev1, ch]` once the window fills. Words shorter than 3
letters contribute zero trigrams (same convention as short words contributing zero bigrams).
Trigrams never cross word boundaries — window resets per word, exactly like bigrams do today.

`finish()` normalizes the same way `normalize_bigram_counts` does today (count / total).

Needs a `trigram_map_serde` module, same shape as the existing `bigram_map_serde` in
[counter.rs](/c:/Users/Admin/projects/keyvolve/src/modes/synthesise/counter.rs), just a 3-char
key instead of 2.

**Backward compatibility:** add `#[serde(default)]` on the new `trigrams` field. Existing
cached `data/stats/*.json` files (missing the field) deserialize with an empty trigram map —
`sfsRatio` reads as `0` until `merge`/`synthesise` is re-run to regenerate the cache. No hard
break, just a silent zero until refreshed.

### `CorpusCounts` ([evaluator/corpus.rs](/c:/Users/Admin/projects/keyvolve/src/evaluator/corpus.rs))

Add `trigrams: FxHashMap<(char, char, char), u64>`, reconstructed from cached stats the same
way `bigrams` already is:

```text
trigram_total = words * (average_word_length - 2.0).max(0.0)
```

(mirrors the existing `bigram_total = words * (average_word_length - 1.0).max(0.0)`, one fewer
because a trigram needs 2 more letters after the first, not 1).

### `LayoutEvaluator::score_corpus` ([evaluator/mod.rs](/c:/Users/Admin/projects/keyvolve/src/evaluator/mod.rs))

New pass alongside the existing `first_chars`/`bigrams` folds:

```rust
let sfs = self.counts.trigrams.iter()
    .map(|(&(a, b, c), &n)| self.score_sfs(a, b, c, keys) * n);
```

`score_sfs` does **not** touch the pairs table at all — it's a pure geometric check on
`keys` (the candidate layout), independent of `keyboard.json`. Returns an incremental
`ScoreResult`-shaped value with just a count bumped when `is_sfs` holds.

### `ScoreResult` ([models/score.rs](/c:/Users/Admin/projects/keyvolve/src/models/score.rs))

New field: `sfs_count: u64` (aggregate — no left/right split needed for v1; SFS by definition
always resolves to one specific hand, so a balance metric doesn't obviously apply the way it
does for effort/rolls. Revisit if it turns out to matter).

New metric:

```text
sfs_ratio = sfs_count / (left_count + right_count)
```

Same denominator convention as `hand_switch_ratio` and `row_switch_ratio` — total presses,
not "eligible trigrams." Simpler, and keeps the number comparable to the other ratio metrics
already in the breakdown table.

### `Targets` / `penalty.rs`

New field `sfs_ratio: Option<Target>`, default `None` (opt-in, same as most non-hand-level
metrics) — no built-in default `value` until it's been observed on a real corpus first. Add
one breakdown row in `terms()`, same shape as every other `Target`-driven metric.

Config usage once implemented:

```yaml
evaluator:
  sfsRatio: { type: max, value: <observe first>, weight: 0.1 }  # start small, tune from breakdown share/pressure
```

### CSV

Add `sfs_ratio` column, same position/format convention as the other ratio columns.

## Impact on rank mode

None. No new questions, no new calibration data. SFS is purely geometric (slot positions +
corpus trigram frequency), computed at scoring time from data already available once the
corpus stats extension above lands.

## Implementation steps

Ordered so each step is independently testable before moving to the next.

1. **`CorpusStatsCounter`/`CorpusStats`** ([counter.rs](/c:/Users/Admin/projects/keyvolve/src/modes/synthesise/counter.rs))
   - Add `trigram_counts`/`total_trigrams` to the counter; extend `add_word`'s sliding window
     to 3 chars (`prev2, prev1, ch`), reset per word.
   - Add `trigrams: FxHashMap<[char; 3], f64>` to `CorpusStats` with `#[serde(default)]` and a
     new `trigram_map_serde` module (copy `bigram_map_serde`, 3-char key).
   - Update `finish()` to normalize trigram counts the same way bigrams are normalized.
   - Test: extend `counter_matches_slice_calculation` and `calculate_stats_counts_requested_metrics`
     with trigram assertions; add a short-word case (len < 3 → zero trigrams) and a
     word-boundary case (no trigram spans two words).

2. **Backward compatibility check**
   - Deserialize an existing `data/stats/*.json` file (or a fixture missing the `trigrams`
     key) and confirm it loads with an empty trigram map instead of failing.
   - Test: a fixture-based deserialize test asserting `trigrams.is_empty()` on old-shape JSON.

3. **`CorpusCounts`** ([evaluator/corpus.rs](/c:/Users/Admin/projects/keyvolve/src/evaluator/corpus.rs))
   - Add `trigrams: FxHashMap<(char, char, char), u64>` and reconstruct it in
     `From<&CachedSourceStats>` using `trigram_total = words * (average_word_length - 2.0).max(0.0)`.
   - Test: mirror `counts_from_cached_stats_match_direct_counts`, extended to trigrams.

4. **SFS geometry check** ([evaluator/mod.rs](/c:/Users/Admin/projects/keyvolve/src/evaluator/mod.rs))
   - Implement `is_sfs`/`score_sfs` per the match condition above. Keep it a pure function of
     `keys` — no dependency on `keyboard.json`.
   - Test: unit tests directly on `is_sfs`/`score_sfs` covering: same-finger skip (true), all
     three same finger (false — already an SFB, must not double-count), different fingers
     throughout (false), key-2 on the opposite hand (still true), literal repeat `ka == kc`
     (true).

5. **Wire into `score_corpus`**
   - Add the trigram fold alongside the existing `first_chars`/`bigrams` folds; accumulate into
     a new `sfs_count` field on `ScoreResult`.
   - Add `sfs_ratio()` (`sfs_count / (left_count + right_count)`).
   - Test: an end-to-end `score_corpus` test with a small hand-built layout + trigram corpus
     where the expected SFS count is known by hand.

6. **`Targets`/`penalty.rs`**
   - Add `sfs_ratio: Option<Target>` (default `None`), one breakdown row in `terms()`.
   - Test: mirror the existing `Targets`/`penalty` serde and breakdown tests (unknown-field
     rejection, breakdown row appears only when configured).

7. **CSV**
   - Add `sfs_ratio` column to `csv_header()`/`to_csv()`.
   - Test: extend the existing CSV round-trip/header test.

8. **Baseline measurement (before picking a default `value`)**
   - Run `evaluate` on qwerty and the current best layout with `sfsRatio` exposed but
     unweighted (or `weight: 0`), read the raw percentage from the breakdown/CSV.
   - Pick an initial `value`/`weight` from that spread, not a guess — then tune from the
     breakdown table's `share`/`pressure` as usual.

9. **Full regression pass**
   - `./scripts/lint.ps1` and `./scripts/test.ps1` — confirm no existing bigram/CSV/config
     tests broke, since steps 1–3 touch shared serialization code paths.

## Open questions (resolve during implementation, not before)

- **Tail filtering:** bigrams get `min_frequency` pruning (`filter_stats_bigrams`) before
  scoring. Trigrams may want the same treatment (17,576-entry space is small, but real text
  produces a long tail of near-zero entries) — likely a straightforward extension of the
  existing filter, not a blocker for a first working version.
- **Default `value` for the `max` target:** unknown until measured. First step after landing
  the raw `sfs_ratio` computation: run `evaluate` on a few known layouts (qwerty, current best)
  and read the raw percentage before picking a cap.
- **Left/right split:** skipped for v1 per above; add only if the breakdown table shows a real
  need for it (e.g. one hand's SFS rate systematically dominates).
