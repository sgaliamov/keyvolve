# keyvolve

Keyboard layout optimizer. Evolves 30-slot layouts using an island-model genetic algorithm.

## What it does

Scores candidate layouts against a bigram-weighted corpus, then evolves them toward lower effort, balanced hand use, and good roll patterns.

**Fitness = effort + hand-imbalance penalty + switch-rate penalty** (lower = better).

## Key particularities

### Layout representation
- 30 physical slots: left hand slots 0–14, right hand 15–29 (3 rows × 5 cols per hand).
- Genome: `Vec<char>` — index = physical slot, value = character. `` ` `` marks empty slots.
- Display format: `"abcde;fghij;klmno;pqrst;uvwxy;z____"` — semicolons separate rows, left before right.

### Scoring
- Bigram effort table precomputed from `keyboard.json`: per-key effort groups + pair costs + symmetry (left-hand pairs mirrored to right automatically).
- Per-bigram penalties: same-hand switch multiplier, corpus-level hand-balance penalty, hand-switch-rate penalty.
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
| Mode         | Description                                        |
| ------------ | -------------------------------------------------- |
| `optimize`   | Run GA, append results to `layouts.csv`            |
| `evaluate`   | Score one layout, print full breakdown             |
| `synthesise` | Build digraph CSV + fake-word corpus from raw text |
| `merge`      | Merge/clean `.txt` files into one corpus           |
| `frequencies`| Count per-character frequencies across text files  |
| `rank`       | Interactively calibrate ordered-pair effort groups  |

## Mode-specific config

### `evaluate`
- `evaluate.input` — layouts CSV to score. Falls back to top-level `layouts` for compatibility.
- `evaluate.output` — destination CSV for scored layouts. Omitted → overwrite `evaluate.input`.
- `evaluate.print` — number of best layouts printed to stdout. Default: `10`.

### `merge`
- `merge.input` — folder containing `.txt` files.
- `merge.output` — merged cleaned corpus file.

### `rank`
- `rank.session` — resumable answer history. Saved atomically after each answer.
- `rank.output` / `rank.report` — generated keyboard JSON and analytical CSV.
- `rank.auditRate` — audit probability during refinement; finished sessions always audit.
- `rank.minMatches` / `rank.maxMatches` — confidence floor and hard per-item cap.
- `rank.maxDeviation` — maximum marginal rating uncertainty before confidence stopping.
- `rank.groups` — output effort bucket count.
- `rank.bucketTolerance` — neighboring buckets allowed by confidence stopping.
- `rank.seed` — optional reproducible question-order seed.

## Data files
- `data/keyboard.json` — effort groups, bigram pair costs, penalty coefficients.
- `data/layouts.csv` — semicolon-layout + fitness; header on first line.
- `data/synthesised` — fake-word corpus used during optimization.
- `keyvolve.yaml` — top-level config (mode, GA params, constraints, paths).

## Crates
- **`darwin/`** — generic GA engine, no domain knowledge.
- **`cliffa/`** — thin CLI wrapper; `AppHandle` signals graceful shutdown.
- **`src/`** — keyboard domain: models, evaluator, GA wiring, modes.
