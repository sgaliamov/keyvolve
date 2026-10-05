# Scoring - how it works

Scoring turns one layout into a fitness number: it types the corpus on the layout, sums
effort from the keyboard JSON, counts ergonomic events, and divides by a penalty built from
those counts. [evaluate](evaluate.md) and `optimize` use the same scoring.

## Inputs

| Input | Config key | Source |
| ----- | ---------- | ------ |
| Keyboard JSON | `evaluate.keyboard` / `optimization.keyboard` | [rank](../modes/rank.md) |
| Corpus stats JSON | `evaluate.corpusStats` / `optimization.corpusStats` | [stats](../modes/stats.md) |
| Penalty targets | `evaluator` (shared by both modes) | [penalty.md](penalty.md) |

## Fingers

Slot numbers: [placement-rules.md](../placement-rules.md#slot-map).

| Column | Left slots | Right slots |
| ------ | ---------- | ----------- |
| pinky | `0 5 10` | `19 24 29` |
| ring | `1 6 11` | `18 23 28` |
| middle | `2 7 12` | `17 22 27` |
| index-inner | `3 8 13` | `16 21 26` |
| index-outer | `4 9 14` | `15 20 25` |

index-inner and index-outer are one finger for same-finger checks (row switches, SFS).
Column metrics keep them separate.

## Corpus

The stats JSON stores shares. Scoring converts them back to counts, rounded:

| Map | Count |
| --- | ----- |
| `first_letters` | share × `word_count` |
| `bigrams` | share × `word_count × (average_word_length - 1)` |
| `trigrams` | share × `word_count × (average_word_length - 2)` |

Only sequences inside words exist; nothing is scored across word boundaries. Entries dropped
by stats filters score nothing.

## Presses and effort

Every letter typed is one press, charged to the hand, column and row of its key.

1. First letter of a word: effort = self pair `k → k` from the keyboard JSON.
2. Each bigram `a → b`, press on `b`:
   - same hand: effort = pair `a → b`;
   - other hand: effort = self pair `b → b`, plus one hand switch.
3. Trigrams add no effort; they only feed the [SFS](trigram-metrics.md) counter.

Total presses `P = words + bigrams`. All `*_ratio` metrics over presses use `P`.

## Counters

| Counter | Counted when |
| ------- | ------------ |
| `hand_switches` | bigram crosses hands |
| `left_rolls` / `right_rolls` | bigram stays on one hand (any fingers, repeats included) |
| `*_row_switch_cost` | same-finger bigram changes row: `1` adjacent row, `2` top ↔ bottom |
| `inward_count` | same hand, different finger, moving toward index-outer |
| `outward_count` | same hand, different finger, moving toward pinky |
| `sfs_count` | trigram is a same-finger skipgram |

Effort is also summed per hand, column and row. Metrics built from these:
[penalty.md](penalty.md#metric-catalog).

A repeated letter (`ll`) is a same-hand, same-finger bigram with no row change: one roll,
effort = self pair.

## Fitness

```text
fitness = fitnessScale / (effort × penalty)
```

| Key | Default | Meaning |
| --- | ------- | ------- |
| `evaluator.fitnessScale` | `1000000` | Display magnitude only. |
| `evaluator.sharpness` | `4` | Penalty curve; see [penalty.md](penalty.md#sharpness). |

Higher fitness = better layout. No targets set → `penalty = 1` → fitness ranks by effort only.

Fitness is comparable only under the same keyboard JSON, corpus stats and `evaluator`
section. After changing any of them, re-score saved layouts with [evaluate](evaluate.md).

## Mirror symmetry

The keyboard JSON defines left-hand pairs; right-hand pairs are their mirrors. A layout and
its hand-swapped mirror get the same fitness and the same metrics with left and right
swapped.

Known bug: metrics using `signed_imbalance_percent` break this, so mirror twins can score
differently. Tracked in [TASKS.md](../../TASKS.md#bugs).

## Failure modes

| Problem | Result |
| ------- | ------ |
| Corpus has a character not in the layout (uppercase, digit, punctuation) | Run aborts: `char ',' (U+002C) not in layout` |
| Keyboard JSON lacks a pair the corpus needs | Run aborts: `no pair effort for keys (from, to)` |
| Keyboard file unreadable | `Failed to read keyboard file: <path>` |
| Keyboard file not valid JSON | Run aborts: `Failed to parse keyboard JSON` |
| Stats file missing | `Missing corpus stats file: <path>` |
| Unknown key in `evaluator` | Config load fails: unknown field |

Build stats from [merge](../modes/merge.md) output to keep the corpus to `a`-`z`.
