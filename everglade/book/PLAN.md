# Plan

This is the plan for [Building Everglade](README.md) (`everglade/book/`).

## Scope & Architecture

Everglade is the flagship D&D 5E SRD (5.2.1) MMORPG built on the OpenAgents
Metaverse Engine and **GameBuilder** (`gamebuilder` CLI and `world.edit.v1`
batches, folded into the monorepo in `#13854`; standalone `everglade-desktop`
and `everglade-mobile` shells retired in `#13856`).

## Completion Milestones

1. **Register & enforce the 4-rule teaching contract (Parts I–II, Ch 1–8):**
   - Maintain `everglade/book/chapters.json` alongside `SUMMARY.md`.
   - Upgrade Chapters 1–6 from essays into full four-rule chapters: hand
     calculation/trace, runnable `gamebuilder` / Coax source with exact expected
     transcript, broken case with refusal/error, and independent check.
2. **Multiplayer & SRD rules parity (Parts III–V, Ch 9–18):**
   - Ground every SRD chapter (ability scores, classes, spells, conditions,
     action timing) in live `rules/srd/` Coax modules rather than retired legacy
     `apps/openagents-web/old/v1/` files.
3. **World systems & beasts (Ch 19–33 + Appendix A):**
   - Fill in planned chapters 19–32 as their `rules/srd/` and
     `metaverse-engine/` modules stabilize, keeping `ch033-lets-build-a-dragon.md`
     synced with `rules/srd/dragon-actions.coax`.
