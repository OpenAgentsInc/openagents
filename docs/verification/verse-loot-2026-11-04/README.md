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
retreat and death. This was the first death in this pass, not a successful escape.

Added observation-only grace, at most 3 seconds and at least 35% HP, for
combat without a selected target. The actual before/after public samples
form a regression check: wait at the first sample, then admit the ordinary
defensive bolt once the eligible attacker is automatically selected.
This does not remove other combat refusals or provide arbitrary emergency
pathfinding. Hostile runs require close supervision until further qualified.

## Installed host and continued qualification

Pushed `3375218295` (loot/V4/evidence V2) and `a58f8bd79a` (SDK input ownership).
Recovered the corpse through ordinary inputs, resurrected near (17636,50364),
and verified 103 HP / 298 mana. A food attempt used the wrong lease field and
was refused before input; natural regeneration restored resources. Cleanly
quit WoW before restarting Verse. The old renderer failed to leave after
close/TERM and needed a forced stop; no gameplay or encoder was killed.
The newly running renderer's `/proc/PID/exe` hash matches the fixed binary.
Holding Shift through panel close/reopen then acquiring/releasing a harmless
desk lease passed without a stale physical-input refusal.

The nested desktop intercepts Super+G: it launched Battle.net on outer
DISPLAY :1, not inner :67. Closed that misplaced instance and launched with
the inner display/socket explicitly. Battle.net's own maximize button fixed
its initially small content rectangle; WoW reopened at native 2548×1428.
The game remains on outer desk 2, inner session `1962342457979885`, handle 3
revision 5. The earlier session's IDs must not be reused.

| Run | XP kills / XP | Captures | Archive bytes | Result |
| --- | --- | --- | --- | --- |
| `native-01` | 1 / 42 | 143 | 369,884,526 | V4 native; coin loot succeeded, premature Escape opened menu |
| `native-02` | 3 / 168 | 375 | 1,073,780,083 | Corrected loot waiting; all money receipts; capacity drain |
| `native-rescue-01` | 1 / 42 | 39 | 114,805,778 | Defended against a respawn after the preceding batch had ended |
| `efficient-01` | 1 / 56 | 158 | 64,014,175 | V5 timing / V3 compact scenes; exact lossless sensor audit |
| `parking-01` | 1 / 56 | 189 | 83,158,166 | Kill/loot, then returned to (17667,50567) before completion |

Each archive audit passed with zero sensor, wire and clock mismatches.
`native-01` exposed an error in the initial loot timeout: its two-second
Escape timeout counted from the kill, so a newly opened auto-loot window
was closed immediately. It raced normal auto-close and opened Game Menu.
The corrected policy waits for the receipt/close, within the full 12-second
corpse deadline; Escape is only considered for an observed full-inventory
window. The corrected policy again killed all 130 mutants. This failure
was caught OOC at full HP without another death at that correction. `native-rescue-01`
minimum HP was 37/103, ending at 56/103. After that fight the operator walked
back to the clear patch and verified full resources; no additional death.

The early attempt to enable advanced logging with a Lua command did not
produce a log. Ordinary `/combatlog` was then verified by the client message
and new `WoWCombatLog-110426_143545.txt`. Only the later native runs have
that client-log coverage; it must not be attributed to earlier trials.

## Storage and efficient-run protocol

The three-kill lossless batch consumed 1 GiB in 191.957 s. At that time
available space fell near 2.7 GiB; no existing record was removed. A codec
experiment encoded 60 prior images losslessly as RGB H.264: all 60 RGB
SHA-256 hashes matched and the final complete PPM was byte-identical. It
used 103,667,699 bytes versus 213,559,861 bytes of the original Zstandard
archives, but this still did not sufficiently reduce sustained scene motion.
The offline probe remains a reproducible experiment, not the live recorder.

Added explicit compact V3 evidence: full-resolution WebP85 for every scene,
plus an original RGB sensor-region PPM.zst, capture-time hashes for both,
all exact public samples/candidate sets/effects/wire and passive native video.
A representative real capture used 533,525 bytes in 291 ms with every decoded
sensor field preserved. Do not claim lossless full-scene replay in this mode.
The independent nine-case fixture accepted a valid compact capture and refused
truncated/missing/misbound/orphaned strips, missing/corrupt/orphaned scenes,
and altered loot fields. Single-frame full-scene replay export was also tested.

