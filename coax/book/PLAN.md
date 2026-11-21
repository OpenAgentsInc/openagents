# Plan

This is the plan for [The Coax Programming Language](README.md) (`coax/book/`).

## Current State

`coax/book/chapters.json` tracks 47 chapters (21 implemented, 10 partly
implemented, 16 planned). Parts I–III are the shared prerequisite for every
other OpenAgents book (`docs/books/README.md`).

## Priorities

1. **Keep capability claims synced:** `coax/book/capability-claims.json` binds
   language claims to checked test files and publication reports, verified on
   every `scripts/check-books.sh --check` run.
2. **Streamline Chapter 3:** `ch03-common-concepts.md` is currently 1,440 lines;
   split it into two focused chapters (*Values, Types & Control Flow* and
   *Memory, Slices & Fixed Buffers*) while preserving legacy anchors/links.
3. **Burn down planned chapters (16):** Promote `partly implemented` and
   `planned` chapters only when their four-rule teaching examples and
   independent checks pass on the qualified compiler.
