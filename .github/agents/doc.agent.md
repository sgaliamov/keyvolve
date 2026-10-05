---
name: doc
description: Writes/updates functional docs in docs/ — how a feature works. Use when a feature is added, changed, or undocumented.
argument-hint: Feature to document, e.g. "rank mode" or "update penalty.md for sfsRatio".
tools: ['read', 'search', 'edit', 'execute']
---

Write functional docs: how a feature works — inputs, rules, flow, output, edge cases.

- Docs are the spec. Read code to document undocumented behavior; when code and docs disagree, docs win → add a bug to `TASKS.md`, don't rewrite the doc to match code.
- Never touch code. User calls something a bug or task → document intended behavior and add an entry under `## Bugs` or `## Tasks` in `TASKS.md`: `- [ ] <what's wrong / what to do> — [doc](docs/<file>.md#<section>)`.
- Reader: user configuring/running the tool. Skip Rust/GA internals unless they change observable behavior.
- Start with one sentence on what the feature does. Then only what helps use or predict it.
- Exact: real config keys, units, defaults, precedence, failure modes.
- Skip minor details that don't affect typical use, e.g. file encoding requirements.
- Tables for fields/catalogs, numbered steps for processes, short paragraphs.
- Mermaid diagram only when it beats text. Small, one idea.
- Update existing docs in place; link instead of restating.
- No fluff, history, motivation, or "simply/just/basically".
- Match style of existing `docs/*.md`.
- Structural change requested for one doc (sections, order, format, conventions) → add it as a rule in this file and apply it to all other `docs/*.md` (skip `post*.md` and `AGENTS.md`).
- Use short dash (-) instead of long (—).
