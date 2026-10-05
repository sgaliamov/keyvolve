# Optimize mode - how it works

Optimize mode searches for a better layout with the same score model as [evaluate.md](../evaluation/evaluate.md),
but it adds placement constraints and runs a small GA. It uses the evaluator in [scoring.md](../evaluation/scoring.md)
and the same penalty targets in [penalty.md](../evaluation/penalty.md), including the trigram SFS term from
[trigram-metrics.md](../evaluation/trigram-metrics.md).

## Running

```sh
keyvolve -m optimize
```

`optimization` holds the optimizer-specific settings. The shared scoring knobs live in the
`evaluator` section of [keyvolve.yaml](../../keyvolve.yaml).

## Configuration

```yaml
optimization:
  keyboard: data/keyboard.json
  corpusStats: data/stats/stats.json
  input: data/layouts.csv
  output: data/layouts.csv
  mutationCount: 10
  maxGroups: 10
  itemsPerGroup: 6

  frozen:
    z: 10
  blocked: [0, 19, 10, 29]
  allowed:
    e: [6, 7, 8]
    _: [0, 1, 10, 11]
  sameSide: ['th', 'er']
  left: [s]
  right: [o]
```

| Name | Meaning |
| --- | --- |
| `keyboard` | Keyboard effort table used to score variants. |
| `corpusStats` | Cached corpus stats used by the GA evaluator. |
| `input` | Seed layout CSVs. Invalid rows are repaired under the current placement rules. |
| `output` | Final CSV written after the run. |
| `mutationCount` | Mutants created per parent each generation. Default: `10`. |
| `maxGroups` | Max home-row groups kept in the final output. Default: `10`. |
| `itemsPerGroup` | Max layouts kept per home-row group. Default: `6`. |
| `frozen` | `{ char: slot }` pins a letter to a physical slot. |
| `blocked` | Slots unavailable for letter placement. Empty slots may still use them. |
| `allowed` | Per-letter legal slot list. Half-positions are mirrored to the opposite hand. `_` means empty slots. |
| `sameSide` | Pairs that must stay on the same hand. Overlapping pairs merge into one group. |
| `left` / `right` | Letters forced to the left or right hand. |

See [placement-rules.md](../placement-rules.md) for the exact slot geometry and empty-slot rules.

## Constraint checks

Before any generation is scored, the optimizer compiles the placement rules and rejects impossible
layouts. A complete genome must:

- contain all 26 letters exactly once;
- contain exactly 4 empty slots;
- keep frozen characters pinned;
- respect `blocked`, `allowed`, `left`, `right`, and `sameSide`;
- avoid duplicate slots or invalid characters.

The validation is stricter than [evaluate.md](../evaluation/evaluate.md): optimize never scores a partial or
infeasible layout. If a seed genome is invalid, it is repaired before the GA starts, and the run
logs:

```text
Repaired imported layouts under current placement constraints
```

The repair step keeps valid assignments where possible and reassigns the rest under the compiled
constraint set.

## GA flow

1. Load the keyboard JSON and corpus stats.
2. Compile `OptimizationConfig` into `PlacementConstraints`.
3. Seed the GA with valid genomes; invalid imports are repaired.
4. Run the island-model GA:
   - `generate` builds a valid genome;
   - `mutate` perturbs a valid layout;
   - `NoopCrossover` keeps the GA simple;
   - `evaluator` rejects invalid genomes and scores valid ones via `LayoutEvaluator::score_corpus`;
   - `callback` prints the current best layout and diversity.
5. After the run, group survivors by their home-row fingerprint.
6. Keep the best layouts from each group, deduplicate identical genomes, and write the final CSV.

`top_by_home_row` is the key post-processing step: the output is not just the global winner, but a
small set of strong layouts spread across distinct home-row patterns.

## Output

The optimizer writes the same score columns as [evaluate.md](../evaluation/evaluate.md#csv):

```text
keys_1,...,keys_6,name,fitness,row_switch_ratio,...
```

Rows are sorted by fitness within each home-row group, and the best group champions are kept in the
final output. The run also logs a penalty breakdown for the top layout before saving.

## Interruption

`Ctrl+C` or shutdown stops the run. The optimizer does not save any CSV when the app reports
abort:

```text
Run aborted; layouts not saved
```

This mirrors the safety rule in [evaluate.md](../evaluation/evaluate.md#interruption): no partial output is
written on an interrupted run.

## Failure modes

| Problem | Typical error |
| --- | --- |
| No valid slot for a letter or `_` | `optimization leaves no legal slots for 'x'` |
| Frozen slots collide | `multiple frozen keys use slot N` |
| Slot index out of range | `frozen slot N must be in 0..29` |
| `allowed` includes illegal full positions | `allowed slot N must be in 0..14 for letter keys` |
| Constraints are impossible | `optimization constraints have no complete layout` |
| A seed layout can never satisfy rules | `no complete layout` during compile |

## Next steps

- Tune the cost terms in [penalty.md](../evaluation/penalty.md) and re-run.
- Feed good layouts into `optimization.input` for a stronger starting population.
- Compare saved outputs with [evaluate.md](../evaluation/evaluate.md) before shipping a layout.
