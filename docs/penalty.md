# Penalty model — configurable scoring knobs

This is the live reference for the score penalty used by the evaluator: it matches the
`Target` API in [../src/models/target.rs](../src/models/target.rs), the default target set in
[../src/models/targets.rs](../src/models/targets.rs), and the metric breakdown in
[../src/models/score.rs](../src/models/score.rs).

The optimizer does not just minimize raw typing effort. It also penalizes left/right imbalance,
row jumps, pinky overload, same-finger repeats, and a dozen other ergonomic signals. Each of
those signals is a metric you can cap or target in the `evaluator` section of
[keyvolve.yaml](../keyvolve.yaml).

## Fitness and penalty

```text
fitness = fitness_scale / (effort × penalty)
penalty = 1 + Σ weight · deviation^sharpness
```

- `effort` — raw bigram cost from the keyboard effort table. It is layout-shape only; targets do
  not change it.
- `penalty` — dimensionless multiplier, always `>= 1.0`. If every metric is on target, `penalty = 1.0`
  and fitness falls back to the effort-only ideal.
- The leading `1` is the neutral element of a multiplier, not a fixed cost. It also keeps the
  divide safe: a penalty near zero would send fitness toward infinity and drown out effort.
- `fitnessScale` only changes display magnitude. It does not reorder layouts.

## Target types

Every metric is a `Target`:

```yaml
metricName: { type: max|target, value: <number>, weight: <number>, tolerance: <number> }
```

- `type` — `max` means lower is better; `value` is the accepted ceiling. `target` means closer is
  better; `value` is the desired point.
- `value` — the ceiling (`max`) or desired point (`target`), in the percent units the CSV prints.
- `weight` — priority against other metrics. Default: `1`.
- `tolerance` — only for `target`; accepted miss in percentage points before cost ramps up. Default:
  `5`. Ignored by `max`.

Deviation is normalized so `0.0` is ideal and `1.0` is the accepted edge:

```text
max:    deviation = |value| / target.value
target: deviation = |value - target.value| / tolerance
```

`norm` is `target.value` for `max` and `tolerance` for `target`.

A metric resting exactly at its edge costs exactly its own `weight`. That makes `value` and
`tolerance` normalizers and `weight` the priority knob.

## Sharpness

```text
cost = weight · deviation^sharpness
```

`sharpness` is config-wide; the default is `4.0` in [../src/evaluator/config.rs](../src/evaluator/config.rs).

| deviation | cost (weight = 1, sharpness = 4) | meaning |
| --------- | --------------------------------- | ------- |
| 0.5       | 0.0625                             | half the limit — nearly free |
| 1.0       | 1.0                                 | exactly at the limit — full weight |
| 1.5       | 5.06                                | 50% over — ~5× weight |
| 2.0       | 16.0                                | double the limit — 16× weight |

- Higher `sharpness` → forgiving under the limit, brutal over it.
- `sharpness = 1.0` → linear.
- Lower `sharpness` → softer wall and smoother trade-offs.

## Reading the breakdown table

`evaluate` and `optimize` emit one row per configured metric for the best layout.

```text
metric                    value     goal     dev        cost   share    pressure
row_switch_ratio           6.20     8.00    0.78      0.3702   12.1%      0.7802
hand_switch_ratio         41.30    38.00    1.09      1.4116   46.2%      0.5343
...
```

- `share` — this term's percent of total penalty right now.
- `pressure` — marginal cost per percentage point of further movement:
  `weight · sharpness · deviation^(sharpness-1) / norm`.
- Low `pressure` + high `share` means the GA is not pushing that metric hard enough. Raise `weight`
  or tighten `value`/`tolerance`.
- Two off-goal metrics with similar pressure are a real physical conflict. Weight changes will not
  resolve it; relax one side instead.

## Metric catalog

All formulas use `ScoreResult` fields from one scored corpus pass. `ratio(a, b) = a / b` (or `0` if
`b = 0`). `signed_imbalance_percent(L, R) = (L / R - 1) * 100`, with `L = R = 0 → 0` and one side
zero clamping to `±100` as a guardrail.

### Hand-level imbalances (opt-in)