V5 uses 300 ms scans, cast waits capped at 1500 ms, OOC resource waits of
2000 ms, and measured low-throughput area advice after three kills. It
supports an explicit clear parking point; `parking-01` exercised the full
kill/loot/park path. Filesystem availability is sampled every 16 observations;
768 MiB remaining or an unreadable value requests drain/parking. V5 policy
mutation at that milestone: 93/93; availability parser/reserve: 47/47. Controller: 185 checks.
Updated analyzer: 114 checks. New descriptors compile and Nix evaluation passes.

## Further qualification and second hostile transition

| Run | XP kills / XP | Captures | Archive bytes | Result |
| --- | --- | --- | --- | --- |
| `efficient-02` | 3 / 154 | 279 | 119,325,932 | ASan/UBSan; minimum 76 HP, full resources at parking point (17661,50572) |
| `leveling-01` | 3 / 126 | 455 | 212,480,855 | Unchanged OOC target and UI errors; stopped at full resources |
| `leveling-02` | 2 / 84 | 87 | 32,319,466 | Failed-pull recovery worked, then loot refusal with a second attacker still active |

All three archive audits pass with zero image/sample mismatches and clock
errors. `leveling-02` first cleared the unchanged errored pull, stepped back
and right, reacquired visually and killed that same previously blocked spawn
7047920. This qualifies that recovery once, not general obstacle planning.

At its second verified kill (spawn 7047845), combat remained active at
76/103 HP. Loot admission refused that combat state and the controller
stopped. The operator again missed the terminal stop while analyzing records;
Foob died to the remaining attacker at (17771,51822). This is the second
death in this pass. The controller audit ends at its refusal and must not be
used as the final death-state record; passive video records the aftermath.
Level is still 5, XP 1105/2800: 19 verified XP kills and +886 XP this pass.

V5 now waits up to 1500 ms for a healthy kill tail, records `loot-deferred`
instead of attempting loot during continued combat/casting or urgent low-HP
recovery, and clears only a freshly rechecked selected own corpse to permit
normal attacker selection. Deferred corpses are not queued for later and
are not counted as looted. Low-health OOC recovery and defense against an
eligible creature publicly targeting the player can continue below the old
35% floor; new-pull health/mana thresholds and old profile behavior remain.
Shared policy mutation: 365/365; V5 policy: 203/203; controller: 189 checks.
No surviving mutants or equivalence exclusions. Live qualification of the
new multi-attacker transition remains required.

Recovered from the graveyard with ordinary bounded movement, using the
resurrection dialog at (18017,51189), then retreated OOC and regenerated.
The visible durability indicator shows broken equipment; the water slot is
empty. Vendor maintenance is the next live task before another hostile run.

Storage remains constrained by concurrent machine activity. Converted 37
operator PPM originals to lossless PNG, decoded every PNG back to PPM and
compared byte-for-byte before removing only the verified duplicate raw
temporary files. Originals' SHA-256 and PNG hashes remain in
`operator-originals-png/original-and-png.sha256`; no unique RGB evidence or
previous run was deleted.

## Maintenance and direct-input trace

Ordinary movement reached Anvilmar after several blocked tree/wall/pillar
attempts. The successful left entry passage reached (18745,44540); nearby
Rybrad Coldbank's ordinary merchant UI opened at (18771,44404). Repair All
cost 104 copper (119→15). The red broken-equipment indicator disappeared
and the equipped robe became visible again. No item was sold or deleted.
The water action remains empty; the remaining money is below the listed
five-water bundle price. Vendor approach is supervised, not a qualified
general indoor route. Quests remain untouched.

Added `verse-game --trace ABSOLUTE_JSONL`, reusing the same durable broker
exchange writer as the hunt. It preflights the output before connection/input
and logs info/list/acquire/capture and actual key/button requests/replies.
The direct CLI still releases its lease on socket close; the trace does not
invent an acknowledged broker-final row. Checks: 88/88; relative trace exit 2,
unwritable trace exit 1 before the deliberately nonexistent socket was tried.
Live capture and movement traces append to `operator-wire.jsonl`; observed
key-down/up pairs balance. Earlier direct commands retain passive video and
tool-history coverage only. The `/targetexact Adlin Pridedrift` maintenance
text was typed through X11 (normal chat command, no target change) and is not
misrepresented as per-character broker wire. No public chat was sent.

`recovery-02.video` and `vendor-01.video` each finalized 2640 frames with
encoder status 0. The client combat log has now flushed 98,563 bytes; a raw
snapshot is retained locally. Additional completed-run scratch PPMs were
losslessly PNG-encoded, decoded and compared byte-exact before duplicate raw
removal; their manifest is in `scratch-originals-png/`.

