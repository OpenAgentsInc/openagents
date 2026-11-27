# Plan

This is the plan for [The Coax Programming Language](README.md) (`coax/book/`).

## Current State

`coax/book/chapters.json` tracks 47 chapters (21 implemented, 12 partly
implemented, 14 planned). Parts I–III are the shared prerequisite for every
other OpenAgents book (`docs/books/README.md`).

## Priorities

1. **Keep capability claims synced:** `coax/book/capability-claims.json` binds
   language claims to checked test files and publication reports, verified on
   every `scripts/check-books.sh --check` run.
2. **Keep Chapter 3 readable:** `ch03-common-concepts.md` is long; add clearer
   part signposts or split it only when doing so preserves every legacy anchor
   and cross-book link without disrupting `examples/ch03_*.coax`.
3. **Burn down planned chapters (14):** Promote `partly implemented` and
   `planned` chapters only when their four-rule teaching examples and
   independent checks pass on the qualified compiler.
