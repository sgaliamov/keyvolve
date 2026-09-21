# Penalty model — configurable scoring knobs

The optimizer doesn't just minimize raw typing effort — it also cares about balance between
hands, how often fingers jump rows, how much load lands on the pinky, and a dozen other
ergonomic concerns. Each of those concerns is a metric you can cap or target in the
`evaluator` config section, and this document is the reference for all of them: the formula
behind each one, its default, and what happens when you turn its dials.

[keyvolve.yaml](../keyvolve.yaml) carries a short one-line reminder per metric; come here for
the full formula and the reasoning behind the defaults.

## Fitness and penalty

```text
fitness = fitness_scale / (effort × penalty)
penalty = 1 + Σ weight · deviation^sharpness
```

- `effort` — raw bigram cost from `keyboard.json`'s pair table. Layout-shape only; unaffected by any target.
- `penalty` — dimensionless multiplier ≥ 1. Every metric on its goal → `penalty = 1.0` → fitness falls back to the effort-only ideal.
- The leading `1` is the multiplicative identity, not a fixed cost. It also guards the division: without it, a near-zero penalty could send fitness toward infinity and drown out effort entirely.
- Higher `fitness` = better layout. Configured via `fitnessScale` (default `1e6`), which only sets display magnitude — it does not change *ranking* between layouts.

## Target types

Every metric is configured as one `Target`:

```yaml
metricName: { type: max|target, value: <number>, weight: <number>, tolerance: <number> }
```

| Field       | Meaning                                                                 |
| ----------- | ------------------------------------------------------------------------ |
| `type`      | `max` — lower is better, `value` is the accepted ceiling. `target` — closer is better, `value` is the desired point. |
| `value`     | The ceiling (`max`) or desired point (`target`), in the **percent units the CSV prints**. |
| `weight`    | Priority against other metrics. Default `1`. |
| `tolerance` | `target` only — accepted miss in percentage points before cost ramps up. Default `5`. Ignored by `max`. |

Deviation (normalized, `0` = ideal, `1` = accepted edge):

```text
max:    deviation = |value| / target.value
target: deviation = |value - target.value| / tolerance
```

`norm` (used for pressure, below): `target.value` for `max`, `tolerance` for `target`.

**A metric resting exactly at its edge (the limit, or `tolerance` points from the target)
costs exactly its own `weight`.** That's what makes `value`/`tolerance` a pure normalizer and
`weight` a pure priority knob — they don't interact.

## Sharpness

```text
cost = weight · deviation^sharpness
```

`sharpness` (config-wide, default `4`) shapes every term's curve identically:

| deviation | cost (weight = 1, sharpness = 4) | meaning |
| --------- | --------------------------------- | ------- |
| 0.5       | 0.0625                             | half the limit — nearly free |
| 1.0       | 1.0                                 | exactly at the limit — costs full weight |
| 1.5       | 5.06                                | 50% over — costs 5× weight |
| 2.0       | 16.0                                | double the limit — costs 16× weight |

- Higher `sharpness` → more forgiving under the limit, more brutal over it (a hard wall).
- `sharpness = 1` → linear, no forgiveness zone.
- Lower `sharpness` → softer wall, metrics trade off more smoothly against each other.

Raising global `sharpness` makes the optimizer treat every configured limit as closer to a
hard constraint instead of a soft preference.

## Reading the breakdown table

`evaluate`/`optimize` log one row per configured metric for the best layout:

```text
metric                    value     goal     dev        cost   share    pressure
row_switch_ratio           6.20     8.00    0.78      0.3702   12.1%      0.7802
hand_switch_ratio         41.30    38.00    1.09      1.4116   46.2%      0.5343
...
```

- **share** — this term's percent of total penalty right now — who's paying.
- **pressure** — marginal cost per percentage point of further movement (`weight · sharpness · deviation^(sharpness-1) / norm`) — who *would* pay if things got slightly worse, i.e. who the GA is most motivated to fix next.
- A metric sitting off-goal with **low pressure** relative to others is being ignored by the GA — raise its `weight` or tighten `value`/`tolerance`.
- Two off-goal metrics with **similar pressure** signal a genuine physical trade-off (e.g. home-row share vs. pinky load) — no weight tweak resolves it, only relaxing one target does.

