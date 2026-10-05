# Penalty model

Scoring knobs in the `evaluator` section of [keyvolve.yaml](../keyvolve.yaml). Code:
[target.rs](../src/models/target.rs), [targets.rs](../src/models/targets.rs),
[penalty.rs](../src/evaluator/penalty.rs), [score.rs](../src/models/score.rs).

## Fitness

```text
fitness = fitness_scale / (effort × penalty)
penalty = 1 + Σ weight · deviation^sharpness
```

- `effort` — raw bigram cost from the keyboard pairs table. Targets don't touch it.
- `penalty` — multiplier `>= 1.0`. All metrics on goal → `1.0` → effort-only fitness.
- `fitnessScale` — display magnitude only; doesn't reorder layouts.

## Targets

Every metric is opt-in. No metric has a built-in default; unset → no penalty term.

```yaml
metricName: { type: max|target, value: <number>, weight: <number>, tolerance: <number> }
```

| Field | Meaning |
| ----- | ------- |
| `type` | `max`: lower is better, `value` = ceiling. `target`: closer is better, `value` = goal point. |
| `value` | Ceiling or goal, in the percent units the CSV prints. |
| `weight` | Priority. Default `1`. |
| `tolerance` | `target` only: accepted miss in percentage points. Default `5`. |

```text
max:    deviation = |value| / target.value        norm = target.value
target: deviation = |value - target.value| / tolerance   norm = tolerance
```

Deviation `1.0` = accepted edge → costs exactly `weight`. So `value`/`tolerance` normalize,
`weight` prioritizes.

Only the explicit form is accepted; legacy `{max: 20, weight: 1}` fails to parse.

## Sharpness

Config-wide exponent. Default `4.0` ([config.rs](../src/evaluator/config.rs)).

| deviation | cost (weight 1, sharpness 4) |
| --------- | ---------------------------- |
| 0.5 | 0.0625 |
| 1.0 | 1.0 |
| 1.5 | 5.06 |
| 2.0 | 16.0 |

Higher → forgiving under the edge, brutal over it. `1.0` → linear.

## Breakdown table and tuning

`evaluate` and `optimize` print one row per configured metric for the best layout:

```text
metric                    value     goal     dev        cost   share    pressure
row_switch_ratio           6.20     8.00    0.78      0.3702   12.1%      0.7802
hand_switch_ratio         41.30    38.00    1.09      1.4116   46.2%      0.5343
```

- `share` — term's percent of total penalty.
- `pressure` — marginal cost per percentage point: `weight · sharpness · deviation^(sharpness-1) / norm`.

Tuning loop:

1. Sort by `share` → where the penalty goes now.
2. High `share` + low `pressure` → GA not pushing it. Raise `weight` or tighten `value`/`tolerance`.
3. Two off-goal metrics with similar `pressure` → physical conflict. Weights won't fix it; relax one.
4. Re-run.

## Metric catalog

`P = left_count + right_count` (all presses). `ratio(a, b) = a / b`, `0` if `b = 0`.

Two imbalance forms:

```text
signed_imbalance_percent(L, R) = (L / R − 1) · 100     L = R = 0 → 0; L = 0 → −100; R = 0 → +100
balance(L, R)                  = (L − R) / (L + R) · 100   L + R = 0 → 0; range [−100, 100]
```

`signed_imbalance_percent` is asymmetric: `L = 2R` → `+100`, `R = 2L` → `−50`. Since `max`
uses `|value|`, left-heavy skew costs more than equal right-heavy skew. Its `R = 0 → +100`
case is a guardrail, not the limit (true limit is `+∞`).

### Hand imbalances

| Config field | Formula |
| ------------ | ------- |
| `effortsImbalance` | `signed_imbalance_percent(left_effort, right_effort)` |
| `handsImbalance` | `signed_imbalance_percent(left_count, right_count)` |
| `rollImbalance` | `signed_imbalance_percent(left_rolls, right_rolls)` |
| `rowSwitchImbalance` | `signed_imbalance_percent(left_row_switch_cost, right_row_switch_cost)` |
| `streakImbalance` | `signed_imbalance_percent(left_streak, right_streak)` |

`streak(count, rolls) = count / (count − rolls)`, `0` if `count <= rolls`.

### Press patterns

| Config field | Formula |
| ------------ | ------- |
| `rowSwitchRatio` | `100 × (left_row_switch_cost + right_row_switch_cost) / P` |
| `handSwitchRatio` | `100 × hand_switches / P` |
| `sfsRatio` | `100 × sfs_count / P` — see [trigram-metrics.md](trigram-metrics.md) |
| `directionalOutwardRatio` | `100 × outward_count / (inward_count + outward_count)` — `0%` all inward, `50%` tie |

### Rows

| Config field | Formula |
| ------------ | ------- |
| `topRowRatio` | `100 × (left_row_effort[top] + right_row_effort[top]) / effort` |
| `homeRowRatio` | `100 × (left_row_effort[home] + right_row_effort[home]) / effort` |
| `bottomRowRatio` | `100 × (left_row_effort[bottom] + right_row_effort[bottom]) / effort` |
| `homeRowBalance` | `signed_imbalance_percent(left_row_effort[home], right_row_effort[home])` |

Row ratios sum to `100%`; use `type: target` with goals that also sum to `100`.

### Columns

One target per finger, applied to each hand separately (breakdown shows `left_*` and `right_*`
rows). Denominator is total effort of both hands.

| Config field | Formula (per hand) |
| ------------ | ------------------ |
| `pinkyRatio` | `100 × column_effort[pinky] / effort` |
| `ringRatio` | `100 × column_effort[ring] / effort` |
| `middleRatio` | `100 × column_effort[middle] / effort` |
| `indexInnerRatio` | `100 × column_effort[index-inner] / effort` |
| `indexOuterRatio` | `100 × column_effort[index-outer] / effort` |

`pinkyBalance`, `ringBalance`, `middleBalance`, `indexInnerBalance`, `indexOuterBalance` =
`balance(left_column_effort[f], right_column_effort[f])`.

### Same-finger row switches

Index inner + outer merged into one `index` finger. Per-hand ratios, one shared target.

| Config field | Formula (per hand) |
| ------------ | ------------------ |
| `pinkyRowSwitchRatio` | `100 × finger_row_switch_cost[pinky] / finger_press_count[pinky]` |
| `ringRowSwitchRatio` | same, ring |
| `middleRowSwitchRatio` | same, middle |
| `indexRowSwitchRatio` | same, index |

`pinkyRowSwitchBalance`, `ringRowSwitchBalance`, `middleRowSwitchBalance`,
`indexRowSwitchBalance` = `balance(left_finger_row_switch_cost[f], right_finger_row_switch_cost[f])`.

## Corpus invariance

Every term is a per-press ratio → corpus size doesn't change the penalty. Average word length
shifts the baseline slightly across different corpora.
