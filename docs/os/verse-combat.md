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
