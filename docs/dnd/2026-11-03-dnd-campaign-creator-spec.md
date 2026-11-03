# The campaign creator: run a 5E campaign in the Verse

Status: specification, 2026-11-03. Nothing on this page is built yet except
the pieces named in [What it builds on](#what-it-builds-on). The rules system
it uses is specified in [README.md](README.md).

This document specifies a campaign creator and table for game masters who run
fifth-edition campaigns: what you can make, what happens at the table, how AI
helps without taking over, the ways to play, and the onboarding from first
visit to first session. It ends with open questions and a build order.

## Why we are building it

On the Episode 290 recording (2026-10-30, [transcript](../transcripts/290.md)),
Christopher tells the story that started this. A cashier at Target overheard
him talking about the hobby project, an MMORPG on the 5E rules, and he showed
the druid spells ported to MMO-style play. The cashier's response:

> "oh my gosh, would this let me, as a DM, dungeon master, like create my own
> worlds and like, like, have a 3D world for the, like, the, the, the
> campaigns that I lead my people to? Like, yes, absolutely." [06:30]

He then frames who would pay for it:

> "The type of people that spend money on their workbooks and their books and
> their figurines, like, would they spend $10 a month on a subscription for
> this or compute or customizing assets? Yes, they would." [06:30–07:00]

Later in the talk, he names the base it stands on: "We've got DND, the SRD 5e
rules already like partially implemented" [33:30]. He also describes AI
NPCs that coordinate as "truly formidable enemies" [26:00], and a Verse
marketplace where makers sell assets they build [08:30–09:30]. Marc's pitch,
"Balloons Tower Defense, but it's D&D" [18:30], became Hold the Stair, the
second world on the shared rules.

So the product is this: a game master builds a campaign, and then runs it
with friends at a real table or remotely. Dice, turns, character sheets and
rules come from the SRD engine. Maps, art, sound and live 3D scenes come from
the Verse.

## Names: what you see

Product copy follows the naming rules in [README.md](README.md#legal-and-naming):

- The product says **5E** or **SRD 5e**, and "compatible with fifth edition".
  It never says "Dungeons & Dragons" or "D&D", and never uses their logos,
  settings or non-SRD names. This folder and this file name say D&D only
  because they are internal.
- The person who runs the game is the **Game Master (GM)** on screen. This
  document says GM throughout. Before launch, check the SRD 5.2.1 PDF for the
  term it uses and match it.
- The product's screens: **Campaigns** (your list), the **Campaign Builder**
  (prep), the **Table** (a live session), the **Table Display** (the shared
  screen), **My Character** (a player's phone or laptop) and the **GM Screen**
  (the GM's private view during play).

No system-internal words reach the screen. A player sees "Roll", "Your turn"
and "Session recap", never "event", "authority", "replay" or "seed".

## What it builds on

| Piece | Where | What the campaign creator uses it for |
|---|---|---|
| SRD 5e rules engine | `aethyr/rules/srd/engine.coax`, `spells.coax`, `battle.coax` | Dice (a deterministic SplitMix64 stream), checks, saves, attacks, hit points, death saves, conditions, spells and their effect steps, the battle grid of 5-foot squares, initiative |
| The 12 classes to level 6 | `battle.coax`, one SRD subclass each (Land Druid, Evoker, Fiend Warlock, Draconic Sorcerer, Lore Bard, Life Cleric, Champion, Devotion Paladin, Hunter, Thief, Berserker, Open Hand) | Character sheets and player turns for levels 1 to 6 |
| The time adapter | [README.md, "The time adapter"](README.md#3-the-time-adapter-one-rule-set-two-clocks) | **SRD rounds** for the Table, **action combat** for a live 3D fight or a persistent world |
| 5E Lab | `apps/openagents-web/fivelab.coax`, [realms/5e-lab/index.md](../../apps/openagents-web/realms/5e-lab/index.md) | The two views of a fight (the flat grid and the 3D glade), the CHARACTERS panel, condition marks on figures, the dice log, Player mode and its class hotbar on keys 1 to 9 |
| Everglade | `apps/openagents-web/everglade-*.coax`, [docs/verse/everglade.md](../verse/everglade.md) | A shared world with a town, guards that escalate, a crypt "a dungeon later", and the destruction hotbar. The base for a persistent campaign world |
| Hold the Stair | [game-ideas/hold-the-stair.md](../verse/game-ideas/hold-the-stair.md) | A second world on the same rules, and lockstep play over the relay |
| Realms and their editing | [realm-host.md](../verse/realm-host.md), [realm-edit.md](../verse/realm-edit.md), [aethyrforge.md](../verse/aethyrforge.md), book ch051–ch055 | A campaign's 3D locations are realms: made from a sentence, edited in place, published with a link and forked |
| Agents in a realm | [realm-seat.md](../verse/realm-seat.md) | An AI player or an AI GM takes a seat with the same verbs a person uses |
| Characters who talk | `coax/web/api/npc.c`, book ch077 | Improvised NPCs that answer in voice or text |
| Voice | book ch069, `aethyr/voice.coax` | Hold to talk; the GM speaks to the copilot, a player casts by name |
| Sound | book ch065–ch068 | Music and ambience from numbers, a sequencer and a mixer |
| Effects | book ch072 (design draft), `aethyr/particles.coax` | Spell effects, fog, torchlight and weather in 3D scenes |
| Rooms and shared state | book ch049–ch050, `coax/web/room/` | Who is at the table, and the same session state on every screen |
| Saves | book ch081, `apps/openagents-web/save.coax` | A character and a campaign that persist between sessions |
| Dice on the wire | `verse.rules.v1` `DiceReveal` in [openagents-verse-protocol.md](../verse/openagents-verse-protocol.md) | A roll's expression, faces and total, revealed after it resolves |

## Who uses it

| Person | Where | What they need |
|---|---|---|
| **The GM** | A laptop behind a real screen at the table, or a desktop when remote | Prep that is fast, a private view of the whole session, control of what everyone sees, rules answers in seconds, and the final word on every ruling |
| **Players at the table** | Their own phone, or nothing but paper and real dice | Their character sheet, a dice roller if they want one, their turn, and handouts the GM sends them |
| **Remote players** | A browser on a laptop or a phone | Everything a player at the table has, plus the Table Display's picture and the table's voice |
| **The Table Display** | A TV, a projector, or a monitor laid flat as a battle mat | The shared picture: the map with its fog, the scene art, the initiative order and open rolls. No controls of its own |
| **Spectators** | A browser with a watch link | The Table Display's picture and the open log, for a stream or a friend who drops in. Never the GM's private view |
| **A co-GM** | Any screen | The GM's view and tools, given by the GM, with the GM still the owner |
| **Agents** | The CLI or MCP tools | The same verbs as a person, through a seat. An agent never sees more than the seat it holds |

## The campaign creator

A campaign is one document that holds everything below. Each part is a typed
record, the way a realm is a document (book ch051), so the GM can edit it by
hand, by voice or through an agent, and every change has an author.

```
Campaign
├── Setting: world, regions, maps, factions, calendar
├── House rules: the campaign's ruleset profile
├── Safety: lines, veils, tools, tone
├── Cast: NPCs, monsters, the player characters
├── Library: loot, handouts, art, music
├── Adventures
│   └── Sessions
│       └── Scenes
│           └── Encounters
├── Quests and clocks
└── Log: every session's events and recap
```

### Setting and world

- **Campaign card:** name, a one-line pitch, a cover image, the level range
  (for example 1 to 5), the expected number of players (3 to 6 is the
  default), how often you play, and the tone (see [Safety](#tone-and-safety-tools)).
- **World:** a short description, written by you or drafted by the copilot
  from a sentence ("a drowned coast of fishing towns and an old sea-god
  cult"). The world is the campaign's own content, marked as yours.
- **Regions:** named places with a description, a map, the NPCs who live
  there, the dangers (encounter tables), and a link to a 3D realm if you made
  one.
- **Maps:** three kinds, at three scales.
  - A **world map** or **region map**: an image you upload or one the copilot
    draws, with pins for places. Travel is by miles and days.
  - A **battle map**: the 5-foot grid from 5E Lab, with walls, doors,
    difficult terrain, light and fog. You draw it with a brush, import an
    image and align the grid over it, or start from a template room.
  - A **3D scene**: a realm made in Aethyrforge, or one of the existing
    worlds (the glade from 5E Lab, Everglade's town and crypt). The scene
    carries the same 5-foot grid, so the battle map and the 3D scene are two
    views of one place, the way 5E Lab's V key switches 2D and 3D.
- **Factions:** groups with goals, a stance toward the party, and a clock
  each (see [Quests and clocks](#quests-and-clocks)).
- **Calendar:** in-world days, so travel, rests, downtime and clocks advance
  on one timeline.

### Adventures, sessions, scenes and encounters

The GM prepares at four levels. Each level can be as thin or as detailed as
the GM likes; a session can be a single line of notes.

| Level | What it holds |
|---|---|
| **Adventure** | A goal, a hook, the regions and NPCs it uses, the level it starts and ends at, and an outline of scenes. Either a starter adventure, an imported one of your own, or one you write |
| **Session** | A date and time, who is coming, the scenes you plan to reach, your notes, and after play its recap |
| **Scene** | Where (a map or a 3D scene), who is there, what the players can learn (read-aloud text, clues, handouts), the art and music, and what can happen next. Scenes link to each other, so a session plan is a small graph, not a script |
| **Encounter** | A fight, a social scene or a trap. A fight lists its creatures and where they stand, the terrain, the light and the win and lose conditions. The builder shows its difficulty against the party, from the SRD's encounter-building rules |

Every encounter has a **Run it** button that opens it at the Table, with the
creatures on the grid and initiative ready to roll.

### NPCs and monsters

- **SRD creatures:** every stat block in the SRD's Monsters and Animals
  chapters, browsable and searchable by name, type, Challenge Rating,
  environment and what it does. Search is semantic, over the records (see
  [How AI is wired](#how-ai-is-wired)). Today the engine holds a few stat
  blocks (the Skeleton, Guard, Commoner, Veteran, Mage, Knight, Priest,
  Archmage, Gladiator); the import in README step 2 brings the rest.
- **Adjusting a creature:** change hit points, add a weapon, rename it,
  reskin its look. An adjusted creature becomes your own record that names the
  SRD creature it came from, marked as yours (`srd: false`).
- **Your own creatures:** a full stat block in the SRD's shape, with a
  builder that checks the numbers against its Challenge Rating.
- **NPCs:** a name, a look (a portrait or a 3D figure), a voice, a
  personality in three lines, what they want, what they know, what they will
  never say, and a stat block if they might fight (often a Commoner, Guard or
  Mage). An NPC can be **voiced by the copilot** at the table (see
  [At the table](#improvised-npcs)).
- **Figures:** each creature and NPC has a figure for the grid and the 3D
  scene. The default figures come from 5E Lab's class looks and the
  creatures' default looks. Better ones can come from the Verse marketplace.

### Loot, handouts, quests and clocks

- **Loot:** SRD equipment and magic items, plus your own items, marked as
  yours. A treasure table per region or encounter. Loot you award lands on a
  character's sheet, and the engine applies what it does (an item's bonus to
  AC or attack rolls, its charges, attunement).
- **Handouts:** an image, a letter, a map fragment, a riddle or a sound
  clip. Each handout is hidden until you send it, to everyone or to one
  player. A handout can reveal part of a map.
- **Quests:** a goal, who gave it, the reward, its status (offered,
  accepted, done, failed) and which scenes advance it. Players see the quests
  they know about in **My Character > Quests**.
- **Clocks:** a progress clock with 4, 6 or 8 segments, for a threat ("the
  cult finishes the ritual"), a faction's plan or a timer in a scene. You
  tick a clock by hand, or the copilot proposes a tick when the session log
  shows the cause. A clock can be secret or shown on the Table Display.

### House rules

House rules are the campaign's **ruleset profile**, the same format worlds use
([README.md, "World tweaks"](README.md#4-world-tweaks)). The builder shows
them as switches, not as a file:

- Which classes players can choose (all 12 by default).
- Starting level, and leveling by XP or by milestone.
- Ability scores by standard array, point buy or rolling.
- Critical hits, encumbrance, material component costs, resting (short and
  long rests as written, or a gritty or heroic variant), and death saves
  shown openly or hidden.
- Granted spells or features outside a class's list, each one named.
- Your own rules as text, for things the engine doesn't run.

The rule from README holds: a tweak names exactly what it changes, and
everything else is the SRD's. A character sheet shows a small mark next to any
number a house rule changed, and tapping it shows the rule.

### Tone and safety tools

Safety is part of setup, not a footnote. The builder has a **Safety** page
that the GM fills in and the players add to in session zero.

- **Tone:** a few sliders the group agrees on: combat, horror, romance,
  humor, how dark the world gets. Each slider shows what the setting will
  and won't include.
- **Lines and veils:** a **line** is content that never appears. A **veil**
  happens off screen, and the story cuts away. Players add their own lines
  and veils privately; the GM sees the combined list, not who added what.
  The copilot treats lines as hard limits for anything it writes and veils as
  "cut away".
- **The X-card:** every player screen and the Table Display have an **X**
  button. Tapping it shows the GM, and only the GM, that someone wants the
  current content gone, with no reason needed and no name shown by default.
  The GM's screen shows **Skip this** and **Rewind**. Other tools the group
  can turn on: **Pause** (a time-out) and an **open door** note (anyone can
  leave a session, no questions).
- **Check-ins:** at the end of a session, an optional two-question check-in:
  what worked, and what to change.

## At the table

The Table is the live session. The GM runs it from the GM Screen. Players use
My Character. The shared picture is on the Table Display, or on each remote
player's screen.

### The GM Screen

One screen, laid out for a laptop:

- **Left:** the session plan (scenes as cards, the current one lit), quests
  and clocks.
- **Center:** what the table sees, with a toggle to see your private layer
  over it (hidden creatures, unrevealed fog, secret doors, notes).
- **Right:** the initiative tracker, the selected creature's stat block, and
  the copilot.
- **Bottom:** the dice tray and the session log.
- Buttons along the top: **Show to table**, **Hide**, **Send handout**,
  **Start combat**, **End combat**, **Short rest**, **Long rest**, **Pause**,
  **End session**.

### Dice

Dice are central, and the group picks how they roll. The choice can differ per
person: the GM can roll virtual dice while a player rolls physical ones.

| Option | How it works | When to pick it |
|---|---|---|
| **Physical dice** | You roll real dice and type the faces in (or tap them on a d20 pad). The engine applies modifiers and works out the result | Groups who love their dice. The default at an in-person table |
| **Virtual dice** | Tap **Roll** on a sheet, a hotbar slot or the dice tray. The engine rolls from its deterministic dice stream, shows the faces and the total, and applies the result | Remote play, fast combat, and players without dice |
| **Verifiable dice** | Virtual dice that anyone can check after the session. See below | Remote groups, online campaigns, AI GMs, and anything with stakes |

**Open or behind the screen.** Every roll is either open (everyone sees it as
it lands, the default for players) or hidden (only the GM sees it, the default
for the GM's monster rolls and secret checks). A hidden roll can be revealed
later with one tap. Death saves follow the house rule.

**How verifiable dice work.** The engine's dice are deterministic: the same
seed gives the same rolls. Before the session starts, the server picks a
session seed and shows everyone a fingerprint of it (its hash). Each player's
device adds its own random contribution when it joins, so no one, the GM or
the server, can pick the seed alone. Every roll draws the next numbers from
the combined stream, in order. When the session ends, the seed is revealed,
and anyone can replay every roll with **Check the dice** and see that it
matches. The `DiceReveal` message in the Verse protocol already carries a
roll's expression, faces and total. Physical rolls are marked as entered by
hand.

**What a roll shows.** The faces, the modifiers by name ("+5 Shortsword, +3
proficiency"), Advantage or Disadvantage with both dice, the total, and the
outcome ("Hits AC 14"). The log keeps every roll.

**Dice in 3D.** When the table is on a 3D scene, a roll can tumble as a die on
the scene, with the result already decided by the engine and the physics only
drawing it. A setting turns the tumble off for speed.

### Initiative and the turn tracker

- **Start combat** rolls initiative for every creature in the encounter
  (virtual by default, or entered by hand) and builds the order. The
  engine's initiative is a d20 plus the Dexterity modifier, as in
  `battle.coax`, with ties broken by the order creatures were added.
- The tracker shows each creature's portrait, hit points (exact to the GM,
  as a rough state to players for monsters: "unhurt", "bloodied", "down"),
  conditions and concentration.
- On a creature's turn, the tracker shows what it has left: its action, Bonus
  Action, Reaction, movement and spell slot. This is the turn economy that
  5E Lab already tracks.
- Buttons: **Next turn**, **Delay**, **Ready an action**, **Add creature**,
  **Remove**. Effects that last a number of rounds count down and end on
  their own. Repeat saves come up at the right moment.
- The tracker works in both styles of play below.

### Theatre of the mind and the grid

| Style | What the table sees | What the engine does |
|---|---|---|
| **Theatre of the mind** | Scene art, the initiative order and the log. No positions | Dice, damage, conditions and durations. Ranges and areas are the GM's call: when a spell needs targets, the GM picks them from a list |
| **The grid** | The battle map of 5-foot squares, with figures, walls, light and fog | Everything above, plus movement, reach, range, areas of effect, difficult terrain and lasting zones (Moonbeam, Spike Growth) |
| **The 3D scene** | The same grid in a live 3D world, as in 5E Lab's glade | The same as the grid. The camera can follow the creature whose turn it is |

The GM can switch style mid-session. A fight can start in theatre of the mind
and move to the grid when it gets tactical.

Today the engine has no Opportunity Attacks (5E Lab's page says so). The Table
needs them on the grid before it can claim the SRD's movement rules; until
then the GM resolves them by hand with a **Roll attack** button.

### Visual aids

- **Maps with fog of war.** The GM reveals the map by painting, by room, or
  automatically from what the characters can see (line of sight and light
  from the engine's light step). Unrevealed areas show as fog on the Table
  Display and are clear on the GM Screen.
- **Scene art.** A full-screen image or a 3D view for a moment ("the
  cathedral as you enter"), sent with **Show to table**. **Hide** takes it
  back.
- **Handing out images.** **Send handout** sends a handout to everyone or to
  one player, who gets it on My Character and keeps it under **Handouts**.
  The Table Display shows it only if the GM sends it there.
- **Music and ambience.** Each scene can have a track and an ambience
  (rain, tavern, cave drips), from the Verse's sound modules (book
  ch065–ch068) or files you upload. The GM's mixer has a volume per layer,
  and a **Combat** button cross-fades to the scene's combat track. Remote
  players hear it in time with the table.
- **Effects.** In a 3D scene, spells draw their looks (the shared default
  look per spell, and a world's own looks). Weather and time of day come
  from book ch071.
- **Pointers and pings.** Anyone can ping a square or a spot on the map so
  everyone sees where they mean.

### Live 3D scenes in the Verse

A scene's 3D view is a realm. The Table reuses what already runs:

- **5E Lab's rendering:** the glade traced per pixel, the class looks, the
  condition marks, the ring on whoever's turn it is, the hit-point bars and
  the concentration line. The flat grid view for theatre-of-the-mind maps.
- **Everglade's world:** a town, a crypt and guards. A campaign can set a
  session in Everglade or in a fork of it (book ch055) that the GM edits.
- **Aethyrforge:** the GM says "a ruined watchtower on a sea cliff at
  dusk", watches it appear, and moves things by hand before the session.
- **Walking in:** players can walk their characters around a scene between
  fights, as in 5E Lab's Player mode, and the GM can lock movement to turns
  when combat starts.
- **Spectator camera:** the Table Display can follow the action on its own
  or hold a view the GM sets.

### Character sheets

- The sheet is the engine's creature record: abilities, proficiency bonus,
  AC, hit points, speed, senses, saves, skills, attacks, spell slots,
  prepared spells, features and their uses, conditions and concentration.
  The sheet never holds a number the engine doesn't compute, except a house
  rule's or a hand-entered roll.
- Tapping anything rolls it: a skill, a save, an attack, a spell. The
  hotbar from 5E Lab holds the character's own abilities on keys 1 to 9 on a
  laptop, or as a row of buttons on a phone.
- Spending happens on the sheet: a slot used, a Rage, a Focus Point, a use
  of Second Wind. Rests restore what the SRD says.
- Leveling up is guided: the sheet shows what the new level gives and asks
  only for the choices the class makes at that level.
- Every spell and feature has its SRD text one tap away, with the
  attribution.
- Today the engine covers the 12 classes to level 6 with one subclass each.
  Species, backgrounds, origin feats and the levels above 6 wait on the SRD
  import (README step 2). Until then, character building offers what the
  engine runs, and says the level cap plainly.

### Session log and recap

- **The log** records the session as it happens: rolls, damage, conditions,
  turns, handouts sent, scenes shown, loot awarded, clocks ticked, and the
  GM's notes. Spoken lines are not recorded unless the group turns on
  transcription in session zero.
- **The recap.** **End session** drafts a recap from the log: what happened,
  who did what, what was found, what changed in quests and clocks, and
  where the party stands. The GM edits it and taps **Share recap**, which
  sends it to the players and pins it at the top of the next session.
- **Previously on.** The next session can open with the recap read aloud by
  a narrator voice, over the scene's music.
- **Hidden things stay hidden.** The recap and the player log never include
  hidden rolls, secret clocks or the GM's notes unless the GM reveals them.

## AI help

### The GM's copilot

The copilot sits in the GM Screen's right column and in the Campaign Builder.
You talk to it or type. It drafts; you decide.

**Prep:**

- Draft a setting, region, NPC, adventure outline, encounter or treasure
  table from a sentence.
- Balance an encounter for the party, with the reasons: "three Skeletons and
  a Mage is a hard fight for four level 3 characters".
- Turn your notes into scenes with read-aloud text, clues and handouts.
- Suggest which SRD creatures fit a place and a level.
- Make a battle map, scene art or a 3D scene for a scene.

**At the table:**

- **Rules lookups:** "Can I grapple while holding a shield?" The answer
  quotes the SRD's text and its section, so you can check it.
- **Improvised NPCs:** "The party asks the barkeep about the lighthouse."
  The copilot gives the barkeep a name, a voice and three lines in a
  second, consistent with the setting and what this NPC is allowed to
  know. It can save the NPC to your cast for next time.
- **Names and details on demand:** a tavern, a shop's stock, a random
  street event, from your setting.
- **Pacing:** a quiet nudge if a scene runs long or a player hasn't acted
  in a while. Off by default, and never shown to players.
- **Clocks and quests:** "the log shows the party freed the prisoners;
  tick the Rebellion clock?"

**After the session:** the recap draft, an updated NPC list, loose threads to
pick up, and a prep list for next time.

#### Improvised NPCs

An NPC the copilot voices speaks through the characters endpoint
(`coax/web/api/npc.c`, book ch077) with its personality, knowledge and limits
from the campaign. The GM chooses how it reaches the table:

- **Whisper to the GM:** the GM sees the line and says it in their own voice.
  This is the default.
- **Speak at the table:** the NPC speaks in its voice on the Table Display.
  The GM can stop it with one tap.

### An optional AI GM

For solo play, one-shots, or a group without a GM, an AI GM can run a
session. It uses the same verbs as a human GM, through a seat
([realm-seat.md](../verse/realm-seat.md)): show a scene, voice an NPC, start
combat, roll for monsters, send a handout.

- The AI GM runs on the same rules engine. It cannot change a roll or a
  result; it decides what monsters do and what the world says.
- It respects the campaign's tone, lines and veils, and the X-card stops it
  at once.
- A human can take over at any time with **Take the GM seat**, and the AI GM
  becomes the copilot.
- Monster tactics in the AI GM and in persistent worlds use the same
  decision path as the AI NPCs Christopher describes: enemies that
  coordinate [26:00].

### The GM keeps authority (hard rule)

- The copilot and the AI GM **propose**; the GM **accepts**. Every change
  the copilot makes to the campaign or the live session comes as a card with
  **Accept** and **Decline**, the same pattern realm editing uses
  ([realm-edit.md](../verse/realm-edit.md)). A GM can turn on "accept small
  things automatically" for names and descriptions, never for changes to
  creatures, loot, clocks or anything players see.
- Rolls and rules results come from the engine, not from a model. A model
  never writes a die face or a damage total.
- The copilot never shows anything to players on its own. Only the GM's
  **Show**, **Send** and **Speak** buttons reach the table.
- The GM can overrule any engine result with **Override** (a ruling), which
  the log records as the GM's ruling.
- When a human GM is present, an AI GM can't run. When an AI GM runs, the
  campaign's owner is still the authority and can pause or replace it.

### How AI is wired

The workspace's agent rules apply:

- **No keyword matching** for what the GM or a player wants. A spoken or
  typed request ("show them the map", "the barkeep says…", "roll a Stealth
  check for the rogue") goes to a central typed semantic selector that picks
  one of the Table's typed verbs. Bounded fields (a creature's name, a
  number of dice, a skill from the SRD's fixed list) are parsed only after
  the verb is chosen.
- **Rules lookups and creature search** use embedding search over the SRD
  records and the campaign's own records, and quote the record they found.
- **Decisions** (which verb a request means, which monster a creature
  targets, whether a log entry ticks a clock) go through our Decisions API,
  `/v1/decisions`, with Clef first. New code never calls a decision model
  directly.
- **Writing** (an NPC's lines, a recap, a description) uses the model path
  the rest of the Verse uses, Google first per our provider rules.
- Every AI output carries the campaign's lines and veils as hard limits.

## Ways to play

| Mode | What it is | What's different |
|---|---|---|
| **Table session** | A GM and a group play a session, in person, remote or mixed | The default. Everything above |
| **Persistent world (West Marches style)** | One shared world, several GMs or one, many players who form parties per session and come back to the same world. The MMO version | The world persists between sessions: a cleared dungeon stays cleared, a town remembers. Characters keep their level and gear (open question below). Sessions are posted as **expeditions** players sign up for. Can run in real time in action combat, like Everglade |
| **Solo play** | One player and an AI GM, or one player running several characters | The AI GM is on by default. Pause and resume at any point |
| **One-shot** | One session, pre-built characters, a short adventure | No setup beyond picking the adventure. Characters are ready to play. The fastest way to try the product |

A persistent world builds on Everglade. Today Everglade is single player; the
transcript says it and the other realms are "a few days away from also turning
these into multiplayer realms" [07:30], and book ch049–ch050 plan the rooms
and shared state.

## Onboarding and setup

Speculative, screen by screen. Each screen names its buttons. A GM can stop
after any screen and come back; nothing is lost.

### The GM's first run

**Screen 1: Campaigns (empty).** The page says "Run a 5E game for your
friends." Three buttons:

- **Run a one-shot tonight**: goes straight to a starter adventure with
  pre-built characters.
- **Start a campaign**: the builder's guided setup.
- **Play solo**: a one-shot with an AI GM.

Below them: **Join a game** (paste an invite link or code).

**Screen 2: Pick a start.** Cards for starting points, each with art, a level
range, a length and a tone:

- **Starter adventures** (ours, original, SRD-only): for example a first
  adventure for levels 1 to 3 in three sessions, a haunted crypt one-shot,
  and a town-defense one-shot that plays like Hold the Stair.
- **Templates**: "Sandbox region", "Dungeon crawl", "City intrigue",
  "Travel and wilderness". Each fills the setting and one empty adventure.
- **Blank campaign.**
- **Describe it**: a text box and a mic button. "A drowned coast of fishing
  towns and an old sea-god cult, levels 1 to 5, spooky not gory." The
  copilot drafts a campaign card, a region, five NPCs and a first adventure
  outline, shown as a board of cards with **Keep** and **Drop** on each.

**Screen 3: The basics.** Campaign name, level range, number of players,
how often you play, in person or remote or mixed. **Next**.

**Screen 4: House rules.** The switches from [House rules](#house-rules),
set to the SRD's defaults, with a **Keep the SRD rules** button that skips
the page. **Next**.

**Screen 5: Safety and tone.** The tone sliders, the GM's own lines and
veils, and which tools are on (X-card on by default). A note that players add
theirs in session zero. **Next**.

**Screen 6: Your first session.** A date and time, and the first scene from
the adventure, already laid out. **Invite players**.

**Screen 7: The tour.** A one-minute guided tour of the GM Screen with a
practice fight: three Skeletons against the GM's own test character. The GM
rolls initiative, takes a turn, sends a handout to a pretend player and ends
the fight. **Skip tour** is always there.

### Inviting players

- **Invite players** gives a link, a QR code and a short code. The GM can
  also send invites by email from the screen.
- A player opening the link sees the campaign card and **Join as a player**.
  They log in (or play as a guest for a one-shot) and choose a name.
- The GM sees who has joined, and can mark someone as a co-GM or remove
  them.
- A watch link for spectators is separate and off by default.

### Building characters

On joining, a player picks one of three ways to get a character:

| Way | What it is | Time |
|---|---|---|
| **Pick a ready hero** | A pre-built character at the campaign's starting level, one per class, like 5E Lab's heroes. Rename it and choose a look | Under a minute |
| **Quick build** | Choose a class and a look; the builder fills sensible scores, gear and spells for that class, and you change anything you want | About 3 minutes |
| **Guided build** | Step by step: class, then (once imported) species and background, ability scores by the campaign's method, skills, gear, spells, a name, a look and a short backstory. Each step explains the choice in a sentence and shows the SRD text on request | 15 to 30 minutes |

- A **Describe your hero** box can start any of them: "a nervous halfling
  thief who's secretly brave" proposes a build the player accepts or edits.
  A non-SRD option is never proposed.
- The GM sees each character as it's made and can **Approve** it or send a
  note. The house rules limit the choices (allowed classes, scoring method,
  starting level).
- A player who rolls physical dice for ability scores types in what they
  rolled.
- The finished sheet shows a preview of the character's figure on the grid
  and in a 3D scene.

### Session zero

Session zero is a guided first session with no fighting, run from the GM
Screen with the players joined.

1. **Welcome:** the campaign card on the Table Display, read aloud by the GM.
2. **Tone check:** each player sets the tone sliders privately; the screen
   shows the group's range and the GM settles it.
3. **Lines and veils:** each player adds theirs privately; the GM sees the
   combined list.
4. **Table tools:** a demo of the X-card, Pause and the open door.
5. **Dice:** each player picks physical or virtual dice, and the group picks
   verifiable dice on or off. A test roll for everyone.
6. **Characters:** players finish or present their characters. The copilot
   can suggest a reason the party knows each other.
7. **Expectations:** how often you play, what happens when someone misses a
   session (their character stays behind, or the GM plays them), and whether
   sessions are transcribed.
8. **Done:** **Save session zero** writes all of it to the campaign's Safety
   page and the log.

### Connecting a table display

- On the GM Screen, **Connect a display** shows a short code and a QR code.
- On the TV, projector or spare laptop, the GM opens the display address in
  a browser and enters the code, or scans the QR from a phone that then casts
  to the TV. The page goes full screen with no controls.
- **Battle mat mode** for a monitor laid flat: the grid at a fixed scale (for
  example one inch per square) so physical miniatures can stand on it, with
  the fog and light still drawn.
- **Mixed tables:** players at the table use the display; remote players see
  the same picture in their browser, and their pings show on the display.
- The GM can connect more than one display, for example a map on the TV and
  the initiative order on a tablet.

## Licensing

- **Rules content is SRD 5.2.1 only**, under CC BY 4.0. The task brief said
  SRD 5.2; the engine and [README.md](README.md) use 5.2.1, the current
  revision, and this product does too.
- **Attribution:** every surface that shows SRD content (character sheets,
  stat blocks, spell text, rules answers, a campaign's public page, its
  `index.md`) carries the SRD's attribution statement word for word, as
  given in [README.md](README.md#legal-and-naming). The copilot's rules
  answers include it.
- **No non-SRD content:** no non-SRD spells, monsters, subclasses, species,
  settings, adventures, names or art. The copilot is instructed not to
  produce them and its outputs are checked against the SRD record names; a
  request for non-SRD content gets "That isn't in the SRD; you can make your
  own version" with a button to start one.
- **Your content is yours and stays separate:** anything the GM or a player
  makes (a world, an NPC, an adjusted creature, a custom item, a house rule)
  is a record marked `srd: false`, shown with a "your own" mark, never mixed
  into the SRD lists, and owned by its maker. Starter adventures and
  templates are ours and original.
- **Uploads:** the GM is responsible for the rights to art and music they
  upload to a private campaign. Anything listed in the Verse marketplace
  needs a stated license.
- **Safety tools:** the X-card and lines and veils are community tools with
  their own attribution practices; check and credit them on the Safety page
  (open question below).

## Open questions for the owner

1. **Name:** what is the product called on openagents.com? "Campaigns" and
   "the Table" are placeholders.
2. **Price:** the transcript floats $10 a month [06:30]. Is that a GM
   subscription (players free), per seat, or credits for AI and compute?
   What's free: one-shots, solo play, a first campaign?
3. **Levels above 6:** do we wait for the engine to reach level 20 before
   launch, or launch with levels 1 to 6 and say so?
4. **Turn-based first:** the README asks whether turn-based mode is needed
   now. The Table needs it; this spec assumes yes.
5. **Persistent characters:** in a persistent world, do levels, XP and gear
   persist per player across sessions and worlds (the README's third open
   question)?
6. **Verifiable dice:** is the commit-and-reveal scheme above enough, or do
   we want each roll anchored somewhere public (for example a Nostr event)?
7. **Voice and transcription:** do we record the table's voice for the
   recap, and where is it kept? Default here: off, opt-in per group.
8. **AI GM scope:** is a full AI GM in scope for the first release, or only
   the copilot?
9. **Marketplace:** can GMs sell adventures, maps and figures they make,
   and how is SRD-derived content in a listing reviewed?
10. **Safety tool credits:** how do we credit the X-card and lines and veils?
11. **Minors:** is the product for ages 13 and up, and what does that change
    in the copilot and in sharing?

## Build order

Each phase ships something a group can use, and leans on what exists.

| Phase | What ships | Built from |
|---|---|---|
| **1. A one-shot at the Table** | The Table with the GM Screen, My Character and the Table Display (on one device, a second browser or a TV). Pre-built heroes at levels 1 to 6. Virtual and physical dice, open and hidden. The initiative tracker in theatre of the mind and on the grid. One starter one-shot | The engine's turn-based clock (README step 4), 5E Lab's grid view, CHARACTERS panel, condition marks and hotbar, rooms for a shared session (ch049–ch050) |
| **2. The Campaign Builder** | Campaigns, setting, regions, maps with fog, scenes, encounters with **Run it**, NPCs, loot, handouts, quests, clocks, house rules as switches, the Safety page | The ruleset profile (README step 5), the realm document model (ch051), saves (ch081) |
| **3. Onboarding** | The first-run screens, invites, quick and guided character builds (classes first), session zero, connecting a display, the tour | Phases 1 and 2. Guided build grows as the SRD import adds species, backgrounds and feats (README step 2) |
| **4. The copilot** | Prep drafts, rules lookups, improvised NPCs, recaps, clock suggestions, all as Accept and Decline cards | The semantic selector, embedding search over SRD records, `/v1/decisions` with Clef first, the characters endpoint (ch077), voice (ch069) |
| **5. 3D scenes and sound** | Scenes in 3D from 5E Lab's glade, Everglade and Aethyrforge, dice in 3D, music and ambience per scene, spell looks | 5E Lab's renderer, Aethyrforge, sound (ch065–ch068), effects (ch072) |
| **6. Verifiable dice and spectators** | The session seed with each player's contribution, **Check the dice**, watch links | The engine's deterministic dice, `DiceReveal` in the Verse protocol |
| **7. Solo play and the AI GM** | An AI GM through a seat, solo and one-shot modes | Seats (realm-seat.md), phase 4, monster decisions through `/v1/decisions` |
| **8. Persistent worlds** | A West-Marches-style shared world on Everglade or a fork of it, expeditions players sign up for, characters that persist, action combat for live fights | Multiplayer realms (ch049–ch050), forks (ch055), the action-combat clock, persistent saves |
| **9. Levels 7 to 20 and the full SRD** | Every class to level 20, every monster, item and spell | README steps 2 and 3, the golden rules tests |

Phase 1 is the smallest thing that answers the cashier's question: a GM runs a
real session with friends, the rolls are real, and the fight is on a map
everyone can see.

This work includes material from the System Reference Document 5.2.1 ("SRD
5.2.1") by Wizards of the Coast LLC, available at
https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative
Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.
