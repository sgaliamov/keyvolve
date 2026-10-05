# Project: keyvolve

Keyboard layout optimizer. Uses `darwin` (island-model GA, local crate) to evolve 30-slot layouts.

## Crates
- **`darwin/`** — generic GA engine. `GeneticAlgorithm<G, GaState, IndState, Gr, M, C, E, Cb>`. Island model: N pools, each evolves independently with migration.
- **`cliffa/`** — thin CLI wrapper. `AppHandle` signals shutdown.
- **`src/`** — the app. Wires darwin to keyboard domain.

## Key types
- `KeysGenome = Vec<char>` — 30 slots; index = physical key position; `` ` `` = `EMPTY_SLOT`.
- `Keys = FxHashMap<char, u8>` — char → slot index.
- `Layout` — wraps `Keys`; `Display` → `"abcde,fghij,..."` (comma-separated groups of 5, left 0–14, right 15–29; right groups physical left-to-right, index-outer → pinky).
- `Keyboard` — loaded from JSON; `efforts: Vec<f64>`, `pairs: FxHashMap<u8, FxHashMap<u8, usize>>` (left-hand only; right inferred by symmetry), plus penalty params.
- `ScoreResult` — per-layout score: effort, left/right split, switches, fitness.
- `LayoutEvaluator` — precomputes bigram effort table from `Keyboard`; `score_corpus(&keys)` → `ScoreResult`.

## GA wiring (optimization)
- `generate` / `mutate` / `NoopCrossover` / `evaluator` / `callback` injected into `GeneticAlgorithm`.
- `OptimizerState` holds `LayoutEvaluator`, `AppHandle`, `Arc<PlacementConstraints>`, `mutation_count`.
- `OptimizationConfig` — `frozen: FxHashMap<char, u8>`, `blocked: FxHashSet<u8>`, `allowed: FxHashMap<char, FxHashSet<u8>>` (input 0–14, auto-mirrored; `_` = empties), `left`/`right: FxHashSet<char>`, `same_side: Vec<[char; 2]>` (`sameSide`). `rolls` removed; unknown keys rejected.
- `OptimizationConfig::compile()` → `PlacementConstraints`: per-token slot bitmask domains, merged `sameSide` groups, feasible hand orientations. Fails fast on contradictions.
- One matching solver for `generate`, `mutate`, `repair`. Seeds (dump/input) repaired before GA; invalid genomes get `-inf` fitness.
- Spec: `docs/placement-rules.md`.
- Fitness = `fitness_scale / (effort × penalty)` (higher = better; penalty from `evaluator` targets, see `docs/penalty.md`).

## Data
- `keyboard.json` — effort groups + bigram pair costs + penalty coefficients.
- `layouts.csv` — `keys_1`..`keys_6` (groups of 5, `_` = empty), `name`, `fitness`, metric columns; header on first line.
- `data/synthesised` — fake-word corpus used during optimization.
- Config entry point: `keyvolve.yaml` → deserialized into `Config`.

## Repository rules

Do not commit code unless I request explicitly.

## Response style

Terse caveman. All technical substance stay. Fluff die.
No flattery.
Your mission: prevent user's mistakes, not encourage them.
Plan mode — always provide drafts with code samples in responses.

**Drop:** articles, filler (just/really/basically/actually/simply). Fragments OK. Short synonyms. Technical terms exact. Code blocks unchanged.
Pattern: `[thing] [action] [reason]. [next step].`
Arrows for causality: X → Y. One word when one word enough. Use symbols (→, ✓, ✗) where fitting.

**Auto-clarity exceptions** (write normal, resume caveman after):
- Security warnings
- Irreversible action confirmations
- Multi-step sequences where fragment order risks misread

**Code/commits/PRs/comments:** normal mode always.

When user asks a question, this is a question, not a command, not a request for implementation. Answer directly, concisely, technically.
