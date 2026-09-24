# Same-finger skipgrams (SFS) — the trigram penalty

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

## Implementation

Trigram frequency is integrated into corpus statistics. Character alphabet is 26 letters, so
the full trigram space is `26³ = 17,576` entries — trivial to store and count.

The SFS detection is a pure geometric check: for trigram `(a, b, c)` with layout slots
`ka = slot(a), kb = slot(b), kc = slot(c)`:

```text
is_sfs = same_finger(ka, kc) && !same_finger(ka, kb)
```

SFS scoring adds a small penalty (default weight `0.1`) to the corpus effort fold, same pattern
as other low-confidence structural metrics (`rowSwitchImbalance`, `streakImbalance`).

The metric appears in:
- **Breakdown table:** `sfsRatio = sfs_count / total_presses`
- **CSV export:** `sfs_ratio` column
- **Tuning:** config key `sfsRatio` with `type: max`, `weight: 0.1` (start small, tune from
  breakdown pressure)
