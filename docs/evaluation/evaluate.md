# Evaluate mode - how it works

Evaluate mode scores layouts from CSV files, prints the best ones, and writes the scores back
to CSV. Use it to compare known layouts and to re-score saved layouts after changing the
keyboard JSON, corpus stats or `evaluator` targets.

## Running

```sh
keyvolve -m evaluate
```

`evaluate` is the default mode when `mode` is unset. For configuration overrides and
precedence, see the [CLI documentation](../../cliffa/README.md).

## Configuration

```yaml
evaluate:
  keyboard: data/keyboard.json
  corpusStats: data/stats/stats.json
  input:
    - data/layouts.csv
    - data/layouts.known.csv
  # output: data/layouts.all.csv
  print: 5
  eSide: any
```

| Key | Default | Meaning |
| --- | ------- | ------- |
| `keyboard` | - (required) | Keyboard JSON from [rank](../modes/rank.md). |
| `corpusStats` | - (required) | Stats JSON from [stats](../modes/stats.md). |
| `input` | - (required) | Layout CSVs to score. At least one. |
| `output` | unset | Combined result CSV, overwritten. Parent folder must exist. Unset → each `input` file is rewritten in place. |
| `print` | `10` | Best layouts printed to stdout. `0` → none. |
| `eSide` | `left` | Orientation of saved layouts: `left`, `right` or `any`. See [Orientation](#orientation). |

Scoring rules and targets: [scoring.md](scoring.md), [penalty.md](penalty.md). The
`evaluator` section is shared with `optimize`.

Unknown keys under `evaluate` are ignored: a misspelled `output` silently switches to
in-place rewrite.

## Input

One layout per row:

```text
keys_1,keys_2,keys_3,keys_4,keys_5,keys_6,name,...
qwert,asdfg,zxcvb,yuiop,hjkl_,nm___,qwerty
```

- Groups 1-3: left hand top, home, bottom. Groups 4-6: right hand top, home, bottom. Each
  group is 5 slots, physical left-to-right ([slot map](../placement-rules.md#slot-map)).
- `_` = empty slot. Each of `a`-`z` exactly once, so 4 slots are empty.
- `name` (column 7): optional. Empty or numeric → name = home-row letters.
- Columns after `name` are ignored; scores are always recomputed.
- Header rows (`keys_1,...`) and blank lines are skipped.
- Spaces around groups are trimmed.
- Identical rows within one file → first kept.

[Placement rules](../placement-rules.md) do not apply: any complete layout is scored.

Any invalid row stops the run before scoring; no file is written.

| Problem | Error |
| ------- | ----- |
| Fewer than 6 groups | `layout row needs 6 key groups, got N` |
| Group not 5 slots | `layout group N must have 5 slots, got M` |
| Symbol other than a letter or `_` | `invalid layout key` |
| Letter repeated | `duplicate layout key` |
| Letter missing, uppercase, or extra | `layout must contain all 26 keys, got N \| missing: ...` |
| `input` empty | `evaluate.input requires at least one CSV` |

## Flow

With `output` set:

1. Load the keyboard JSON, corpus stats and every `input` file.
2. Score all layouts.
3. Sort by fitness, best first.
4. Log the [penalty breakdown](penalty.md#breakdown-table-and-tuning) of the best layout.
5. Print the top `print` layouts.
6. Write all layouts to `output`.

Without `output`, steps 3-5 run over all files combined, then each `input` file is rewritten
with only its own layouts, best first.

In-place rewrite replaces the file: rows are re-sorted, re-oriented per `eSide`, extra
columns are dropped and score columns are regenerated. Keep a copy if the original matters.

## Orientation

| `eSide` | Effect on printed and saved rows |
| ------- | -------------------------------- |
| `left` | Layouts with `e` on the right hand are mirrored so `e` is on the left. Mirror twins collapse to one row. |
| `right` | Same, with `e` on the right. |
| `any` | Layouts kept as given. Mirror twins stay as separate rows. |

Mirroring swaps hands only; each letter keeps its finger and row. Scores are hand-symmetric
([scoring.md](scoring.md#mirror-symmetry)), so a twin's score columns only swap left/right
values.

With `left`/`right`, identical layouts are collapsed after mirroring, including across input
files when writing `output`; the best-scoring copy is kept. With `any`, duplicates across
files stay.

## Output

### CSV

Header plus one row per layout, best first:

```text
keys_1,...,keys_6,name,fitness,row_switch_ratio,...
```

| Column kind | Format |
| ----------- | ------ |
| `fitness` | Higher = better. |
| `effort`, `left_effort`, `right_effort` | Raw effort sums. |
| `*_ratio` | Percent, `05.20%`. |
| `*_imbalance`, `*_balance` | Absolute percent plus side: `←` left heavier, `→` right heavier, `·` even. |
| `*_count`, `hand_switches`, `*_rolls`, `*_row_switch_cost` | Raw counts. |
| Grouped (`left_column_effort_ratio`, `column_balance`, ...) | One value per finger, separated by ` │ `. Right hand in physical order (index-outer → pinky). |

`keys_1`..`keys_6` and `name` are layout metadata before the score columns.

| Name | Description |
| ---- | ----------- |
| `fitness` | Higher is better; final score after effort and penalty. |
| `row_switch_ratio` | Share of same-finger moves that change rows. |
| `row_switch_imbalance` | Left/right skew in same-finger row-switch cost. |
| `hand_switch_ratio` | Share of transitions that switch hands. |
| `hands_imbalance` | Left/right count skew between same-hand bigrams. |
| `sfs_ratio` | Share of same-finger skipgrams in the corpus. |
| `inward_ratio` | Share of same-hand motions moving toward the center of the hand. |
| `outward_ratio` | Share of same-hand motions moving away from the center of the hand. |
| `directional_outward_ratio` | Outward share among directional rolls; `0%` all inward, `50%` tie. |
| `effort` | Raw keyboard effort before penalty. |
| `efforts_imbalance` | Left/right skew in total effort. |
| `roll_imbalance` | Left/right skew in same-hand roll count. |
| `mean_streak` | Average run length across both hands. |
| `streak_imbalance` | Left/right skew in average run length. |
| `left_streak` | Average streak length on the left. |
| `right_streak` | Average streak length on the right. |
| `left_column_effort_ratio` | Left-hand column effort shares in finger order: pinky, ring, middle, index-inner, index-outer. |
| `right_column_effort_ratio` | Right-hand column effort shares in physical display order: index-outer, index-inner, middle, ring, pinky. |
| `left_column_press_ratio` | Left-hand column press shares. |
| `right_column_press_ratio` | Right-hand column press shares, display order reversed to match physical layout. |
| `column_balance` | Left/right column-balance deltas for the grouped finger columns. |
| `left_finger_row_switch_ratio` | Left-hand row-switch share by finger, pinky → index. |
| `right_finger_row_switch_ratio` | Right-hand row-switch share by finger, in physical display order. |
| `finger_row_switch_balance` | Left/right row-switch balance by finger. |
| `top_row_effort_ratio` | Share of effort on the top row. |
| `home_row_effort_ratio` | Share of effort on the home row. |
| `bottom_row_effort_ratio` | Share of effort on the bottom row. |
| `row_balance` | Left/right imbalance for top, home and bottom rows. |
| `left_effort_ratio` | Left-hand effort share of total effort. |
| `right_effort_ratio` | Right-hand effort share of total effort. |
| `left_count_ratio` | Left-hand press share of total presses. |
| `right_count_ratio` | Right-hand press share of total presses. |
| `left_effort` | Total raw effort on the left hand. |
| `right_effort` | Total raw effort on the right hand. |
| `left_count` | Same-hand bigram count on the left. |
| `right_count` | Same-hand bigram count on the right. |
| `hand_switches` | Number of transitions that cross hands. |
| `left_row_switch_cost` | Weighted same-finger row-switch cost on the left. |
| `right_row_switch_cost` | Weighted same-finger row-switch cost on the right. |
| `left_rolls` | Same-hand rolls on the left. |
| `right_rolls` | Same-hand rolls on the right. |
| `inward_count` | Same-hand directional moves toward the center. |
| `outward_count` | Same-hand directional moves away from the center. |
| `sfs_count` | Number of same-finger skipgrams in the corpus. |
| `home_row_balance` | Left/right effort imbalance on the home row. |
| `left_pinky_ratio` / `left_ring_ratio` / `left_middle_ratio` / `left_index_inner_ratio` / `left_index_outer_ratio` | Left-hand per-finger effort share. |
| `right_pinky_ratio` / `right_ring_ratio` / `right_middle_ratio` / `right_index_inner_ratio` / `right_index_outer_ratio` | Right-hand per-finger effort share. |
| `left_pinky_row_switch_ratio` / `left_ring_row_switch_ratio` / `left_middle_row_switch_ratio` / `left_index_row_switch_ratio` | Left-hand per-finger row-switch share. |
| `right_pinky_row_switch_ratio` / `right_ring_row_switch_ratio` / `right_middle_row_switch_ratio` / `right_index_row_switch_ratio` | Right-hand per-finger row-switch share. |
| `pinky_balance` / `ring_balance` / `middle_balance` / `index_inner_balance` / `index_outer_balance` | Left/right effort balance for each finger. |
| `pinky_row_switch_balance` / `ring_row_switch_balance` / `middle_row_switch_balance` / `index_row_switch_balance` | Left/right row-switch balance for each finger. |

Metric definitions: [penalty.md](penalty.md#metric-catalog). Counters: [scoring.md](scoring.md#counters).

### Console

Before the list, the penalty breakdown of the best layout is logged (only when at least one
target is set). Then each printed layout shows its slots, `[pool 0]` (always `0` in
evaluate) and a summary:

| Symbol | Metric |
| ------ | ------ |
| `φ` | fitness |
| `↕` / `⇄` | `row_switch_ratio` / `hand_switch_ratio` |
| `⟳Δ` / `Δ` / `εΔ` | `roll_imbalance` / `hands_imbalance` / `efforts_imbalance` |
| `↕↔` / `→Δ` | `row_switch_imbalance` / `streak_imbalance` |
| `→` / `ε` | `mean_streak` / `effort` |
| `cL` / `cR` | column effort shares per hand, physical order |
| `sΔ` | per-finger row-switch balance: pinky, ring, middle, index |
| `rT` / `rH` / `rB` | top / home / bottom row effort: left, right, total |
| `bal` | top / home / bottom row left-right imbalance |

## Interruption

Ctrl+C stops scoring. Nothing is written; every `input` file and `output` keep their
previous content. Without `output`, the run logs
`Evaluation interrupted before all files were scored; skipped rewriting input files`.

Known bug: with `output` set, an interrupted run currently overwrites `output` with the
partially scored layouts. Tracked in [TASKS.md](../../TASKS.md#bugs).

## Next steps

Feed good layouts to [optimize](../modes/optimize.md) as seeds (`optimization.input`), or tune `evaluator` targets
with the breakdown table and re-run. See [readme.md](../../readme.md) for the mode list.