| Config field | Formula | What it caps |
| ------------ | ------- | ------------ |
| `effortsImbalance` | `signed_imbalance_percent(left_effort, right_effort)` | total effort skew between hands |
| `handsImbalance` | `signed_imbalance_percent(left_count, right_count)` | raw press-count skew |
| `rollImbalance` | `signed_imbalance_percent(left_rolls, right_rolls)` | same-hand roll count skew |
| `rowSwitchImbalance` | `signed_imbalance_percent(left_row_switch_cost, right_row_switch_cost)` | same-finger row-jump cost skew |
| `streakImbalance` | `signed_imbalance_percent(left_streak, right_streak)` | streak-length skew |

`streak(count, rolls) = count / (count - rolls)` when `count > rolls`, else `0`.

Recommended defaults to enable early: `effortsImbalance` and `handsImbalance` at `value: 5`, `weight:
0.75` and `0.5`.

### Aggregate press-pattern metrics

| Config field | Formula | What it caps |
| ------------ | ------- | ------------ |
| `rowSwitchRatio` | `100 × (left_row_switch_cost + right_row_switch_cost) / (left_count + right_count)` | same-finger vertical row moves |
| `handSwitchRatio` | `100 × hand_switches / (left_count + right_count)` | hand alternation frequency |
| `sfsRatio` | `100 × sfs_count / (left_count + right_count)` | same-finger skipgram share |
| `inwardRatio` | `100 × inward_count / (left_count + right_count)` | same-hand moves toward center |
| `outwardRatio` | `100 × outward_count / (left_count + right_count)` | same-hand moves away from center |
| `directionalOutwardRatio` | `100 × outward_count / (inward_count + outward_count)` | outward share among directional rolls (`0%` = all inward, `50%` = tie) |

`inwardRatio` is usually a `target`; `outwardRatio` and `directionalOutwardRatio` are usually `max` caps.

### Row-distribution targets

| Config field | Formula | Default |
| ------------ | ------- | ------- |
| `topRowRatio` | `100 × (left_row_effort[top] + right_row_effort[top]) / effort` | `target: 25, weight 1, tolerance 5` |
| `homeRowRatio` | `100 × (left_row_effort[home] + right_row_effort[home]) / effort` | `target: 60, weight 1, tolerance 5` |
| `bottomRowRatio` | `100 × (left_row_effort[bottom] + right_row_effort[bottom]) / effort` | `target: 15, weight 1, tolerance 5` |
| `homeRowBalance` | `signed_imbalance_percent(left_row_effort[home], right_row_effort[home])` | `max: 15, weight 1` |

The three ratio defaults sum to `100%` (`25 + 60 + 15`), so they are a real target distribution.
Tightening `homeRowRatio.tolerance` is the strongest lever for “keep typing on the home row.”

### Column distribution and left/right balance

Each column metric applies the same `Target` to both hands independently. In `Targets` the
field names are `pinkyRatio`, `ringRatio`, `middleRatio`, `indexInnerRatio`, and
`indexOuterRatio`; the code emits both `left_*` and `right_*` breakdown rows using the same target.

| Config field | Formula (per hand) | Default |
| ------------ | ------------------ | ------- |
| `pinkyRatio` | `100 × left/right_column_effort[pinky] / effort` | `target: 10, weight 1` |
| `ringRatio` | `100 × left/right_column_effort[ring] / effort` | `target: 10, weight 1` |
| `middleRatio` | `100 × left/right_column_effort[middle] / effort` | `target: 10, weight 1` |
| `indexInnerRatio` | `100 × left/right_column_effort[index-inner] / effort` | `target: 10, weight 1` |
| `indexOuterRatio` | `100 × left/right_column_effort[index-outer] / effort` | `target: 10, weight 1` |

Balance metrics are signed imbalances per finger:

| Config field | Formula |
| ------------ | ------- |
| `pinkyBalance` | `signed_imbalance_percent(left_column_effort[pinky], right_column_effort[pinky])` |
| `ringBalance` | `signed_imbalance_percent(left_column_effort[ring], right_column_effort[ring])` |
| `middleBalance` | `signed_imbalance_percent(left_column_effort[middle], right_column_effort[middle])` |
| `indexInnerBalance` | `signed_imbalance_percent(left_column_effort[index-inner], right_column_effort[index-inner])` |
| `indexOuterBalance` | `signed_imbalance_percent(left_column_effort[index-outer], right_column_effort[index-outer])` |

### Same-finger row-switch metrics

