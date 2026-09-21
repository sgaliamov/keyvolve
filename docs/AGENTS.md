# Agent instructions — writing docs/post.md

## What the post actually is

A step-by-step guide to using `keyvolve` to design a custom keyboard layout. Not an essay
about the journey with technical asides bolted on — the technical content (metrics,
constraints, modes) *is* the walkthrough. Decisions and parameters get explained inline, in
passing, as the narrative reaches the point where they matter — not pulled out into separate
"why" sections that interrupt the how-to flow.

Rust, GA internals, code structure: irrelevant to the reader, don't surface them. The reader
cares about typing comfort and how to drive the tool, not about the implementation.

Post explains **why**, in the reader's language (comfort, ergonomics); docs explain
**exactly how** (units, edge cases, exact weights). If a sentence starts turning into a spec,
cut it and link the relevant doc instead.

## Voice

- First person, opinionated, a little ironic. Author's own bias is stated plainly, not
  hedged ("I put ER and TH on one hand because I like it — you might not").
- Keep the character of the existing draft: dry humor, comfortable admitting "this is a
  guess, not proven" (e.g. the qwerty E/P-adjacency bit).
- Practical over academic — this is "here's how I set it up and why," not a research paper.

## Language

Final published version: English. Drafting: mix languages freely, whatever the author
thinks fastest in (mostly Russian) — the point of drafting in Russian is to nail the tone
and jokes first, translate once the voice is locked in. Don't force early translation;
don't flatten the irony translating too early either. Don't translate draft passages
yourself unless explicitly asked — that's a separate, later pass.

## Structure (mirrors the actual usage flow, keep in sync with `docs/post.md`)

1. **Introduction** — DIY keyboard, why a custom layout, pinky-strain motivation, "no
   perfect layout, comfort wins" framing.
2. **Building a corpus** — merge/synthesise: turning raw text into the frequency stats the
   optimizer scores against, and why (speed: score bigram stats, not raw text).
3. **Calibrating effort (rank mode)** — how the interactive bigram-ranking session produces
   `keyboard.json`, told as part of the setup flow, not a separate feature dump.
4. **Metrics, as they come up while configuring** — each metric introduced at the point the
   walkthrough needs it, with the *reasoning* for why it exists (imbalance alone wasn't
   enough → added row-switch → aggregate hid a hot pinky → added per-finger caps). Exact
   formulas/defaults: link [penalty.md](/c:/Users/Admin/projects/keyvolve/docs/penalty.md),
   don't restate them.
5. **Placement constraints** — which keys get pinned/blocked/restricted and why (usage
   stats + aesthetics), as part of configuring `keyvolve.yaml`. Full rule semantics: link
   [placement-rules.md](/c:/Users/Admin/projects/keyvolve/docs/placement-rules.md).
6. **Running the optimizer and reading results** — evaluate/optimize walkthrough. Mode
   table and CLI flow: link [readme.md](/c:/Users/Admin/projects/keyvolve/readme.md).
7. **QMK tricks** — not covered by any doc in this repo (outside the tool). Write directly,
   nothing to link.
8. **Conclusion** — "one true layout is the wrong goal," bias disclosure, invite the reader
   to tune their own.
9. **Acknowledgements** — sources, papers, prior art.

## Reference docs

| Doc | Covers |
| --- | --- |
| [readme.md](/c:/Users/Admin/projects/keyvolve/readme.md) | Modes, fitness formula, CLI |
| [penalty.md](/c:/Users/Admin/projects/keyvolve/docs/penalty.md) | Every scoring metric: formula, default, tuning |
| [placement-rules.md](/c:/Users/Admin/projects/keyvolve/docs/placement-rules.md) | `frozen`/`blocked`/`allowed`/`sameSide` semantics |
| [rank-mode.md](/c:/Users/Admin/projects/keyvolve/docs/rank-mode.md) | Bradley–Terry calibration internals |

## Known open gaps in the current draft

- Need a concrete layout example where a per-finger row-switch cap catches an overloaded
  pinky that an OK aggregate `rowSwitchRatio` was hiding.
- Constraint section is a placeholder note, not written yet.

## Rules

- Discuss before editing `docs/post.md`. No unapproved rewrites.
- Prefer extending/filling the existing draft over rewriting sections — preserve the
  author's own phrasing and rough edges; polish only what's asked.
