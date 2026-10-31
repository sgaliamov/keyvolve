# keyvolve

Keyboard layout optimizer. Evolves 30-slot layouts using an island-model genetic algorithm.

## What it does

Scores candidate layouts against a bigram-weighted corpus, then evolves them toward lower effort, balanced hand use, and good roll patterns.

**Fitness = fitnessScale / (effort × penalty)** (higher = better). The penalty is a
dimensionless multiplier built from corpus metrics — see below.

## Quick start

Install Rust and Cargo, then run these PowerShell commands from the repository root:

```powershell
.\scripts\run.ps1 -m optimize
.\scripts\run.ps1 -m evaluate --evaluate.eSide left
```

The first command searches for layouts using the settings in `keyvolve.yaml`; the second scores and displays the saved layouts. The script builds a release version before running. The supplied keyboard effort data reflects the author's keyboard and preferences.

To repeat the search ten times instead, use:

```powershell
.\scripts\run.ps1 -r 10 -m optimize
.\scripts\run.ps1 -m evaluate --evaluate.eSide left
```

Repeated runs provide more results to compare: similar scores or layouts across runs can suggest the search is stable, while different results may reveal other promising regions. This does not prove a global optimum or guarantee that the search has escaped local optima. With the current configuration, completed optimization runs append to `data/layouts.csv`; evaluation reads that file and rewrites it in place. Back it up first if you want to preserve its current contents.

`--evaluate.eSide left` normalizes layouts so that `e` is on the left hand and removes duplicate layouts, including mirrored pairs. Use `right` instead if you prefer `e` on the right hand. Mirroring a layout can violate asymmetric placement constraints; evaluation does not check those constraints.

## PowerShell scripts

Run scripts from the repository root:

| Command | Purpose |
| --- | --- |
| `.\scripts\run.ps1 -m optimize` | Build and run in release mode. Pass other mode and configuration options through to the application. |
| `.\scripts\run.ps1 -r 10 -m optimize` | Run the command ten times sequentially. |
| `.\scripts\run.ps1 -d -m optimize` | Build and run in debug mode with full backtraces and one Rayon thread. |
| `.\scripts\test.ps1` | Run workspace documentation and target tests offline; dependencies must already be available locally. |
| `.\scripts\lint.ps1` | Apply Cargo and Clippy fixes, run Clippy, and format code. This modifies files. |
| `.\scripts\update.ps1` | Update the Rust toolchain and Cargo dependencies. Add `-u` to run `cargo upgrade` as well; it requires `cargo-edit`. |
| `.\scripts\submodules.ps1` | Initialize submodules recursively and update them from their configured remote branches. |

## Key particularities