Each per-finger row-switch metric is computed as weighted row-switch cost divided by that finger's
press count. The code merges index inner + outer into a single `index` bucket.

| Config field | Formula |
| ------------ | ------- |
| `pinkyRowSwitchRatio` | `100 × left/right_finger_row_switch_cost[pinky] / left/right_finger_press_count[pinky]` |
| `ringRowSwitchRatio` | same, ring |
| `middleRowSwitchRatio` | same, middle |
| `indexRowSwitchRatio` | same, merged index |

Balance variants are the same signed imbalance on left-vs-right row-switch cost for each finger:

| Config field | Formula |
| ------------ | ------- |
| `pinkyRowSwitchBalance` | `signed_imbalance_percent(left_finger_row_switch_cost[pinky], right_finger_row_switch_cost[pinky])` |
| `ringRowSwitchBalance` | same, ring |
| `middleRowSwitchBalance` | same, middle |
| `indexRowSwitchBalance` | same, merged index |

## Corpus invariance

Every factor is a per-press ratio. Doubling the corpus size leaves the penalty effectively unchanged,
so fitness stays comparable across corpus sizes and reruns. Average word length changes the baseline
slightly, but the metric itself is still well-defined per corpus.

## Signed-imbalance edge case

`signed_imbalance_percent` is generally unbounded above, but it is clamped at exactly `±100%` when
one side is fully idle instead of diverging to infinity. In practice, values near this clamp only
appear in badly broken constraints, such as one hand having no letters at all.

## Tuning workflow

1. Run `evaluate` or `optimize`, then read the champion layout's breakdown table.
2. Sort by `share`; that is where the fitness spend is happening right now.
3. Check `pressure` for the metrics already near their goal. Low pressure + high share means the GA
   is not pushing that metric hard enough.
4. Two off-goal metrics with similar pressure are a real physical conflict. Relax one side instead of
   fighting both.
5. Re-run and repeat.

## Live config example

This is the current project config used in [../keyvolve.yaml](../keyvolve.yaml):

```yaml
evaluator:
  fitnessScale: 100000000000000000
  sharpness: 2

  effortsImbalance: { type: max, value: 5, weight: 0.75 }
  handsImbalance: { type: max, value: 5, weight: 0.5 }
  handSwitchRatio: { type: max, value: 37, weight: 3 }
  sfsRatio: { type: max, value: 6, weight: 1 }
  inwardRatio: { type: target, value: 19, weight: 2 }
  outwardRatio: { type: max, value: 12, weight: 1 }
  directionalOutwardRatio: { type: max, value: 40, weight: 1 }

  rollImbalance: { type: max, value: 5, weight: 0.1 }
  rowSwitchImbalance: { type: max, value: 10, weight: 0.01 }
  streakImbalance: { type: max, value: 10, weight: 0.1 }
  homeRowBalance: { type: max, value: 10, weight: 0.01 }

  topRowRatio: { type: target, value: 24, weight: 1 }
  homeRowRatio: { type: target, value: 64, weight: 2 }
  bottomRowRatio: { type: target, value: 12, weight: 1 }

  pinkyRatio: { type: max, value: 5, weight: 1.0 }
  ringRatio: { type: max, value: 10, weight: 1.0 }
  middleRatio: { type: max, value: 15, weight: 1.0 }
  indexInnerRatio: { type: max, value: 12, weight: 1.0 }
  indexOuterRatio: { type: max, value: 8, weight: 1.0 }

  pinkyBalance: { type: max, value: 10, weight: 0.25 }
  ringBalance: { type: max, value: 10, weight: 0.25 }
  middleBalance: { type: max, value: 10, weight: 0.25 }
  indexInnerBalance: { type: max, value: 10, weight: 0.25 }
  indexOuterBalance: { type: max, value: 10, weight: 0.25 }

  pinkyRowSwitchRatio: { type: max, value: 3, weight: 0.5 }
  ringRowSwitchRatio: { type: max, value: 5, weight: 0.5 }
  middleRowSwitchRatio: { type: max, value: 8, weight: 0.5 }
  indexRowSwitchRatio: { type: max, value: 8, weight: 0.5 }
```

The code intentionally rejects legacy forms such as `{"max": 20, "weight": 1}`; the only valid
shape is the explicit `{"type": "max", "value": 20, "weight": 1}` form.
