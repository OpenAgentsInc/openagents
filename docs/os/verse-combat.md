# Verse OS combat journal

Owner request, 2026-11-03: log back into WoW, reliably find creatures that
give XP and fight well; defer quests. GitHub #12744. Work continues from
the native presentation and local Elephant waypoints in #12728–#12730.

## Live setup

Reconnected the existing Classic Era client, selected Foob (level 4 gnome
warlock), and entered Coldridge Valley on Deviate Delight. Initial observed
health/mana: 88/272; XP 233/2100. The existing navigation sampler remains
unchanged. Original local Psionic Elephant steered another waypoint from
the graveyard slope to (19896,46104), within the radius of (19900,46100).

The new read-only SDK strip reports resources, selected creature identity,
level, health and eligibility facts, action slots, cast/cooldown/range
feedback, UI errors, and player kill events. Coax admits targets and actions.
Its initial profile uses the observed Shadow Bolt (686), Demon Skin (687),
attack (6603), water (159) and food (4604) slots. It does not assume every
warlock has these bindings or that later ranks are interchangeable.

## Development trials

- First loop independently selected and approached a Ragged Young Wolf
  (NPC 705), cast Shadow Bolt, and gained 52 XP, ending at full health.
  It stopped when target selection disappeared before a corpse sample.
  Added the public player `PARTY_KILL` event and matched NPC/spawn suffix
  plus positive XP delta; target death alone is not enough.
- The next loop killed a Small Crag Boar (NPC 708), gained 104 XP and
  finished at 84/88 health. Clearing an already absent target opened the
  menu. It now clears only a selected target.
- The long RGB strip initially used a byte sum. A wrong UI-scale pitch
  occasionally passed that weak check with wrong fields. Replaced it with
  CRC-32/ISO-HDLC, reusing Coax's checked PNG CRC. The fixture's CRC was
  independently computed with Python zlib.
- Subsequent runs refused stale data. A visible SDK diagnostic identified
  restricted nameplate `GetCenter` measurements on this client. Removed
  that call; optional nameplate geometry stays unavailable. `pcall` keeps
  adapter failures visible, and the controller refuses stale samples.
- A run started during UI reload refused unavailable observations before
  input. Reload completion is a prerequisite, not a reason to bypass the
  sensor gate.

Early trial journals are retained as development evidence; their byte-sum
samples are not used as qualification of the final CRC profile.

## Protocol and boundaries

See [external-combat-v1](../protocol/external-combat-v1.md). An XP kill
requires a new player kill event naming the engaged creature and a positive
XP delta. The controller retains the current window/target lifetime, limits
each input pulse, releases input before waiting, and has fight, damage,
survey and observation budgets. `OUT/stop` ends a loop; `verse-game stop`
releases a currently held key immediately. A closed process connection
releases its input lease.

The live development controller is `os/game-combat.coax`, run with
`coax/web/tool.sh os/game-combat.coax --out NEW_ABSOLUTE_DIR --kills 3`.
It uses a rolling `frame.ppm` to keep disk use bounded, full typed JSONL
observations and immutable `evidence-N.ppm` at starts and verified kills.

## First repeatable loop

After these fixes, trial 07 completed three consecutive kills without manual
steering or targeting: a Small Crag Boar and two Ragged Young Wolves. New
matching player kill events and XP gains verified 104 + 52 + 52 = 208 XP.
Foob started at 389 XP and finished at 597, at 87/88 health; minimum health
was 83/88. Two water uses were observed between pulls. The loop consumed
168 observations, including searching for the third target after the first
two kills. A longer eight-kill batch follows to measure repeatability.

The final sensor authority killed all 134 entry-source mutants; the combat
policy with the after-kill grace killed all 249, zero survivors. The controller
import closure passed 106 Coax checks. The semantic protobuf compiled. `tools-check` reports
12 unrelated existing/concurrent files without reason headers. No quest
was accepted or turned in by this work; existing quest counters can still
advance when their creature is killed.

This remains a bounded starter-warlock profile, levels 2–5, on map 1426 with
the specified action slots. General zone/elite/PvP/combat reliability is not
implied. The eight-kill run and sanitizer qualification are in progress.

The first longer batch gained another 104 XP, then refused a transient
combat flag after its verified kill. Added a three-second idle-only grace
for that exact post-kill condition, with fresh life/UI/resource checks.
The next eight-kill batch is testing it. The native bundle installation and
Nix flake evaluation passed. See the
[raw trials](../verification/verse-combat-2026-11-03/README.md).

