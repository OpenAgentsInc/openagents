# Plan

This is the plan for [Building Everglade](README.md) (`everglade/book/`).

## Scope & Architecture

Everglade is the flagship D&D 5E SRD (5.2.1) MMORPG built on the OpenAgents
Metaverse Engine and **GameBuilder** (`gamebuilder` CLI, `world.edit.v1`
batches, embedded OpenAgents Desktop Metaverse/GameBuilder pane `#14043`–`#14045`,
`OA_PACK_METAVERSE` `#14057`, and five live `gamebuilder.org` worlds;
standalone `everglade-desktop` and `everglade-mobile` shells retired in `#13856`).

## Completion Milestones

1. **Register & enforce the 4-rule teaching contract (Parts I–II, Ch 1–8):**
   - Maintain `everglade/book/chapters.json` alongside `SUMMARY.md`.
   - Upgrade Chapters 1–3 and 5–8 from essays into full four-rule chapters: hand
     calculation/trace, runnable `gamebuilder` / Coax source with exact expected
     transcript, broken case with refusal/error, and independent check.
2. **Multiplayer, SRD rules, guard & action combat (Parts III–VII, Ch 9–25):**
   - Chapters 4, 14, and 33 are implemented with runnable examples, refusals,
     and exact transcripts.
   - Chapters 19–25 are drafted and grounded in live `everglade/server/`,
     `everglade/client/`, `everglade/shared/`, `everglade/npc/`, and
     `everglade/range/` modules; extract standalone runnable examples to promote
     them to implemented.
3. **City, live operations & beasts (Ch 26–33 + Appendix A):**
   - Chapters 26–32 are written and partly implemented with runnable Coax
     examples (`everglade/examples/*.coax`) covering clearing/plots, townspeople
     & guard-guide AI, quests & item ledger, multi-room travel, accounts &
     fenced checkpoints, release-set gates, and live SLO/moderation/soak
     operations; promote them to implemented once live Everglade II city
     interiors, daily NPC routines, player-to-player trade wire commands, and
     150-player zones ship, keeping `ch033-lets-build-a-dragon.md` synced with
     `rules/srd/dragon-actions.coax`, `everglade/mounts.coax`, and
     `gamebuilder/tools/dragon-bake.coax`.

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
| [14. Ability scores, proficiency and the d20](ch014-ability-scores-proficiency-and-the-d20.md) | V | Implemented |
| [15. Classes, levels 1 to 6](ch015-classes-levels-1-to-6.md) | V | Partly implemented |
| [16. Spells and slots](ch016-spells-and-slots.md) | V | Partly implemented |
| [17. Conditions](ch017-conditions.md) | V | Partly implemented |
| [18. Turns become time](ch018-turns-become-time.md) | V | Partly implemented |
| [19. Familiars and Wild Shape](ch019-familiars-and-wild-shape.md) | V | Partly implemented |
| [20. A guard walks the path](ch020-a-guard-walks-the-path.md) | VI | Partly implemented |
| [21. Attack the guard](ch021-attack-the-guard.md) | VI | Partly implemented |
| [22. Death and respawn](ch022-death-and-respawn.md) | VI | Partly implemented |
| [23. Mouse look and the crosshair](ch023-mouse-look-and-the-crosshair.md) | VII | Partly implemented |
| [24. The longbow](ch024-the-longbow.md) | VII | Partly implemented |
| [25. Cast while you run](ch025-cast-while-you-run.md) | VII | Partly implemented |
| [26. Walk into a city](ch026-walk-into-a-city.md) | VIII | Partly implemented |
| [27. Townspeople](ch027-townspeople.md) | VIII | Partly implemented |
| [28. Interactions](ch028-interactions.md) | VIII | Partly implemented |
| [29. Many rooms](ch029-many-rooms.md) | IX | Partly implemented |
| [30. Accounts and persistence](ch030-accounts-and-persistence.md) | IX | Partly implemented |
| [31. Ship it](ch031-ship-it.md) | IX | Partly implemented |
| [32. Run a live world](ch032-run-a-live-world.md) | IX | Partly implemented |
| [33. Let's build a dragon](ch033-lets-build-a-dragon.md) | X | Implemented |
