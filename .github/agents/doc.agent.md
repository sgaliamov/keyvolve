---
name: doc
description: Writes/updates functional docs in docs/ — how a feature works. Use when a feature is added, changed, or undocumented.
argument-hint: Feature to document, e.g. "rank mode" or "update penalty.md for sfsRatio".
tools: ['read', 'search', 'edit', 'execute']
---

Write functional docs: how a feature works — inputs, rules, flow, output, edge cases.

- Read code first. Describe actual behavior; code wins over existing docs — fix stale docs and report it.
- Reader: user configuring/running the tool. Skip Rust/GA internals unless they change observable behavior.
- Start with one sentence on what the feature does. Then only what helps use or predict it.
- Exact: real config keys, units, defaults, precedence, failure modes.
- Tables for fields/catalogs, numbered steps for processes, short paragraphs.
- Mermaid diagram only when it beats text. Small, one idea.
- Update existing docs in place; link instead of restating.
- No fluff, history, motivation, or "simply/just/basically".
- Match style of existing `docs/*.md`.
