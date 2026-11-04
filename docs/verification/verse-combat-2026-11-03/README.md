# Visible Classic Era combat, 2026-11-03

Foob, level 4 warlock, Deviate Delight, Coldridge Valley (map 1426).
Native Verse OS desk 2, 2548×1428. Ordinary leased input only; SDK code
reads public state and draws observations, never casts or moves.

| Trial | Result | Journal |
| --- | --- | --- |
| First wolf | 52 XP; stopped on disappearing target before corpse sample | [01](01-first-wolf.jsonl) |
| Boar and menu | 104 XP with matched kill event; clearing absent target opened menu | [02](02-menu-after-boar.jsonl) |
| Corrected batch | Three consecutive kills, 208 XP; minimum health 83/88, final 87/88; two water uses | [07](07-three-kills.jsonl) |
| Longer batch | One boar, 104 XP; transient combat flag after kill refused the next step | [08](08-combat-lingers.jsonl) |
| Interrupted development fight | Four kills, 174 XP; hard stop during next fight, later death; manually recovered | [09](09-interrupted-fight.jsonl), [10](10-low-health-refusal.jsonl), [death](interrupted-fight-death.png) |
| After recovery | Three kills, 104 XP; food, regeneration and Demon Skin before pulls | [11](11-patrol-after-recovery.jsonl) |
| Finish during combat | ASan/UBSan: finish requested during active fight, exactly one XP kill, alive/out of combat, zero held input | [12](12-finish-during-combat.jsonl), [check](finish-check.txt) |
| Blind range chase | One wolf, then survey-radius refusal while healthy; led to visible-bar selection | [13](13-blind-range-boundary.jsonl) |
| First visual batch | Three kills, 130 XP without manual steering/targeting | [14](14-visual-three-kills.jsonl) |
| Elephant return | Original local model: 60 actions to (20306,45905), goal (20300,45900) | [15](15-elephant-return.jsonl) |
| Longer visual batch | ASan/UBSan: six kills, 208 XP in ten minutes; min 83/88 health, final 88/88 and 272/272 mana; eight requested, budget stop at six | [16](16-six-kill-budget.jsonl), [blocked tree](blocked-tree.png) |
| Blocked-leg development | Five kills, 182 XP; requested finish and completed current encounter | [17](17-blocked-leg-development.jsonl) |
| Short-turn investigation | ASan/UBSan: two wolves, 52 XP; finish drained active fight, alive/out of combat | [18](18-short-turns-finished.jsonl) |
| First-projectile deadline | Original 12-second no-damage deadline expired during cast after approach; immediately resumed and verified 26 XP | [19](19-first-projectile-deadline.jsonl), [20](20-deadline-resumed.jsonl) |
| Final source, sanitized | Three autonomous kills, 130 XP; min 81/88 health, final 88/88; blocked-leg recovery executed, search resumed, kill target drained until out of combat | [21](21-final-three-kills.jsonl), [screenshot](final-three-kills.png), [controller output](final-sanitized-gameplay.txt) |
| Installed native executable | Finish requested during combat observation 15; one verified wolf, 26 XP; min 86/88 health, final 88/88, alive/out of combat, zero held keys/buttons | [22](22-installed-finish.jsonl), [check](installed-finish-check.txt) |

The final profile adds a bounded three-second idle grace after a verified
kill when combat has not yet cleared. It cannot permit attacking or moving
during that grace. Finish files, kill-count completion and observation/time
budgets wait until the encounter ends and the player leaves combat. Hard
stop remains immediate and cannot stop enemies in the external game.

The survey has a stable home at (20300,45900), a 600×600 circuit and an
1800-unit displacement fence. A blocked leg gets bounded back/sidestep
recovery, then is skipped. This is not arbitrary terrain path planning.
Combat has 45-second total and 20-second no-damage deadlines. Emergency
retreat and automatic resurrection remain unqualified. One development
pause caused a death; successful later trials do not erase that failure.

Journals are raw development evidence. Trials 01/02 used the superseded
byte-sum framing, which could admit a wrong UI-scale pitch in some fields.
Trial 07 onward uses CRC-32 and omits the client's restricted nameplate
geometry API. No early incorrect sample is silently rewritten as final
profile qualification. [Screenshot after the first wolf](first-wolf.png).

`frame.ppm` is a rolling local image cache. Immutable selected PPM snapshots
are local `evidence-N.ppm` artifacts at starts and kills, not a retained image
for every JSONL observation. The journal records full sensed facts and
capture identities; it does not replay WoW's simulation.

Checks: unchanged sensor 134/134 entry-source mutants killed at
`a3869fe159`; combat policy [343/343](combat-mutation.txt), visual policy
[117/117](vision-mutation.txt), survey [82/82](survey-mutation.txt),
drain/deadlines [21/21](drain-mutation.txt), all zero survivors. Controller
closure: [163 checks passed](controller-checks.txt). Finish fixture: 85
checks passed. Observation v1 and control v2 protobuf compilation passed.
Nix flake evaluation and native bundle installation passed at the first
milestone; the updated controller is installed atomically into that bundle.
ASan/UBSan/leak detection instrument the controller/host adapter, not the
external game or graphics driver. [tools-check](tools-check.txt) reports
12 unrelated existing/concurrent header violations before rebase. The
[final rebased tree](tools-check-rebased.txt) reports 15, including three
more concurrent files outside this work. No new non-Coax tool was
introduced by this change. The controller import closure and native host
sources were unchanged by the rebase, so their passing checks were retained.

[Observation v1](../../protocol/external-combat-v1.md) ·
[Control v2](../../protocol/external-combat-control-v2.md) ·
[Work journal and limits](../../os/verse-combat.md).
