# Placement rules — current behavior

This document describes how optimizer placement constraints are compiled and enforced.

## Model

- Layout size: 30 slots.
- Symbols: 26 lowercase letters (`a..z`) + 4 empty slots.
- Internal empty marker: `` ` `` (`EMPTY_SLOT`).
- Config can use `_` to configure empty-slot allowed domains.

## Constraint inputs

- `frozen: { char: slot }` — hard pin for a character.
- `blocked: [slot]` — hard-empty slots.
- `allowed: { char: [slots] }` — hard domain restriction for a character.
  - For indices `< 15`, mirrored right-hand slots are added automatically.
  - For `_`, this configures optional allowed empty positions.
- `left: [chars]` / `right: [chars]` — hard side restriction.
- `sameSide: ["ab", ...]` — character pairs that must be on same side.
  - Pairs are independent (one pair may land left, another right).
  - Overlaps are transitive components (`th` + `st` => `t,h,s` together).

## Precedence and hard rules

1. `frozen` wins for pinned keys.
2. Frozen positions are exclusive for pinned keys.
3. Effective blocked positions are forced empty unless that slot is frozen.
4. Non-frozen letters must satisfy domain and side constraints.
5. Same-side components must be placed entirely on one hand.

Notes:

- `allowed["_"]` is permissive, not exact-fill: listed slots may be empty, not must.
- Effective blocked slots may stay empty even if not listed in `allowed["_"]`.
- Soft preferences (like contiguity) never override hard constraints.

## Compilation

Before GA starts, constraints are compiled into:

- per-token slot domains (bitmasks),
- same-side connected components,
- feasible component-hand orientation combinations.

Compilation fails fast on contradictions (example: impossible domains, conflicting frozen same-side group).

## Placement solver

One augmenting-path matching engine is used for all placement operations:

- generation,
- local mutation,
- stale/invalid seed repair.

The solver matches all 30 tokens to slots under compiled domains.
If no full assignment exists, operation fails (or config is rejected at compile time).

## Generation

1. Pick a feasible orientation combination for same-side components.
2. Build domains for all letters and 4 empty tokens.
3. Solve full assignment with matching.

Result is always a complete valid 30-slot genome.

## Mutation

1. If parent is invalid, repair first.
2. Select mutable units (same-side component units + single letters), respecting frozen keys.
3. Release selected units; keep other placements fixed.
4. Re-run matching for released units with all empty tokens available.
5. Try bounded attempts to produce a changed valid candidate.

Mutation never returns invalid genome.
No-change mutation is allowed when no alternative valid placement is found.

## Seed/dump repair

Imported/resumed layouts are repaired through the same solver:

- keep recognizable existing placements when possible,
- reconstruct missing/duplicate/malformed symbols,
- return full valid genome shape before GA usage.

Valid input stays unchanged.

## Final validation before scoring

Layouts are strictly validated before fitness scoring:

- length = 30,
- each letter appears exactly once,
- exactly 4 empties,
- every placed symbol satisfies compiled domains and same-side constraints.

Invalid layouts are rejected.