## Metric catalog

All formulas use `ScoreResult` fields from one scored corpus pass. `ratio(a, b) = a/b` (or `0`
if `b = 0`). `signed_imbalance_percent(L, R) = (L/R − 1) × 100`, with `L=R=0 → 0`, one side `0`
→ `±100` as an edge case (see note below).

### Hand-level imbalances (no built-in default — opt-in)

| Config field           | Formula                                                        | What it caps |
| ----------------------- | ---------------------------------------------------------------- | -------------- |
| `effortsImbalance`      | `signed_imbalance_percent(left_effort, right_effort)`             | Total effort skew between hands — the main "is one hand doing more work" signal. |
| `handsImbalance`        | `signed_imbalance_percent(left_count, right_count)`               | Raw keystroke-count skew, independent of effort weighting. |
| `rollImbalance`         | `signed_imbalance_percent(left_rolls, right_rolls)`                | Skew in same-hand consecutive bigrams ("rolls") between hands. |
| `rowSwitchImbalance`    | `signed_imbalance_percent(left_row_switch_cost, right_row_switch_cost)` | Skew in same-finger row-jump cost between hands. |
| `streakImbalance`       | `signed_imbalance_percent(left_streak, right_streak)`              | Skew in average run length (sustained same-hand sequences). |

`streak(count, rolls) = count / (count − rolls)` when `count > rolls`, else `0`.

**Recommended:** `effortsImbalance` and `handsImbalance` are the two you almost always want
(project config: `value: 5, weight: 0.75` and `value: 5, weight: 0.5`). The others are finer
knobs — enable only if the breakdown table shows one of them dominating `share`.

### Aggregate press-pattern metrics (no built-in default)

| Config field        | Formula                                                                 | What it caps |
| -------------------- | -------------------------------------------------------------------------- | -------------- |
| `rowSwitchRatio`     | `100 × (left_row_switch_cost + right_row_switch_cost) / (left_count + right_count)` | How often, on average, a same-finger press needs a vertical row move (adjacent row = 1, skip-row = 2). |
| `handSwitchRatio`    | `100 × hand_switches / (left_count + right_count)`                          | Hand-alternation frequency. Replaces the old `meanStreakPower` — see derivation in [penalty.rs](/c:/Users/Admin/projects/keyvolve/src/evaluator/penalty.rs) module docs: `mean_streak = presses / (switches + words)`, so both are monotone in the same variable. |

**Recommended:** project config only sets `handSwitchRatio: { value: 38, weight: 2 }`
(`rowSwitchRatio` left unset — the per-finger row-switch caps below cover it more precisely).

### Row-distribution targets (effort share by row)

| Config field     | Formula                                                          | Default          |
| ----------------- | ------------------------------------------------------------------- | ------------------ |
| `topRowRatio`     | `100 × (left_row_effort[top] + right_row_effort[top]) / effort`       | `target: 25, weight 1, tolerance 5` |
| `homeRowRatio`    | `100 × (left_row_effort[home] + right_row_effort[home]) / effort`     | `target: 60, weight 1, tolerance 5` |
| `bottomRowRatio`  | `100 × (left_row_effort[bottom] + right_row_effort[bottom]) / effort` | `target: 15, weight 1, tolerance 5` |
| `homeRowBalance`  | `signed_imbalance_percent(left_row_effort[home], right_row_effort[home])` | `max: 15, weight 1` |

The three ratio defaults sum to exactly 100% (25+60+15) — a real target distribution, not
independent caps. Tightening `tolerance` on `homeRowRatio` is the single strongest lever for
"most typing should stay on the home row."

### Column (finger) distribution targets — effort share per finger

Each field configures **two** breakdown terms: `left_<finger>_ratio` and `right_<finger>_ratio`
— the same `Target` applied to both hands independently (not a combined two-hand total).

