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

## Chapter Table

| Chapter | Part | Status |
| --- | --- | --- |
| [1. A reference image](ch001-a-reference-image.md) | I | Partly implemented |
| [2. What the prototypes taught](ch002-what-the-prototypes-taught.md) | I | Partly implemented |
| [3. Start from a bare world](ch003-start-from-a-bare-world.md) | I | Partly implemented |
| [4. Ground you can stand on](ch004-ground-you-can-stand-on.md) | II | Implemented |
| [5. Plant a forest with a function](ch005-plant-a-forest-with-a-function.md) | II | Partly implemented |
| [6. The glade's light](ch006-the-glades-light.md) | II | Partly implemented |
| [7. Character Create in the world](ch007-character-create-in-the-world.md) | II | Partly implemented |
| [8. If you can see it, you can walk to it](ch008-if-you-can-see-it-you-can-walk-to-it.md) | II | Partly implemented |
| [9. A path through dense woods](ch009-a-path-through-dense-woods.md) | III | Partly implemented |
| [10. Hold 60 frames per second](ch010-hold-60-frames-per-second.md) | III | Partly implemented |
| [11. One address](ch011-one-address.md) | III | Partly implemented |
| [12. The room](ch012-the-room.md) | IV | Partly implemented |
| [13. Coming back](ch013-coming-back.md) | IV | Partly implemented |
| [14. Ability scores, proficiency and the d20](ch014-ability-scores-proficiency-and-the-d20.md) | V | Partly implemented |
| [15. Classes, levels 1 to 6](ch015-classes-levels-1-to-6.md) | V | Partly implemented |
| [16. Spells and slots](ch016-spells-and-slots.md) | V | Partly implemented |
| [17. Conditions](ch017-conditions.md) | V | Partly implemented |
| [18. Turns become time](ch018-turns-become-time.md) | V | Partly implemented |
| 19. Familiars and Wild Shape | V | Planned |
| 20. A guard walks the path | VI | Planned |
| 21. Attack the guard | VI | Planned |
| 22. Death and respawn | VI | Planned |
| 23. Mouse look and the crosshair | VII | Planned |
| 24. The longbow | VII | Planned |
| 25. Cast while you run | VII | Planned |
| 26. Walk into a city | VIII | Planned |
| 27. Townspeople | VIII | Planned |
| 28. Interactions | VIII | Planned |
| 29. Many rooms | IX | Planned |
| 30. Accounts and persistence | IX | Planned |
| 31. Ship it | IX | Planned |
| 32. Run a live world | IX | Planned |
| [33. Let's build a dragon](ch033-lets-build-a-dragon.md) | X | Partly implemented |

