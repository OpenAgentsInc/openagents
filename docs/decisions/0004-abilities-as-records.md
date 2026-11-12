# 0004. Abilities as data records

Date: 2026-11-11
Status: Accepted
Source: [Everglade and Aethyr against AAA MMORPG practice](../audits/2026-11-11-everglade-aethyr-aaa-audit.md), section G2; [everglade/ARCHITECTURE.md](../../everglade/ARCHITECTURE.md)

## Context

SRD 5.2.1 classes, spells, and martial actions in `rules/srd/` and `coax/web/room/` grew through per-ability branches for cast times, targeting shapes, line of sight, cooldowns, saving throws, and client VFX/HUD descriptions. Divergent per-ability code caused mismatches between client bars, Everglade v1 casts, and Everglade II casts (for example W5, Everglade II casts skipping line of sight).

## Decision

Define every spell, weapon attack, and class feature once as a typed ability record in `rules/srd/` consumed by one shared execution pipeline:

- Each record declares targeting mode, range, area shape, line-of-sight requirement, resource cost, cast/recovery timing, cooldown, saving throw or attack roll formula, conditions applied, and presentation tags.
- One resolver validates eligibility, range, resources, and line of sight and applies effects on the authoritative room server while driving client prediction, HUD tooltips, and VFX cues.

## Consequences

- Every ability automatically obeys the shared line-of-sight, cooldown, GCD, and combat-log pipeline.
- Adding SRD spells, feats, or monster actions means authoring table rows and composable effect primitives rather than new room message handlers.