| Config field        | Formula (per hand)                                    | Default             |
| --------------------- | ----------------------------------------------------------- | ---------------------- |
| `pinkyRatio`          | `100 × {left,right}_column_effort[pinky] / effort`         | `target: 10, weight 1` |
| `ringRatio`           | same, ring column                                          | `target: 10, weight 1` |
| `middleRatio`         | same, middle column                                        | `target: 10, weight 1` |
| `indexInnerRatio`     | same, index-inner column                                   | `target: 10, weight 1` |
| `indexOuterRatio`     | same, index-outer column                                   | `target: 10, weight 1` |

The built-in default is a flat 10% goal for every column on every hand (10 columns × 10% =
100%). It is a neutral starting point, not a physiologically-graded one.

**Recommended:** override all five with `type: max` instead of the `target` default —
asymmetric ceilings that respect finger strength, e.g. `pinkyRatio: { max, 6 }`,
`ringRatio: { max, 10 }`, `middleRatio: { max, 13 }`, `indexInnerRatio: { max, 12 }`,
`indexOuterRatio: { max, 9 }`. This lets the effort map decide placement freely below the cap
and only pushes back once a finger is overloaded — softer than pinning every finger to an
exact point.

### Column (finger) left/right balance

| Config field           | Formula                                                              | Default             |
| ------------------------ | ------------------------------------------------------------------------ | ---------------------- |
| `pinkyBalance`           | `signed_imbalance_percent(left_column_effort[pinky], right_column_effort[pinky])` | `max: 15, weight 0.5` |
| `ringBalance`            | same, ring                                                                 | `max: 15, weight 0.5` |
| `middleBalance`          | same, middle                                                               | `max: 15, weight 0.5` |
| `indexInnerBalance`      | same, index-inner                                                          | `max: 15, weight 0.5` |
| `indexOuterBalance`      | same, index-outer                                                          | `max: 15, weight 0.5` |

**Recommended:** project config tightens all five to `value: 10, weight: 0.25` — a softer
ceiling with lower priority than the ratio caps above, since exact per-finger L/R symmetry
matters less than the overall effort split.

### Row-switch metrics, per finger — merged index (inner+outer counted together)

Formula source: `left_finger_row_switch_ratio[f] = left_finger_row_switch_cost[f] / left_finger_press_count[f]`, `f ∈ {pinky, ring, middle, index}`.

| Config field                | Formula                                                | Default              |
| ------------------------------ | ----------------------------------------------------------- | ----------------------- |
| `pinkyRowSwitchRatio`          | `100 × {left,right}_finger_row_switch_cost[pinky] / {left,right}_finger_press_count[pinky]` | `max: 8, weight 0.75` |
| `ringRowSwitchRatio`           | same, ring                                                    | `max: 8, weight 0.75` |
| `middleRowSwitchRatio`         | same, middle                                                  | `max: 7, weight 0.75` |
| `indexRowSwitchRatio`          | same, merged index                                            | `max: 7, weight 0.75` |

Per-finger caps exist because a healthy aggregate `rowSwitchRatio` can still hide one
overloaded finger (typically the pinky) — this is exactly the failure case called out in
[readme.md](/c:/Users/Admin/projects/keyvolve/readme.md).

**Recommended:** project config tightens to pinky/ring `3%`/`5%`, middle/index `8%`/`8%`,
`weight: 0.5` — a pinky ceiling roughly half the others.

### Row-switch left/right balance, per finger (no built-in default)

| Config field                   | Formula                                                                |
| ---------------------------------- | --------------------------------------------------------------------------- |
| `pinkyRowSwitchBalance`           | `signed_imbalance_percent(left_finger_row_switch_cost[pinky], right_finger_row_switch_cost[pinky])` |
| `ringRowSwitchBalance`            | same, ring |
| `middleRowSwitchBalance`          | same, middle |
| `indexRowSwitchBalance`           | same, merged index |

**Recommended:** low weight (project config `value: 5, weight: 0.1` for all four) — a
tie-breaker, not a driver.

## Corpus invariance

Every factor is a per-press ratio, so doubling the corpus size leaves the penalty unchanged —
fitness stays comparable across corpus sizes and reruns. Average word length (`W/P`) shifts
`hand_switch_ratio`'s baseline slightly across corpora with different average word length,
but the metric itself stays well-defined per corpus.

