# Everglade world events and levels 1 to 3

Date: 2026-11-03. Status: specification, with the build list in section 12. Epic: #12737, under
the Everglade multiplayer epic #12718.

This spec adds world events to Everglade (`/worlds/everglade`, Everglade 5E). A world event is a
timed happening that the room server runs for everyone in an instance at once. The first event
type is the **Cultist Raid**: waves of cultists and low-level warlocks, led by a level 17 Fiend
Warlock, attack the town. The spec also adds character progression: every character starts at
level 1 and gains experience points (XP) up to level 3, fighting the raiders.

In the owner's words: "Figure out a system of world events where we trigger these things like
raids on the city, which will end up at a certain point. [...] An event that I can turn on or off
or configure the dials of the intensity. [...] I want people to have their character start at
level one and gain experience up to level three max fighting against these. So make them
killable, level one to three, but the super high level warlock should not be killable."

Related reading:

- [The Everglade multiplayer spec](2026-11-03-everglade-multiplayer-spec.md): the room server
  (section 3), persistence (section 7) and admin edits (section 6.8).
- The playable level 17 Warlock (#12734), whose spells the raid's leader casts.
- The 5E Lab Siege (`apps/openagents-web/fivelab-siege.coax`,
  `aethyr/rules/srd/npcs.coax`): the cult's stat blocks and its mix, which the raid reuses.

## Contents

1. Goals
2. What exists today
3. World events
4. The Cultist Raid
5. The warlock who can't be killed
6. Progression: XP and levels 1 to 3
7. Rewards and aftermath
8. Admin controls
9. Server authority, the wire and performance
10. Persistence
11. Testing
12. Build list and issues
13. Open questions for the owner

## 1. Goals

- An admin turns a world event on or off for the Everglade realm, starts one now or on a
  schedule, and sets its intensity. Changes reach every instance on every shard within a few
  seconds, with no deploy.
- Every event ends at a point that everyone can see coming: a timer, a wave count, or an
  objective. The banner says which.
- The raiders are SRD 5e creatures at levels 1 to 3. Players can kill them and earn XP for it.
- The raid's leader can't be killed, and the page always shows why a hit didn't hurt it.
- A character starts at level 1, reaches level 2 at 300 XP and level 3 at 900 XP, and stops at
  level 3. The cap shows on screen.
- Everything runs in the room server's simulation, so it is the same for everyone in an
  instance. A full raid costs the tick little enough to keep the p95 tick time under 5 ms.
- Nothing changes for players until an admin switches it on: the event and the levels are off by
  default.

## 2. What exists today

- **The room server** (`coax/web/room/room-everglade.coax`) runs each Everglade instance at
  30 Hz: up to 16 seats, the town's 38 people, summoned creatures, zones, the town's pieces
  breaking and burning, and the R rebuild. The SRD 5e rules for a room are
  `apps/openagents-web/everglade5e-room-rules.coax`, one battle of up to 216 creatures
  (`RS_MAX`). Today it holds 16 seats, 38 people and 32 summoned creatures: 86.
- **Characters.** A seat plays Everglade's druid (the SRD 5.2.1 Druid at level 9, with a house
  rule's level 17 spell slots) or the level 17 Fiend Warlock (#12734). Nobody gains XP.
- **Persistence** (#12715): the server reports each ticketed seat's HP to
  `POST /api/playtest/room-vitals`, and the seat ticket brings it back for 10 minutes
  (`verse.room_vitals`).
- **Admin control** (#12714): every 2 seconds, each oa-room shard asks
  `GET /api/playtest/room-control` for the realm's `pvp` row, the edit stamp and the kicks.
  Realm admins (`verse.edit_may('everglade', ...)`) kick from a player's nameplate.
- **Instances** (#12717): a realm's players are spread over instances of 16 seats, on 6
  oa-room shards.

## 3. World events

### 3.1 Event types

An event type is a kind of happening with its own content and its own dials. The framework
knows the lifecycle, the end conditions, the banner, XP and the rewards; a type supplies what
spawns and how it behaves.

| Type | Status | What happens |
| --- | --- | --- |
| `raid` (Cultist Raid) | First | Waves of cultists attack the town from its gates, led by a level 17 Fiend Warlock. |

Later types fit the same frame. For example, a wolf hunt in the wood, or a flood that the
players hold back at the bridges. Each new type is its own issue.

### 3.2 Lifecycle

Each instance runs at most one event at a time. An event moves through these phases:

| Phase | Lasts | What happens |
| --- | --- | --- |
| `idle` | Until a start | Nothing. The banner shows the next scheduled start, if there is one. |
| `announce` | 30 s | The banner says what is coming and from where. Horns sound; nothing spawns yet. |
| `wave` | Until the wave is down, or 90 s | One wave spawns at the gates and attacks. |
| `lull` | 15 s | A pause between waves. The banner counts down to the next wave. |
| `climax` | Until the leader withdraws, or 3 minutes | After the last wave, the leader comes in person. |
| `end` | 5 s | The outcome shows: the town held, or the town fell. |
| `aftermath` | 60 s | Rewards are paid. Raiders who are still up flee. The town rebuilds. |

An event starts in one of two ways:

- **On a schedule.** The admin sets a period, for example every 60 minutes. Each start is at a
  wall-clock time that is a whole multiple of the period, so every shard and instance starts the
  same raid at the same moment.
- **Now.** The admin presses **START NOW**. Every live instance starts within 2 s, which is the
  control poll's period. An instance that opens after the start doesn't join a raid in progress;
  it waits for the next one.

An admin's **STOP** ends every running event at once. Raiders vanish, the outcome is "called
off", and no completion bonus is paid. XP already earned stays.

### 3.3 How an event ends

Every event ends. Each type names its end conditions, and the banner always shows the one
that is closest. For the Cultist Raid, the first of these to happen ends it:

| End | Outcome | How the banner shows it |
| --- | --- | --- |
| Every wave is beaten and the leader withdraws | The town held | `WAVE 3 OF 3`, then `MALGRIN WITHDRAWS` |
| The time limit runs out | The town held if the square is still held; else it fell | `9:12 LEFT` |
| The raiders hold the square for 60 s | The town fell | `THE SQUARE: CULT 0:42` |
| The town's people are all down | The town fell | `TOWNSFOLK 6 OF 38 STANDING` |
| An admin presses STOP | Called off | `CALLED OFF` |

The square is the town's central square, a circle of 18 m about its well. It is held by
whichever side has more creatures standing in it; players and the watch count for the town.

## 4. The Cultist Raid

### 4.1 The cult

The cult reuses the 5E Lab Siege's stat blocks and mix (`aethyr/rules/srd/npcs.coax`,
`rs_siege_attacker`), so the Lab and Everglade field the same armies. A wave is drawn from the
cult's mix, scaled by the admin's dials:

| Raider | SRD 5.2.1 | AC | HP | Attack | XP |
| --- | --- | --- | --- | --- | --- |
| Cultist | Cultist, CR 1/8 | 12 | 9 | Ritual Sickle, +3, 1d4 + 1 Slashing plus 1 Necrotic | 25 |
| Cultist Fanatic | Cultist Fanatic, CR 2 | 13 | 44 | Pact Blade, +4, 1d8 + 2 Slashing plus 2d6 Necrotic; Hold Person 1/Day | 450 |
| Warlock, level 1 | Fiend Warlock, class level 1 | 13 | 9 | Eldritch Blast, 1d10 Force | 50 |
| Warlock, level 2 | Fiend Warlock, class level 2 | 13 | 15 | Eldritch Blast with Agonizing Blast | 100 |
| Warlock, level 3 | Fiend Warlock, class level 3 | 13 | 21 | Eldritch Blast; Dark One's Blessing | 200 |

The SRD gives a Challenge Rating and XP for stat blocks, not for class levels. The warlocks' XP
is a house rule: a level 1 warlock counts as CR 1/4, level 2 as CR 1/2 and level 3 as CR 1, the
SRD's XP for those ratings. The raid caps the cult's warlocks at level 3 (the Siege's go to 5).

### 4.2 Waves

- Each wave spawns at the gates the admin picked: north road, east road, south road, the west
  wood and the river bridge. Raiders of a wave spawn in groups of up to 8, spread over those
  gates.
- Raiders march on the square by the town's roads. On the way, each attacks whatever is
  nearest within 12 m: a player, then a guard, then a townsperson.
- Cultists and Fanatics run in and strike in melee. Warlocks stop at 15 to 25 m and cast
  Eldritch Blast. A Fanatic casts Hold Person at the strongest player in reach once a raid.
- When the town's damage dial is on, the warlocks' blasts break and burn the town's pieces the
  way a player's do (spec 6.6). When it is off, the raid harms only creatures.
- A wave is beaten when all its raiders are down. The next wave starts after a 15 s lull, or
  at once if the 90 s wave timer runs out first; a wave still up then stays and fights.

### 4.3 How raiders decide

Raiders run the room server's own cheap routine (section 9.3), not the Lab's full planner. The
choices are the Siege's: who to attack first, when to cast, and when to fall back. The rolls are
the SRD engine's (`rs_roll_attack`, `rs_damage`, saving throws), and every roll goes to the
room's log, as a player's does.

## 5. The warlock who can't be killed

The cult's leader is Malgrin, the SRD 5.2.1 Fiend Warlock at level 17 (`rs_siege_arch`), with
#12734's spells: Eldritch Blast, Fireball, Blight, Circle of Death, Finger of Death and
Dimension Door. Players can fight Malgrin, and their hits do show, but Malgrin never dies. The
fiction and the page both say why.

### 5.1 The wards

Malgrin enters the town in the climax behind three **ward stones**: dark obelisks that the cult
raises at three of the gates as the climax starts.

- While any ward stone stands, nothing hurts Malgrin. Every hit on Malgrin shows `WARDED`
  instead of a number, and a beam runs from each standing stone to Malgrin, so everyone can
  see where the protection comes from.
- A ward stone is an object with AC 15 and 60 HP that every player can attack. The banner
  shows `WARDS 3 OF 3`.
- When the last stone breaks, Malgrin is exposed: the banner says `MALGRIN EXPOSED`, and hits
  hurt Malgrin as they would any creature.

### 5.2 The retreat

Malgrin's HP never reaches 0:

- When Malgrin drops to half its Hit Points or below, it casts Dimension Door (a 5th-level Pact
  slot; the SRD's 500-foot teleport) out of the town, and the raid ends with
  `MALGRIN WITHDRAWS`. The town held.
- When the climax's 3 minutes run out, Malgrin withdraws the same way, unhurt. The town held
  if the square was held.
- Damage that would take Malgrin below half its Hit Points takes it to half, then it
  withdraws. The page shows the hit's real number, then `DIMENSION DOOR`.

In the UI, Malgrin's nameplate reads `MALGRIN · LEVEL 17 · CAN'T BE KILLED`, and its HP bar
shows a mark at half, labelled `WITHDRAWS`. Nothing about this is silent: every protected hit
says `WARDED`, and the retreat says `DIMENSION DOOR`.

### 5.3 Its aggression

The admin's aggression dial sets how Malgrin fights:

| Value | What Malgrin does |
| --- | --- |
| 0, watches | Stays at the square's edge and casts nothing. The wards still hold. |
| 1, harries | Casts Eldritch Blast and Blight. |
| 2, attacks | Also Fireball, at groups of three or more. The default. |
| 3, razes | Also Circle of Death and Finger of Death, and casts twice as often. |

Malgrin's spells use the SRD's damage, so at aggression 3 a level 1 character in a Fireball
drops at once. The banner shows the aggression in words, so players know what they face.

## 6. Progression: XP and levels 1 to 3

### 6.1 The levels

When the admin turns on **Levels 1 to 3**, every character is built at its level from the SRD
5.2.1 class rules, instead of Everglade's level 9 druid and level 17 warlock:

| Level | XP (SRD 5.2.1, Character Advancement) | Druid (Circle of the Land) | Warlock (Fiend Patron) |
| --- | --- | --- | --- |
| 1 | 0 | 2 1st-level slots; cantrips; HP 9 | 1 1st-level Pact slot; Eldritch Blast; HP 9 |
| 2 | 300 | 3 1st-level slots; Wild Shape, Wild Companion; HP 15 | 2 Pact slots; Agonizing and Repelling Blast; HP 15 |
| 3 | 900 | 4 1st, 2 2nd-level slots; Land's Aid; HP 21 | 2 2nd-level Pact slots; Dark One's Blessing; HP 21 |

The numbers are the engine's own `rs_druid` and `rs_warlock` at that level. Cantrips scale
with the character's level as the SRD says.

- A spell needs a slot of its level. A level 1 druid can't cast Moonbeam (2nd level) or Meteor
  Swarm, so the server refuses the cast with `r SEQ level` and the page says
  `NEEDS LEVEL 3`. The page also dims those slots on the bar.
- Level 3 is the cap. At 900 XP the XP bar fills and reads `LEVEL 3 · MAX`. XP past 900 is
  kept but gives nothing more.
- When the switch is off, characters play as they do today, and XP still counts and saves.

### 6.2 Earning XP

- A raider gives its SRD XP (section 4.1), times the admin's reward multiplier.
- **The split in multiplayer.** The SRD divides an encounter's XP among the characters who took
  part. The room does the same per raider: its XP is split evenly among the seats that hurt it
  or healed someone who did, in the 30 s before it fell, and who are within 60 m of where it
  fell. Each share is rounded down, and at least 1.
- A ward stone gives 100 XP, split the same way. Malgrin gives no kill XP, since it can't be
  killed; its withdrawal pays the completion bonus (section 7).
- Townsfolk, guards and other players give no XP.
- The page shows each gain over the raider (`+25 XP`) and a level-up as a banner
  (`LEVEL 2`). Every nameplate shows its player's level.

## 7. Rewards and aftermath

- **The completion bonus.** When the town holds, every seat that earned XP in the raid gets
  100 XP times the multiplier. When the town falls or the raid is called off, there is no
  bonus.
- **The aftermath.** For 60 s after the end, raiders still standing flee to the gates and
  vanish, and Malgrin's Zombies fall. At the aftermath's end, the server runs the R rebuild
  itself (spec 6.6): the town's pieces stand, the fires go out, and the town's people stand up
  whole. The R key's 10 s and 30 s rules don't hold the server's own rebuild back.
- A character who drops in a raid falls as they do today: they can be healed, and they keep
  their XP.

## 8. Admin controls

### 8.1 Where they are

Realm admins (`verse.edit_may('everglade', key, site)`: the realm's owner, its admins and the
site's admins) see an **EVENTS** button in Everglade's top row. It opens the events pane:

- **EVENT ON / OFF**: whether events run at all.
- **START NOW** and **STOP**.
- **EVERY**: the schedule's period, or off.
- The dials, each with its range written beside it.
- **SAVE**: the dials go to the API, and every instance hears them within 2 s.

Everyone else sees only the banner. The pane posts `POST /api/playtest/room-event` through the
page's API pipe, as KICK does (#12714).

### 8.2 The dials

Every limit shows on screen, next to its dial. The server enforces the same ranges, and a
value outside one is set to the nearest end and shown as set.

| Dial | Range | Default | What it does |
| --- | --- | --- | --- |
| Event | on, off | off | Whether events start, on a schedule or by START NOW. |
| Every | off, 15 to 240 minutes | off | The schedule's period. |
| Waves | 1 to 8 | 3 | How many waves before the climax. |
| Wave size | 4 to 24 raiders | 12 | Raiders in each wave. |
| Cultists, Fanatics, Warlocks | 0 to 10 parts each | 6, 1, 3 | The wave's mix, in parts. |
| Warlock levels | 1 to 3 | 3 | The cult's warlocks are levels 1 up to this. |
| Gates | any of 5 | all 5 | Where waves spawn. |
| Leader | on, off | on | Whether Malgrin comes in the climax. |
| Aggression | 0 to 3 | 2 | How Malgrin fights (section 5.3). |
| Town damage | on, off | on | Whether the raid breaks and burns the town. |
| Reward | 50% to 300% | 100% | The XP multiplier. |
| Time limit | 5 to 30 minutes | 15 | The whole raid, from the announcement. |
| Levels 1 to 3 | on, off | off | Characters at levels 1 to 3 from their XP (section 6.1). |

At most 48 raiders stand in an instance at once, waves together, and the pane says so beside
**Wave size**: `48 STANDING AT MOST`. A wave that would go past it spawns the rest as others
fall.

### 8.3 The API

- `GET /api/playtest/room-event`: the realm's dials as JSON, for the pane.
- `POST /api/playtest/room-event {dials...}`, `{start: true}` or `{stop: true}`: a realm
  admin's change. Anyone else gets 403.
- `GET /api/playtest/room-control` adds two lines for the shards:
  `event ON EVERY WAVES SIZE CULT FAN WL WLMAX GATES ARCH AGGR HARM REWARD MINUTES START STOP`
  and `levels 0|1`. START and STOP count each press, so a shard acts on a press once.
- The dials live in a new table, `verse.room_events`, one row per realm. The migration is
  additive and safe to run twice.

## 9. Server authority, the wire and performance

### 9.1 Where it runs

- The event, its raiders, Malgrin, the ward stones, the XP and the levels all run on the room
  server, in `coax/web/room/room-events.coax` and the rules' raid unit
  `apps/openagents-web/everglade5e-room-raid.coax`. The shared files get one-line hooks only.
- Raiders are creatures of the room's one SRD battle, after the summoned creatures: 48
  raiders, Malgrin and 3 ward stones. The battle grows from 86 creatures to 138, inside
  `RS_MAX` (216).
- Each instance runs its own raid on its own copy of the town. Starts agree across instances
  and shards because each comes from the wall clock (a schedule) or a counted press (START
  NOW) that every shard reads from the API.

### 9.2 The wire

New D entries, which pages that don't know them skip:

- `g KIND PHASE WAVE WAVES SECS STANDING SLAIN SQUARE WARDS ARCH OUTCOME`: the event's state,
  heard by every seat on change. SECS is the phase's seconds left; SQUARE who holds the square
  and for how long; ARCH Malgrin's state (away, warded, exposed, withdrawn).
- `v SS LEVEL XP`: a seat's level and XP, heard by every seat on change, for nameplates and
  your XP bar.
- `e xp SS AMOUNT` and `e level SS LEVEL`: a gain and a level-up.
- `e ward EID`: a hit that the wards stopped, so the page shows `WARDED` over Malgrin.
- Raiders, Malgrin and the stones are `c` entries (creatures, spec 3.6), with new kinds:
  `WK_RAIDER` (P1 the stat block, P2 the most HP) and `WK_WARD`.
- A cast refused for the character's level is `r SEQ level`.

### 9.3 Performance

The latency work (#12731) holds the tick's p95 under 5 ms at 32 players. A full raid in an
instance must add less than 0.5 ms to that instance's tick. The design keeps it cheap:

- **Decisions on the beat.** A raider chooses its target once a beat (1.5 s, the room's turn),
  with each raider's beat offset, so about 1 raider in 45 decides each tick. A choice scans
  the 16 seats and the 38 people: about 60 distance checks.
- **Moves each tick.** Between decisions, a raider steps toward its goal on a straight line, as
  the summoned wolves do (`e5rr_beasts_step`), with no path search. Roads and gates give the
  routes.
- **Attacks through the engine** on the beat only.
- **Written once, heard by many.** Each raider's `c` entry is written and hashed once a tick,
  as the players' entries are (`eg_cache`), and each seat compares hashes. A raider is heard
  again when it moves 0.5 m or turns 0.3 radians, or when its HP changes.
- **Bandwidth.** 48 raiders in view, moving, cost a seat about 20 KB/s. Raiders more than
  120 m away aren't sent (`EG_VIEW`).

`load.coax` (#12711) gets a raid case: 16 seats in one instance and 48 raiders fighting, which
reports the tick's p95 with and without the raid.

## 10. Persistence

XP saves the way HP does (#12715):

- The server's vitals report becomes `PID HP MAX XP` a line. The API keeps XP in a new
  `verse.room_vitals.xp` column, by account when the player is logged in, else by player.
  XP only rises: the API keeps the larger of the old and the new.
- The seat ticket brings XP back: its HP field becomes `HPxXP` (for example `14x320`, or `-x320`
  with no HP to bring back). A ticket without the `x` part means 0 XP, so old tickets still
  seat.
- The level is computed from XP, never stored.
- XP doesn't expire. Unlike HP, it comes back on any visit.

## 11. Testing

- **Room checks** (`coax test coax/web/room/room.coax --only "events:"`): the lifecycle from
  start to aftermath on a test clock; each end condition; the dials' clamping; XP split among
  two seats; level-up at 300 and 900 and the cap; a level 1 seat refused a 2nd-level spell;
  Malgrin's wards, retreat at half HP, and never 0 HP.
- **API checks** (`coax/web/api/test_room.py`): `room-event` refuses visitors; dials round-trip;
  `room-control`'s new lines; XP kept by the larger value.
- **Page checks** (`everglade5e.coax --only "events:"`): the banner from `g` entries, the XP
  bar, `WARDED`, and the admin pane's dials and ranges.
- **Load**: section 9.3's raid case.

## 12. Build list and issues

Epic #12737 holds the work. Each part is its own issue, built in this order:

| Part | Issue | Status |
| --- | --- | --- |
| The event framework on the server, the API's dials and the admin pane, off by default | #12738 | In progress |
| XP and levels 1 to 3, with persistence | #12739 | Planned |
| The Cultist Raid: raiders, waves, the AI and drawing them | #12740 | Waits on the Siege's stat blocks landing on `main` |
| Malgrin: the wards, the retreat and its spells | #12741 | Waits on #12740 |
| Rewards, aftermath and the raid load test | #12742 | Waits on #12740 |

## 13. Open questions for the owner

1. Should **Levels 1 to 3** be on whenever an event is on, or stay a separate switch? This spec
   keeps it separate, so the classic level 9 druid stays until you choose.
2. Should XP come from anything outside a raid, such as the wood's wolves?
3. When the town falls, should anything else happen, such as the cult holding the square until
   the next rebuild? This spec rebuilds the town in the aftermath either way.