### Layout representation
- 30 physical slots: left hand slots 0–14, right hand 15–29 (3 rows × 5 cols per hand).
- Genome: `Vec<char>` — index = physical slot, value = character. `` ` `` marks empty slots.
- Display format: `"abcde;fghij;klmno;pqrst;uvwxy;z____"` — semicolons separate rows, left before right.

### Scoring
- Bigram effort table precomputed from `keyboard.json`: per-key effort groups + pair costs + symmetry (left-hand pairs mirrored to right automatically).
- Corpus penalty: one limit per metric in the percent units the CSV prints:
  `penalty = 1 + Σ weight · (|value| / max) ^ sharpness`. Every metric on target → `1.0`.
  `max` normalizes metrics against each other; `weight` (default 1) only decides which
  metric gives way first. A `handSwitchRatio` limit replaces the old `meanStreakPower`,
  since `mean_streak = presses / (hand switches + words)`.
- Corpus: synthesised fake-word file (built from real text via `Synthesise` mode), not raw text — keeps evaluation fast.

### GA engine (darwin crate)
- Island model: N independent pools, configurable migration, parallel evaluation via Rayon.
- Sigma annealing: Gaussian mutation noise decays `sigma.max → sigma.min` over generations.
- Stagnation detection: auto-halts when fitness plateaus.
- Operators injected as closures: `generate`, `mutate`, `NoopCrossover`, `corpus_evaluator`, `callback`.

### Constraint system
- `frozen`: pin specific characters to specific slots.
- `blocked`: exclude slots from use entirely (e.g. thumb keys).
- `allowed`: restrict a character to a set of half-positions (auto-mirrored to both hands).
- `rolls`: force character pairs onto adjacent same-hand, neighboring-row slots ("roll" positions).
- Generator enforces all constraints; invalid genomes never enter the pool.

### Modes
| Mode          | Description                                        |
| ------------- | -------------------------------------------------- |
| `optimize`    | Run GA, append results to `layouts.csv`            |
| `evaluate`    | Score layout CSVs, print best + penalty breakdown  |
| `synthesise`  | Build digraph CSV + fake-word corpus from raw text |
| `merge`       | Merge/clean `.txt` files into one corpus           |
| `stats`       | Build corpus stats JSON used by evaluate/optimize  |
| `frequencies` | Count per-character frequencies across text files  |
| `rank`        | Interactively calibrate ordered-pair effort groups |

## Mode-specific config

### `optimize`
- `optimization.input` / `optimization.output` — seed layouts CSV / final layouts CSV.
- `optimization.frozen`, `blocked`, `allowed`, `sameSide`, `left`, `right` — placement constraints.

Details: [docs/modes/optimize.md](docs/modes/optimize.md). Placement:
[docs/placement-rules.md](docs/placement-rules.md).

### `evaluate`
- `evaluate.input` — array of layouts CSVs to score.
- `evaluate.output` — combined scored CSV. Omitted → each input file rewritten in place.
- `evaluate.print` — number of best layouts printed to stdout. Default: `10`.
- `evaluate.eSide` — hand `e` is mirrored to on save: `left` (default), `right`, `any`.

Details: [docs/evaluation/evaluate.md](docs/evaluation/evaluate.md). Scoring:
[docs/evaluation/scoring.md](docs/evaluation/scoring.md), [docs/evaluation/penalty.md](docs/evaluation/penalty.md).

### `merge`
- `merge.input` — folder containing `.txt` files.
- `merge.output` — merged cleaned corpus file.
- `merge.shuffle` — randomize word order. Default: `false`.
- `merge.seed` — optional reproducible shuffle seed.

Details: [docs/modes/merge.md](docs/modes/merge.md).

### `stats`
- `stats.input` — corpus text file, usually `merge.output`.
- `stats.output` — output folder; writes `stats.json` + CSV reports. Set `evaluate.corpusStats` / `optimization.corpusStats` to `<output>/stats.json`.
- `stats.minTrigramFrequency` — drop rarer trigrams. Default: `0.0001`.

Details: [docs/modes/stats.md](docs/modes/stats.md).

### `rank`
- `rank.session` — resumable answer history. Saved atomically after each answer.
- `rank.output` / `rank.report` — generated keyboard JSON and analytical CSV.
- `rank.auditRate` — audit probability during refinement; finished sessions always audit.
- `rank.minMatches` / `rank.maxMatches` — confidence floor and hard per-item cap.
- `rank.maxDeviation` — maximum marginal rating uncertainty before confidence stopping.
- `rank.forcedAnswerWeight` — confirmations recorded by one `!` answer; saves re-answering the same pair.
- `rank.seed` — optional reproducible question-order seed.

Details: [docs/modes/rank.md](docs/modes/rank.md).

Diagnostic tests (`#[ignore]`, read-only over the live `data/rank-session.json`):

```sh
# Preference cycles among majority edges:
cargo test -q scan_live_session_for_cycles -- --ignored --nocapture
```

## Data files
- `data/keyboard.json` — effort groups, bigram pair costs, penalty coefficients.
- `data/layouts.csv` — semicolon-layout + fitness; header on first line.
- `data/synthesised` — fake-word corpus used during optimization.
- `keyvolve.yaml` — top-level config (mode, GA params, constraints; mode configs own their paths).

## Crates
- **`darwin/`** — generic GA engine, no domain knowledge.
- **`cliffa/`** — thin CLI wrapper; `AppHandle` signals graceful shutdown.
- **`src/`** — keyboard domain: models, evaluator, GA wiring, modes.
