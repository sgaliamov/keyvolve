# Placement rules - how they work

Placement rules restrict where `optimize` may put each letter and the 4 empty slots. Every
layout the optimizer generates, mutates, scores or saves obeys them. `evaluate` ignores them.

## Slot map

30 slots: 26 letters `a`-`z` + 4 empties. Left hand `0`-`14`, right hand `15`-`29`, physical
left-to-right:

| Row    | Left             | Right            |
| ------ | ---------------- | ---------------- |
| top    | `0 1 2 3 4`      | `15 16 17 18 19` |
| home   | `5 6 7 8 9`      | `20 21 22 23 24` |
| bottom | `10 11 12 13 14` | `25 26 27 28 29` |

Mirror = same finger on the other hand: `0`↔`19`, `4`↔`15`, `7`↔`22`. Formula for a left slot
`i`: `(i / 5) * 5 + (4 - i % 5) + 15`.

A row segment is one row of one hand: 5 slots, e.g. `0`-`4` or `25`-`29`.

## Configuration

```yaml
optimization:
  blocked: [0, 10, 19, 29]
  frozen:
    z: 10
  allowed:
    '_': [0, 1, 10, 11]
    e: [6, 7, 8]
    x: [4, 14]
  sameSide: ['th', 'er']
  left: [s]
  right: [o]
```

| Key        | Default | Meaning                                                                                                                       |
| ---------- | ------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `frozen`   | none    | `{ letter: slot }`, slot `0`-`29`. Letter always sits at that slot.                                                           |
| `blocked`  | none    | Slots `0`-`29` that letters may not use. They stay empty unless a letter is frozen there.                                     |
| `allowed`  | none    | `{ key: [slots] }`, slots `0`-`14`. Each slot also allows its mirror. Key `_` (or `` ` ``) sets where empties may go.         |
| `left`     | none    | Letters restricted to slots `0`-`14`. Not mirrored.                                                                           |
| `right`    | none    | Letters restricted to slots `15`-`29`. Not mirrored.                                                                          |
| `sameSide` | none    | Two-letter strings. Both letters of each pair sit on one hand.                                                                |

Without an `allowed` entry, a letter or empty may go anywhere the other rules permit.

## Rules

1. `frozen` letters sit at their slot. `allowed`, `left`, `right` and `blocked` do not apply
   to them. No other letter or empty may use a frozen slot.
2. Unfrozen letters avoid `blocked` slots and stay inside their `allowed` slots and
   `left`/`right` hand.
3. With `allowed._` set, empties go only to `allowed._` or `blocked` slots. `allowed._`
   permits, it does not force: a listed slot may hold a letter.
4. `sameSide`: each pair picks a hand independently. Pairs sharing a letter merge into one
   group (`th` + `st` → `t`, `h`, `s` on one hand). A frozen letter fixes the hand of its
   group.
5. No gaps in the middle of a row segment. Empties may only sit at the ends of a segment,
   never between two letters (see [Row gaps](#row-gaps)).

A letter listed in both `left` and `right` has no legal slot unless frozen.

## Row gaps

Within each row segment, letters must be contiguous. An empty slot with letters on both
sides of it in the same segment is a gap.

| Segment | Valid |
| ------- | ----- |
| `gjv__` | ✓     |
| `__gjv` | ✓     |
| `_gjv_` | ✓     |
| `g_jv_` | ✗     |
| `gj_v_` | ✗     |

The rule applies to all empties, including `blocked` slots. Rules that make a gap
unavoidable stop the run at startup (e.g. `blocked: [1, 3, 11, 13]`).

Not implemented yet: the optimizer currently allows gaps and only prefers keeping letters
together. Tracked in [TASKS.md](../TASKS.md#tasks).

## Startup checks

Rules are checked before the GA starts. The run stops with an error when:

| Problem                                                          | Error                                                           |
| ---------------------------------------------------------------- | --------------------------------------------------------------- |
| Unknown key under `optimization` (e.g. old `rolls`)              | unknown field                                                   |
| Slot outside `0`-`29` in `frozen` or `blocked`                   | `... slot N must be in 0..29`                                   |
| Slot outside `0`-`14` in `allowed` (any key, `_` included)       | `allowed slot N must be in 0..14`                               |
| Two frozen letters on one slot                                   | `multiple frozen keys use slot N`                               |
| Key other than `a`-`z` in `frozen`, `left`, `right`, or other than `a`-`z`/`_` in `allowed` | `... must be a lowercase letter a-z ...` |
| `sameSide` entry not exactly two distinct letters `a`-`z`        | `same-side pair must contain exactly two distinct ...`          |
| A letter or empty has no legal slot                              | `optimization leaves no legal slots for 'x'`                    |
| Rules allow no complete layout (e.g. too many letters per hand)  | `optimization constraints have no complete layout: ...`         |

Exactly 4 slots end up empty, so:

- At most 4 `blocked` slots may be unfrozen.
- With `allowed._` set, `allowed._` + `blocked` minus frozen slots must give at least 4
  slots.

Known bug: `allowed._` currently accepts slots `15`-`29` (used as-is, not mirrored).
Tracked in [TASKS.md](../TASKS.md#bugs).

## Generation and mutation

- New layouts are random, always complete and valid.
- A mutant moves several letters at once. `sameSide` groups move as a whole and may switch
  hands. Empties move too. Frozen letters never move.
- A mutant may equal its parent when no other valid placement exists, e.g. when all letters
  are frozen.

## Seed repair

Layouts loaded from `ga.dump` or `optimization.input` are checked against the current rules.

- Valid layouts are used unchanged.
- Invalid ones (rules changed, wrong length, missing, duplicate or unknown symbols) are
  repaired: letters keep their old slot where legal, the rest are re-placed. `sameSide`
  groups take the hands that keep most letters in place.
- Repairs log `Repaired imported layouts under current placement constraints` with the count.

## Scoring and output

Layouts that break any rule are not scored and never written to `optimization.output`.
