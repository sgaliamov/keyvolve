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

Feed good layouts to `optimize` as seeds (`optimization.input`), or tune `evaluator` targets
with the breakdown table and re-run. See [readme.md](../../readme.md) for the mode list.