## Signed-imbalance edge case

`signed_imbalance_percent` is generally unbounded above (`(L/R − 1) × 100` grows without limit
as `L ≫ R`), but is bounded at exactly `±100%` for a fully-idle hand (`R = 0` clamps to `+100`,
`L = 0` clamps to `−100`) rather than diverging to infinity. In practice you'll only see values
near this clamp for badly broken constraint setups (e.g. one hand accidentally has no letters).

## Tuning workflow

1. Run `evaluate` or `optimize`, read the logged breakdown table for the champion layout.
2. Sort mentally by `share` — that's where the fitness is being spent *right now*.
3. Check `pressure` for the metrics near their goal already — low pressure + high `share` means
   raise `weight` or tighten `value`/`tolerance` (it's not being pushed hard enough).
4. Two metrics with equal `pressure`, both off-goal → physical conflict. Loosen one
   (`value`, `tolerance`, or `weight` down) rather than fighting both.
5. Re-run. Repeat.

## Full annotated example

The project's own working config (values it actually optimizes against):

```yaml
evaluator:
  fitnessScale: 100000000000000000
  sharpness: 4

  # Left/right effort asymmetry — the main hand-balance signal.
  effortsImbalance: { type: max, value: 5, weight: 0.75 }
  # Left/right same-hand bigram count asymmetry — press count, not effort.
  handsImbalance: { type: max, value: 5, weight: 0.5 }
  # Hand-switches per same-hand bigram — caps alternation frequency.
  handSwitchRatio: { type: max, value: 38, weight: 2 }
  # Left/right roll count asymmetry.
  rollImbalance: { type: max, value: 5, weight: 1 }
  # Left/right row-switch cost asymmetry — low weight, tie-breaker only.
  rowSwitchImbalance: { type: max, value: 10, weight: 0.01 }
  # Left/right streak-length asymmetry — low weight, tie-breaker only.
  streakImbalance: { type: max, value: 10, weight: 0.1 }
  # Home row left/right effort balance — low weight, tie-breaker only.
  homeRowBalance: { type: max, value: 10, weight: 0.01 }

  # Row effort distribution: most typing should stay on the home row.
  topRowRatio: { type: target, value: 24, weight: 1 }
  homeRowRatio: { type: target, value: 64, weight: 2 }
  bottomRowRatio: { type: target, value: 12, weight: 1 }

  # Per-column effort caps, weakest finger gets the lowest ceiling.
  pinkyRatio: { type: max, value: 6, weight: 1.0 }
  ringRatio: { type: max, value: 10, weight: 1.0 }
  middleRatio: { type: max, value: 13, weight: 1.0 }
  indexInnerRatio: { type: max, value: 12, weight: 1.0 }
  indexOuterRatio: { type: max, value: 9, weight: 1.0 }

  # Per-finger left/right effort balance.
  pinkyBalance: { type: max, value: 10, weight: 0.25 }
  ringBalance: { type: max, value: 10, weight: 0.25 }
  middleBalance: { type: max, value: 10, weight: 0.25 }
  indexInnerBalance: { type: max, value: 10, weight: 0.25 }
  indexOuterBalance: { type: max, value: 10, weight: 0.25 }

  # Per-finger row-switch load caps — catches an overloaded pinky/ring even
  # when the aggregate rowSwitchRatio looks fine.
  pinkyRowSwitchRatio: { type: max, value: 3, weight: 0.5 }
  ringRowSwitchRatio: { type: max, value: 5, weight: 0.5 }
  middleRowSwitchRatio: { type: max, value: 8, weight: 0.5 }
  indexRowSwitchRatio: { type: max, value: 8, weight: 0.5 }

  # Per-finger row-switch left/right balance — tie-breakers only.
  pinkyRowSwitchBalance: { type: max, value: 5, weight: 0.1 }
  ringRowSwitchBalance: { type: max, value: 5, weight: 0.1 }
  middleRowSwitchBalance: { type: max, value: 5, weight: 0.1 }
  indexRowSwitchBalance: { type: max, value: 5, weight: 0.1 }
```