## Control profile v2 and failures kept visible

The next long run verified four kills for 174 XP. I requested a hard stop
while a fifth encounter was already underway, then spent too long working
before resuming it. The low-health gate refused the resumed controller and
Foob died. This was an operator pause error, not a successful combat run.
Released spirit, walked the corpse route through ordinary bounded inputs,
and accepted the resurrection. The controller then ate one food, restored
resources and reapplied Demon Skin before pulling again.

Added `OUT/finish`: stop new pulls, finish the admitted fight, wait out of
combat, then exit. A live fixture requests this during an actual combat
observation and checks exactly one XP kill, a living out-of-combat player
and zero held broker input. It passed with the controller under ASan/UBSan
and leak detection. Observation/time budgets now drain as well; hard stop
remains immediate. A stopped process does not stop an external enemy.

The first patrol run after recovery verified three kills for 104 XP.
A subsequent sanitized batch killed one wolf, then followed a range bit in
the wrong direction and hit the survey boundary while still healthy. Tested
the normal interact key as a facing aid, then restored its temporary F6
binding and the original `autoInteract=0`. The final controller uses bounded
screen-based selection instead of persistent click-to-move.

The visual detector reads neutral creature bars directly from the RGB
capture, turns toward a central candidate and clicks it. Pixels never admit
an attack: the next sample must identify an eligible creature and all input
is rechecked against a second capture. Each pre-pull forward pulse requires
another visual selection. The first visual batch verified three kills for
130 XP. A 600×600 circuit now covers more of the patch, with an 1800-unit
displacement fence. Original local Elephant successfully returned from
(21122,47114) to (20306,45905), inside the radius of (20300,45900), in 60
actions. The final longer sanitized batch is in progress.

The unchanged sensor retains its prior 134/134 mutation result from
`a3869fe159`. The v2 profile is documented separately; observation v1 was
not changed in place.

## Longer visual search and blocked terrain

The first longer visual batch, under ASan/UBSan and leak detection, verified
six kills for 208 XP in ten minutes. Minimum health was 83/88; it ended out
of combat at 88/88 health and 272/272 mana. It did not reach its eight-kill
request. The final screenshot showed a tree blocking the patrol leg.
Added a 12-second distance-progress deadline and a two-step back/sidestep
recovery that skips a blocked leg. The home is now fixed at (20300,45900),
so repeated batches cannot gradually move the survey fence across the zone.

A development recovery batch verified five kills for 182 XP. The following
sanitized batch verified two wolves for 52 XP, then honored `finish` after
the active fight and exited alive, out of combat. It exposed slow visual
alignment: repeated 35 ms turns often left the observed heading unchanged.
Large turns now use 90 ms while final alignment retains 35 ms; survey turns
have a 60 ms minimum. This is feedback from the public heading and visible
bar, not a claimed physical turn angle or a hidden creature position.

One later pull hit the original 12-second damage deadline during its first
successful cast, after range correction. Resumed immediately and verified
the wolf's 26 XP. The final profile allows 20 seconds without observed
damage within a 45-second encounter bound. Kill-count completion now uses
the same out-of-combat drain as `finish`, rather than exiting on the kill
sample while the game's combat flag can still linger.

Final authorities: combat 343/343, visual candidate policy 117/117,
survey 82/82 and drain/deadlines 21/21 killed, zero survivors. The controller
closure passes 163 checks. Real final trials are recorded in the
verification index. Emergency retreat and automatic corpse
recovery remain outside this profile; an unsafe refused fight can still
require intervention in the external game.

The final source completed a three-kill ASan/UBSan/leak-detection trial:
two boars and a wolf, 130 XP, minimum health 81/88, final health 88/88.
The back/sidestep recovery ran at observations 283/284; the controller
resumed surveying and found the third creature. It waited until combat
cleared, recorded `finished: kill-target`, and exited successfully at
observation 363. No manual steering, targeting or casting occurred during
that run. The native executable was installed into the existing Verse
bundle; artifact hashes and raw results are in the verification directory.

The installed executable then passed the same live finish fixture: request
at active combat observation 15, one wolf for 26 XP, minimum 86/88 health,
final 88/88, alive and out of combat, zero held keys/buttons. Final XP:
1898/2100, level 4. The controller is stopped; the game remains open.
