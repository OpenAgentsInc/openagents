# Visible WoW looting and XP-route improvement, 2026-11-04

Issue [#12863](https://github.com/OpenAgentsInc/openagents/issues/12863).
Worktree began at `1026ab52cd`; prior v3 evidence remains in its separate
[record](../verse-combat-2026-11-04/README.md). No old gate was rerun merely
to reconfirm it. New records live under
`~/.openagents/verse-os/combat/2026-11-04-loot/` on coderos-4080.
This is the same visible native 2548×1428 game on outer desk 2, inner
`verse-desk.sock`. No headless game or private game-memory control is used.

The session had expired. Reconnected through the normal UI and verified Foob
in world at level 5, 219/2800 XP, 103 HP, 298 mana. Installed the read-only
loot strip and reloaded OOC. Autoloot was already enabled; public bag APIs
reported 7 free / 26 total slots, 63 copper. Added F6 INTERACTTARGET and F7
TARGETLASTTARGET only if unbound, using the ordinary chat command interface;
kept autoInteract off. An initial `/reload` keystroke lost its slash and
appeared as “reload” in local say; corrected the operator typing delay and
verified the new strip before any automated loot trial.

## Initial live qualification

| Run | XP kills / XP | Captures | Archive bytes | Result |
| --- | --- | --- | --- | --- |
| `loot-01` | 1 / 18 | 47 | 60,747,525 | Wolf, 2 Chipped Claws; item receipt, free slots 7→6 |
| `tier-01` | 1 / 42 | 285 | 689,471,335 | Level 3 boar; item receipt; ASan/UBSan clean |
| `troll-01` | 1 / 42 | 63 | 104,842,947 | Level 3 troll; 2 item events + 3 copper; slots 6→5; ASan/UBSan clean |
| `troll-02` | 1 / 56 | 131 | 229,274,270 | Level 4 troll, money receipt; later ambient-combat refusal and operator recovery failure |

The four complete archive audits reconstructed every image and compared
combat, navigation and loot samples: zero mismatches and zero wire/clock
errors. `tier-01` was finished deliberately after 161.676 s because search
dominated: 143.409 s search, 13.369 s combat, 4.898 s recovery. Its minimum
health was 97/103. `troll-01` minimum health was 90/103, ending OOC at
321/2800 XP and 103 HP. New receipts require a matching newly opened/closed
loot source and an item event or money increase; kill/death alone is not loot.

Lossless evidence v2 uses previous observations as Zstandard dictionaries,
with a standalone keyframe every 30 observations and each recheck based on
its own observation. Early stationary observations dropped from roughly
6 MiB standalone to roughly 0.6 MiB deltas. This varies with camera motion;
the boar search was still expensive. The archive budget is now 1 GiB and
drains both combat and the pending corpse. No old recording was deleted.
The real-image harness accepted valid evidence and refused truncated,
missing, misbound and orphaned archives. A separate wrong-delta-base fixture
returned exit 1. Passive native 4 fps video started before gameplay actions;
`operator-01.video` completed 2640 frames / 660.258 s, encoder exit 0.

## Scouting and input defects

Original pinned local Elephant on CUDA (identity ending `681d0ba`) reached
(20803,47486), (20002,48590), then the waypoint near (18500,49900) while
alive/OOC. Its fourth leg toward (18000,51300) stopped at a snowy slope near
(17973,51217) after two ineffective forward pulses. It did not continue
blindly. A short operator back/turn exposed trolls around the hillside.
The creature detector previously saw yellow bars only. V4 also recognizes
red bars as selection candidates, followed by the same public-state combat
admission. The troll qualification exercised this path, including normal
defensive combat when a hostile creature approached.

Found four host input-ownership defects: Wayland enter ignored its actual
held-key snapshot; desk key-up was lost after closing the panel; pointer-up
could be lost outside/after closing a panel; and a disconnected hand socket
could retain a pinch. The SDK effect fixes reconcile focus while preserving
real held keys, avoid replaying toggle/voice presses, retain releases for the
original desk recipient, and release a pinch on EOF. The independent native
fixture failed on the original stale-focus behavior and the closed-panel
key release, then passed with ASan/UBSan. Both realm host binaries built
against their unchanged installed Coax object files. Live activation and
longer hunt results are recorded below when complete.

## Checks at this milestone

- Loot sensor: 91/91 mutants killed; loot policy: 130/130; evidence reference
  policy: 10/10; XP-quality/level-goal policy: 19/19; updated creature vision:
  139/139; ambient-combat grace: 25/25. No survivors or equivalence exclusions.
- Controller: 178 checks before the additional recorded-transition regression;
  analyzer: 113; passive video: 100; loot CLI: 93.
- New loot v1 / combat control v4 / evidence v2 protobuf descriptors compile.
  Nix package evaluation passes; launcher and Nix package include the loot CLI.
- tools-check reports 21 pre-existing non-Coax tools without reason headers;
  none is introduced here. The addon and new SDK effect oracle declare their
  allowed SDK / independent-oracle reasons.

Earlier `loot-01` / `tier-01` binaries predate the configuration journal row
and red-candidate change. Their generated C/binary and exact commands are
retained with the local evidence; later runs record explicit configuration.
This work reuses Aethyr waypoint geometry and the external-event/record
boundary described in its book. The record replays admitted observations and
intents, not Blizzard's world simulation. Combat/loot remain deterministic
Coax policies; Elephant steers the separate qualified waypoint controller.

## Hostile-run failure and correction

`troll-02` killed/looted one level 4 troll for 56 XP and 3 copper. It then
repeatedly selected a low-level neutral animal. Added explicit
`--hostile-only` candidate filtering so a hostile patch can ignore neutral
bars instead of bouncing between an unwanted animal and a short turn.

A respawn approached while no target was selected. Combat became true one
sample before WoW's automatic target appeared; v3's policy refused that
unbound combat state and exited at 103 HP. The next passive video frame
already had an eligible attacking troll selected, 100 HP. The operator
missed the terminal refusal while editing; by inspection HP had fallen to
38. Three ordinary two-second retreat holds were insufficient and Foob died
at (17586,50579), still 377/2800 XP. The lossless controller record correctly
ends before this death; passive video retains the subsequent damage, failed
retreat and death. This is one death in this pass, not a successful escape.

Added observation-only grace, at most 3 seconds and at least 35% HP, for
combat without a selected target. The actual before/after public samples
form a regression check: wait at the first sample, then admit the ordinary
defensive bolt once the eligible attacker is automatically selected.
This does not remove other combat refusals or provide arbitrary emergency
pathfinding. Hostile runs require close supervision until further qualified.