## Environmental failure and guarded navigation V2

Pushed direct tracing as `05054aa4ce`. The latest full-checkout tools-check
at that commit reported 28 existing missing reason headers after upstream
changes (earlier count 21); this milestone introduced no non-Coax tool.
Narrowed only this disposable worktree with sparse checkout to release about
1.3 GiB of unrelated docs/fixtures; the main checkout and evidence are intact.

Original pinned Elephant remained ready on CUDA. `maintenance-exit-01`
used V1 to approach an indoor waypoint. It stopped for no progress after 21
actions at (18676,44688), beside a brazier. The navigation strip still said
alive/OOC, but original full RGB observation 18 has 103/103 HP and 19 has
88/103 HP. V1 did not consume health. The operator missed the later damage;
Foob died from the environmental hazard. This is the third death in this
pass, with XP unchanged at 1105. Passive `vendor-03.video` covers the damage
and death; V1's final observation (22) still has 97 HP and is not the death
record. The client log snapshot and images preserve the distinction.

Recovered through ordinary movement and the resurrection dialog outside the
building at (18724,45408), then backed clear and regenerated to 103/103 HP
and 298 mana. The repaired gear remains equipped. No spirit-healer recovery,
quest turn-in, item sale or deletion was used.

Added explicit `--health-guard`, profile `wow-coldridge-waypoint-v2`, preserving
V1. It binds combat/nav serials on the same frame, requires 85% initial HP,
retains every pre-input recheck and received model answer, and records broker
wire. OOC health loss cancels travel; after a fresh same-heading/life/UI/map
check, a preceding bounded forward move can be followed by one 500 ms reverse
and an exit-3 outcome with the final captured health. It does not recognize
fire, guarantee a safe reverse path, or provide combat escape.

The authority policy kills all 51 mutants. Navigator/dependency checks:
170/170. The independent Unix-socket SDK peer under ASan/UBSan replays the
actual original frames 18/19/20 against the emitted native controller. It
requires precisely one forward key pair, one fresh health-loss recheck, one
500 ms reverse key pair, both stops, a final capture and exit 3. It passes;
JSONL has one terminal outcome and the observed trigger is retained. This
is a controlled replay of recorded inputs, not a successful live fire escape.
Protobuf V2 descriptors compile.

Installed V2 and tested `health-live-01` on clear ground with original local
Elephant: three admitted forward decisions, arrived at (18717,45627), still
103 HP, alive/OOC. This qualifies the ordinary V2 path, not the hazard escape.
The third death's recovery and this trial are retained in passive recordings.

## Survey priority correction and repaired combat

Pushed guarded navigation as `cb747cc466`. A full-checkout tools-check after
the upstream update reports 29 existing missing reason headers; the new SDK
oracle has its independent-oracle header. A sparse-checkout run had reported
83 because omitted files were unreadable; that is not the full-checkout gate.

| Run | XP kills / XP | Captures | Archive bytes | Result |
| --- | --- | --- | --- | --- |
| `defensive-03` | 1 / 42 | 490 | 205,127,257 | ASan/UBSan; minimum 96 HP; item receipt; immediate stop file OOC near home |
| `survey-04` | 1 / 42 | 343 | 146,784,363 | ASan/UBSan; minimum 96 HP; receipt; finish file drained and parked at (20308,45909) |

Both audits pass with zero mismatches/clock errors. `defensive-03` exposed
search starvation: at observations 190–192 a low-tier visible candidate
interrupted an already active survey leg (`roaming=true`, scans=8). Rejected
low-tier selections now count toward the scan budget, and unbound candidates
wait until an active leg ends. The corrected V5 authority kills 213/213
mutants; the controller passes 190 checks. `survey-04` exercised advancement
to leg 1 and completed its explicit park. Legacy profiles are unchanged.

The southern circuit still mostly contains low-level wolves. At this point
Foob is level 5, 1189/2800 XP, 103 HP / 298 mana, four free slots: 21 verified
XP kills and +970 XP this pass. These isolated boar runs do not qualify the
new continued-combat/corpse-deferral transition. Three deaths remain recorded
in the pass, with no additional death in these two batches.

Deduplicated only byte-identical completed SDK-fixture images with hardlinks
to their immutable original frames after `cmp`; paths and bytes remain.
More operator PPMs were round-trip-verified as lossless PNG. No unique
evidence, inventory item or another task's data was removed.
