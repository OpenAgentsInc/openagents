# Agentics: a profession for commanding agents (spec)

Status: specification, 2026-11-09. Nothing in this document is built yet,
except the pieces marked **exists today**. Owner's ask: "define a profession
called Agentics. The workbench lets characters start to control agents …
command agents from an agent workbench inside the world to have actions taken
inside the game world or outside of the game world."

This spec defines:

- the profession, as fiction and as a game system that follows the SRD 5.2.1
  where the SRD has a rule;
- the agent workbench in the player's house, and how you command agents from
  it;
- what agents may do inside Everglade, and what they may never do;
- the bridge that lets a command at the bench start real OpenAgents work, with
  its consent, permission, audit and payment rules;
- a three-phase plan, with the files and tests each phase needs;
- the questions the owner needs to answer.

Agentics is Everglade's own invention. It isn't SRD content, and every record
it adds is marked `srd: false`, as the house spells in
[`aethyr/rules/srd/house.coax`](../../aethyr/rules/srd/house.coax) are. It is
built from SRD rules wherever they apply: tool proficiency, ability checks,
crafting time and cost, Help, Find Familiar, Unseen Servant and Planar
Binding.

## Contents

1. [Names](#1-names)
2. [What the SRD already says](#2-what-the-srd-already-says)
3. [What other MMOs did](#3-what-other-mmos-did)
4. [The profession](#4-the-profession)
5. [The workbench](#5-the-workbench)
6. [Agents](#6-agents)
7. [Commanding agents](#7-commanding-agents)
8. [Actions inside the world](#8-actions-inside-the-world)
9. [Actions outside the world: the bridge](#9-actions-outside-the-world-the-bridge)
10. [Safety](#10-safety)
11. [Data model](#11-data-model)
12. [Phases](#12-phases)
13. [Open questions for the owner](#13-open-questions-for-the-owner)
14. [Sources](#14-sources)

## 1. Names

**Agentics** stays the system's internal name: the code prefix (`ag_`), the
protocol records (`agentics.*`), the docs and the issue labels. Players see a
fantasy name.

### Candidate profession names

| Name | Practitioner | What it suggests | Weakness |
| --- | --- | --- | --- |
| **Bindcraft** | binder | Binding a spirit to a vessel and a task, in the words of the SRD's Planar Binding and Find Familiar | "Binding" can read as coercion; the fiction has to make it a pact |
| Golemancy | golemancer | Building and commanding constructs | Only constructs; spirits and familiars don't fit; golems suggest combat |
| Familiar-binding | familiar-binder | The SRD's Find Familiar, scaled up | Long; sounds like one spell, not a trade |
| Wardcraft | warden | Guarding and watching | Says nothing about errands, crafting or messages; "Warden" is already a Druid order in the SRD |
| Sigilry | sigilwright | Writing orders as sigils that spirits obey | Abstract; players won't guess that it means agents |
| Stewardry | steward | Running a household of servants | Mundane; no magic in the word |
| Errantry | errant-master | Sending agents on errands | Archaic; easy to misread as "errors" |
| Spiritwright | spiritwright | Crafting spirits into workers | Close to Bindcraft, but harder to say and to verb |

**Recommendation: Bindcraft.** It is one short word, it works as a verb
("bind an agent", "your bound"), and the practitioner's name, *binder*, is
plain. It ties straight to SRD text that players of 5E already know: Planar
Binding binds a creature "to serve you", to "guard a location, or to deliver a
message", and it "travels to you to report" when it is done. That is exactly
what an agent does. It also covers all three kinds of agent in section 6:
spirits, constructs and the far envoys that act outside the world.

The coercion reading is handled in the fiction: a binder makes a **pact**. The
spirit agrees to the work and is paid for it (in the incense and spirit-salts
of section 4.5), and a pact can be ended by either side.

### Candidate bench names

| Name | Notes |
| --- | --- |
| **Binding Table** | Recommended. Matches Bindcraft; "use the Binding Table" reads naturally |
| Binder's Bench | Good second choice if "table" reads like furniture |
| Artificer's Bench | Implies item crafting more than agents |
| Sigil Desk | Fits Sigilry, not Bindcraft |
| Summoning Circle | Implies combat summons, which agents aren't |

In this spec, "the bench" means the Binding Table, and "agent" means a bound
agent.

## 2. What the SRD already says

The SRD 5.2.1 has no profession system, no Downtime chapter and no Bastions
(those are in other 5E books, not the SRD). It does have the pieces a
profession needs. Page numbers are the SRD 5.2.1 PDF's.

| Rule | SRD 5.2.1 | What it gives Agentics |
| --- | --- | --- |
| Tools: Ability, Utilize, Craft, Variants | Equipment, "Tools", p. 93 | The shape of a new tool kit's record |
| Tool proficiency | Equipment, "Tool Proficiency", p. 93 | Add your Proficiency Bonus to a check with the tool; with proficiency in a skill used in the same check, you also have Advantage |
| Artisan's Tools | Equipment, pp. 93-94 | Each tool needs its own proficiency; Tinker's Tools (50 GP, Dexterity) is the closest existing kit |
| Utilize action | Playing the Game, "Actions", p. 10 | "Use a nonmagical object": using the bench is a Utilize action |
| Skilled origin feat | Feats, p. 87 | "Proficiency in any combination of three skills or tools of your choice": an SRD route to the new tool |
| Hirelings | Equipment, "Services", p. 102 | Skilled hireling 2 GP a day, untrained 2 SP a day, messenger 2 CP a mile: the price floor for agent work |
| Crafting Nonmagical Items | Equipment, p. 103 | Raw materials cost half the item's price; days of 8 hours = price in GP ÷ 10, rounded up; days needn't be consecutive; one assistant with the same proficiency divides the time |
| Brewing Potions of Healing; Scribing Spell Scrolls | Equipment, p. 103 | Fixed-time, fixed-cost crafting jobs; a scroll needs the spell prepared each day |
| Crafting Magic Items | Magic Items, pp. 206-207 | Arcana proficiency plus the right tool (Tinker's Tools for a Wondrous Item); time and cost by rarity: Common 5 days and 50 GP, Uncommon 10 days and 200 GP, Rare 50 days and 2,000 GP; halved for consumables other than scrolls |
| Find Familiar | Spells, p. 130 | A spirit in animal form that "acts independently of you, but … obeys your commands"; it can't attack; telepathy within 100 feet; one at a time |
| Unseen Servant | Spells, p. 171 | A mindless force that performs "simple tasks that a human could do, such as fetching things, cleaning, mending"; it can't attack; it ends if sent more than 60 feet away |
| Planar Binding | Spells, pp. 152-153 | A bound creature follows your commands: accompany you, guard a location, deliver a message, then report back; 24 hours, up to 366 days with higher slots |
| Animate Objects | Spells, pp. 108-109 | Combat constructs under Concentration for 1 minute: the model of what agents must *not* become |
| Homunculus | Monsters, p. 299 | A Tiny Construct with a Telepathic Bond to its master on the same plane: the model for the first agent's body |
| Help action | Playing the Game, "Actions", p. 10 | The only combat assistance an agent gives (section 8.3) |
| Travel Pace | Playing the Game, p. 12; Gameplay Toolbox, p. 192 | Normal pace is 3 miles an hour, 24 miles a day: how long an errand between towns takes |

Two SRD lessons shape the design:

- **Servants in the SRD can't attack.** Find Familiar and Unseen Servant both
  say so outright. An agent follows the same rule.
- **Crafting runs on 8-hour workdays that needn't be consecutive.** That is a
  timed queue, already. Agentics uses it as written and only maps the workday
  onto real time (section 4.4).

**Exists today.** Tool proficiency and checks are in
[`aethyr/rules/srd/check-training.coax`](../../aethyr/rules/srd/check-training.coax)
(37 SRD tools as bits, `rs_tool_bit`) and
[`ability-checks.coax`](../../aethyr/rules/srd/ability-checks.coax), with the
tool-plus-skill Advantage rule. Help is in
[`help.coax`](../../aethyr/rules/srd/help.coax). Find Familiar runs as a shared
actor in [`familiar.coax`](../../aethyr/rules/srd/familiar.coax) and its
senses and touch files. Equipment crafting is **missing**: the
[compliance ledger](../../docs/dnd/srd-compliance-ledger.md) lists
CONTENT-EQUIPMENT as partial, with crafting not modeled.

## 3. What other MMOs did

### Neverwinter

Neverwinter is the closest model, because it is also built on 5E-style
rules and it let players run professions from outside the game.

- **Leadership** was a profession of hired mercenaries that you assigned to
  timed tasks of 2 to 8 hours. Tasks needed a number of people and gave
  experience, gold, items and refinement stones. The number of tasks of one
  rank at a time was capped (players reported three rank-1 tasks).
- **The Gateway** was a web app that let you assign profession tasks without
  logging in to the game. Bots used it to run Leadership around the clock and
  mass-produce Astral Diamonds, the premium-adjacent currency. Cryptic took
  Leadership off the Gateway, then removed most Astral Diamond rewards from
  it; it came back to the Gateway in September 2015 with those rewards gone.
  Leadership later became a "Forgotten Profession".
- **The 2018 overhaul** (Heart of Fire) replaced faceless profession assets
  with a personal **workshop** and over 200 named **artisans** you hire. You
  place orders at a worktable, and artisans complete them over time, even
  while you are away. A few instant crafts a day were free and more cost
  Astral Diamonds. The workshop has a Master's Desk (upgrades and artisan
  applications), a Retainer and a dispatch board.
- **Masterwork** professions were gated behind a profession rank of 25 and the
  guild's Stronghold rank, with quests from an Artisan NPC in the Stronghold.

**Lessons for Agentics:**

1. Named agents in a room you own are better than icons in a menu. The bench
   lives in your house, and your agents stand in it.
2. Any out-of-game control surface gets botted. If it pays a tradable
   currency, it inflates the economy. Agentics never pays a tradable currency
   from an unattended loop (section 10.4).
3. Paying to skip the timer turns a profession into a store. Agentics has no
   paid skip (open question 13.4).

### Other games

- **World of Warcraft garrisons** (Warlords of Draenor): you send followers on
  missions from a mission table. A mission lists enemy threats, each follower
  counters some, and the table shows the success chance before you commit.
  Lesson: show the odds before the player commits, computed by the same rule
  that resolves the task.
- **Star Wars: The Old Republic crew skills**: companions run gathering,
  crafting and mission skills on real-time timers while you play. Missions can
  fail and lose their cost. The number of simultaneous missions grows with
  your level. Lesson: slots grow with progression; failure has a cost, but a
  small one.
- **Final Fantasy XIV retainers**: hired NPCs you send on ventures that bring
  back items; you profit by selling what they bring, not from the venture.
  Lesson: agents produce goods, not money.
- **EVE Online industry**: manufacturing jobs run on blueprints whose time
  depends on skills and research, and concurrent jobs grow with a skill (older
  guides; a 2014 change removed facility slots). Lesson: the queue and its
  timers are the game; good tooling around them makes it deep.
- **RuneScape Player-Owned Ports**: you send ships with a captain and crew on
  voyages; each voyage has several stat requirements, and its success chance
  is the *least-met* requirement. You hold at most 5 captains and 25 crew.
  Lesson: a bottleneck rule ("your weakest stat decides") makes team choice
  interesting with simple math.

## 4. The profession

### 4.1 In the world

A **binder** is someone who makes pacts with small spirits and houses them in
vessels: a carved homunculus, a glass lantern, a folded paper bird. The spirit
works for the binder in exchange for incense and spirit-salt, and reports what
it did. Binders are clerks, couriers and caretakers more than wizards, and in
Everglade's towns you meet them at a counting-house or a post.

The first binder you meet teaches you the trade and sells you a kit. Your
house's Binding Table is where you make pacts, write orders and read reports.

### 4.2 The tool: Binder's Tools

A new tool kit, in the SRD's tool format (Equipment, p. 93), marked as
Everglade's own:

> **Binder's Tools (50 GP)** *(Everglade, not SRD)*
> **Ability:** Intelligence
> **Weight:** 6 lb.
> **Utilize:** Read the terms of a pact on a vessel or familiar (DC 10), or
> calm a spirit that is refusing an order (DC 15)
> **Craft:** Vessels (section 6.2), Order Scrolls, Spirit-salt

- It costs what Tinker’s Tools costs, the nearest SRD kit. It uses
  Intelligence, as Alchemist’s Supplies and Jeweler’s Tools do, because
  binding is study and bargaining, not handwork.
- **Proficiency** adds your Proficiency Bonus to every Bindcraft check. If you
  are also proficient in **Arcana**, you have Advantage on the check, as the
  SRD's tool proficiency rule says for a skill used in the same check.
- In code it is a house tool bit next to the SRD's 37 in
  `check-training.coax`. It takes a reserved house range (for example bit 48
  and up) so the SRD tools stay contiguous and `RS_TOOLS` still counts SRD
  tools only.

### 4.3 Becoming a binder

You gain proficiency with Binder's Tools in one of three ways:

- **Skilled** (origin feat, SRD p. 87), choosing Binder's Tools as one of its
  three picks. This is the SRD route.
- **A class or background grant.** None exists in the SRD; Everglade may add
  Binder's Tools to its own background list later, marked as its own.
- **Training** from a binder NPC: a quest, a fee and a number of workdays.
  The SRD 5.2.1 has no training rule, so this is a declared house rule
  (`RS_DEV_TRAINING`, added to the world profile of
  [chapter 18](../book/ch018-turns-become-time.md)).

Without proficiency, you can still own a bench and run one agent on the
simplest tasks: you make the check without your Proficiency Bonus, as the SRD
allows for any tool.

### 4.4 Time: the workday in real time

The SRD measures crafting in 8-hour days that needn't be consecutive. Everglade
keeps the day as the unit and maps it to real time with one world constant:

```text
AG_WORKDAY_S = 1200        one SRD workday (8 hours of work) = 20 real minutes
```

- A task's length is always stated in SRD days, then converted. A Heavy
  Crossbow (50 GP) is 5 days to craft in the SRD, so it is 100 real minutes
  at the bench.
- **Agents work while you are offline**, as Neverwinter's artisans do. The
  server keeps the clock; nothing runs on the client.
- The mapping is a declared deviation, `RS_DEV_WORKDAY`, listed in the world
  profile next to the timing deviations. A strict world sets
  `AG_WORKDAY_S` to 28,800 (8 real hours).

Whether 20 minutes is right is open question 13.2.

### 4.5 Costs

Every cost is in-game currency and materials, from SRD prices where they
exist:

| Cost | Rule |
| --- | --- |
| A task's wage | The SRD's hireling pay: 2 SP a workday for an untrained task, 2 GP for a skilled one, 2 CP a mile for a message |
| Materials for a crafted item | Half its price (SRD p. 103) |
| A vessel | Crafted as a nonmagical item from its price (section 6.2), or a Common magic item (5 days, 50 GP) for a familiar-grade vessel |
| Incense to bind a spirit | 10 GP, the burning incense that Find Familiar consumes |
| Spirit-salt | The agent's upkeep: one per workday of work, craftable with Binder's Tools |

Money for out-of-world actions is separate and is never in-game currency
(section 9.6).

### 4.6 Ranks and progression

Bindcraft has five ranks. Rank is earned by completed tasks, not by spending.

| Rank | Title | Agents bound at once | Tasks queued per agent | Unlocks |
| --- | --- | --- | --- | --- |
| 1 | Apprentice | 1 | 3 | Household and gathering tasks in your house and town |
| 2 | Journeyman | 2 | 3 | Errands between towns; scouting |
| 3 | Adept | 3 | 4 | Crafting assistance; guarding the house; live commands (phase 2) |
| 4 | Master | 4 | 5 | Familiar-grade vessels; two agents on one task |
| 5 | Grand Binder | 5 | 6 | Envoys: out-of-world actions (phase 3) |

- Rank thresholds are task counts weighted by task days (open question
  13.5 for the numbers).
- The rank is a profession rank, not a character level. It never changes
  combat numbers.
- The 5-agent ceiling copies RuneScape's captain cap and SWTOR's mission cap:
  small enough that each agent has a name.
- Every limit in this table is shown on the bench, with the next rank's
  value, before you hit it.

## 5. The workbench

### 5.1 Where it is

The bench is the Workbench prop in the player's first house in Everglade
Experimental. The house work gives it a stable id and an interaction spot. The
current props are `Workbench` and `Workbench_Drawers` in
[`verse/experimental-props.coax`](../../verse/experimental-props.coax). Agentics
reads the bench id; it never places or moves the prop.

### 5.2 Using it

1. You walk to the interaction spot, and the prompt says **Use the Binding
   Table**.
2. You press the interact key. That is an SRD **Utilize** action; it can't
   happen in combat or while you are Incapacitated.
3. The bench view opens over the world. You stay standing at the bench, and
   other players see you working at it.
4. Moving away or taking damage closes the view. Orders you submitted stay
   submitted.

### 5.3 What the bench shows

| Panel | Contents |
| --- | --- |
| **Agents** | Each bound agent by name, its vessel, what it is doing, its time left, and its spirit-salt |
| **Orders** | Each agent's queue, in order, with each task's days, cost, DC and success chance |
| **Reports** | Finished tasks: what happened, the roll, what came back. Reports are also letters in your mailbox |
| **Pacts** | Bind a new agent, rebind one to a new vessel, end a pact |
| **Envoys** (rank 5, phase 3) | Linked OpenAgents account, allowed actions, spending limits, and pending confirmations |

The success chance is computed by the same server function that resolves the
task, so the number shown is the number rolled against (the garrison lesson).

## 6. Agents

### 6.1 What an agent is

In the fiction, an agent is a spirit under a pact, housed in a vessel. In the
rules, it is a creature with its own small stat block, owned by one
character:

- It **can't attack**, as Find Familiar's and Unseen Servant's servants can't.
- It acts on its own task clock, not on your turns, except during live
  commands (section 7.3).
- It drops to 0 Hit Points like any creature. Its spirit returns to the bench,
  and you rebind it into a new or repaired vessel at the cost of the vessel,
  without losing its rank or name.
- It is **one per character per pact**: you can't bind another player's agent,
  and agents can't be traded (open question 13.6).

### 6.2 Kinds of agent

| Kind | Fiction | Body (from the SRD) | Rank | What it does |
| --- | --- | --- | --- | --- |
| **Hearth spirit** | A small spirit in a clay or wood figure | Unseen Servant's numbers: AC 10, 1 HP, Strength 2, can't attack | 1 | Household tasks at the house: tidy, mend, sort, stock |
| **Homunculus** | A carved construct with a Telepathic Bond | The SRD Homunculus (Monsters, p. 299), with its Bite removed | 1-2 | Gathering, errands, carrying, shopping |
| **Familiar-grade** | A spirit in a CR 0 beast form | A Find Familiar form's stat block (Owl, Hawk, Cat…) | 4 | Scouting, guarding, longer errands |
| **Envoy** | A spirit that speaks through the bench's Veilstone to the world beyond | No body in the world | 5 | Out-of-world actions (section 9) |

The Veilstone is the bench's speaking stone, a sending-stone in the fiction
(the SRD's Sending spell): the way an envoy reaches the world outside the game.

### 6.3 Agent record

Each agent has:

- a name the player chooses (filtered like character names);
- a kind and a vessel;
- **Bindcraft checks** made with the binder's numbers: Intelligence modifier,
  plus Proficiency Bonus if proficient with Binder's Tools, with Advantage if
  also proficient in Arcana;
- an **aptitude** in one task family (gathering, crafting, scouting, errands,
  guarding), earned by doing it: +1 per 10 tasks of that family, up to +3.
  This is Everglade's own and is shown as such.

## 7. Commanding agents

There are three ways to command an agent. All three go through the room
server; a client only sends an intent.

### 7.1 Orders (phase 1)

An order is a task from the bench's list, with its parameters:

```text
order  := agent, task kind, target (a place, an item, a recipient), repeat count
task   := days (SRD), wage, materials, DC, ability, aptitude family
```

1. You pick an agent and a task. The bench shows its days, real time, cost,
   DC and success chance.
2. You confirm. The server takes the wage and materials at once, as SWTOR
   does, and queues the task.
3. When the agent's queue reaches it, the timer starts. When it ends, the
   server rolls the check: a d20, plus the binder's modifiers and the agent's
   aptitude, against the DC (`rs_check_plan` with the Binder's Tools bit).
4. On success, the result goes to your house chest or mailbox. On failure,
   you get half the materials back and no product; the report says what the
   roll was.
5. Queued tasks run one after another; an agent works on one task at a time.

**Helping.** The SRD lets one assistant with the same proficiency divide the
crafting time. Two of your agents on one task (rank 4) halve its days. A
player who is at your bench and proficient can take the Help action on your
current task, which gives your check Advantage, as `help.coax` already does
for checks.

### 7.2 Standing orders (phase 1)

A standing order repeats a task until you stop it, until materials or wages
run out, or until 7 SRD days pass, whichever comes first. After 7 days, it
stops and asks you to renew it at the bench. This keeps an unattended loop
from running forever.

### 7.3 Live commands (phase 2)

At rank 3, you can take one agent out with you and command it in real time,
like a familiar:

- The agent follows you within 100 feet, Find Familiar's telepathy range.
- As a **Bonus Action**, you command it to do one thing: move up to its Speed
  and interact with an object, pick up or carry an item, hold a spot, or take
  the **Help** action for you. This is Unseen Servant's command, with Find
  Familiar's range.
- A live agent is out of its task queue while you have it out. Its queued
  tasks wait.
- Live commands use the same intents as the familiar's controls in Everglade
  Experimental (`docs/protocol/everglade-familiar-v1.md` and its step and
  senses records), so the agent reuses the familiar's shared actor path.

### 7.4 Spoken and typed orders (phase 2)

At the bench you can type or say an order ("send Pip to Riverford with the
letter for the smith"). The text goes through the **semantic selector** to a
typed order, never through keyword matching (workspace rule on semantic
routing). The bench then shows the typed order, with its cost and chance, and
you confirm it. The text never acts on its own.

## 8. Actions inside the world

### 8.1 What agents can do

| Family | Examples | Rule |
| --- | --- | --- |
| Household | Tidy, mend, sort the chest, restock the larder | Unseen Servant's simple tasks; no check below DC 10 |
| Gathering | Herbs, wood, stone, scrap near the house or along a known road | A Wisdom (Survival) or Intelligence (Nature) DC by the area; days by the amount |
| Crafting assistance | An agent works as the SRD's assistant on your crafting job | Halves the time; it needs the craft's tool proficiency, which a vessel can carry (an "apprentice" vessel) |
| Scouting | Map an area you have visited; report creatures seen near a road | Familiar-grade only; uses Cartographer's Tools' "draft a map" DC 15; reports what is there now, not loot |
| Errands | Carry an item or letter to another town or player's mailbox; buy from a shop | Time from Travel Pace (24 miles a day); wage 2 CP a mile; the item is in transit and can't be used |
| Guarding | Watch the house while you are away and report anyone who entered | A Passive Perception for the agent; it raises an alarm and reports; it never fights |

### 8.2 What agents never do

These rules are for fairness in a shared world:

- **No combat.** Agents never attack, cast attack spells, deal damage or
  make Opportunity Attacks. They are SRD servants, and Animate Objects is
  never their model.
- **No botting your own character.** Nothing in Agentics moves, aims, casts
  for or makes choices for your character. A command never queues your own
  actions.
- **No loot from monsters or other players.** Agents don't kill, loot or
  pickpocket. Scouting reports; gathering takes only from gathering nodes.
- **No trades and no market.** Agents don't post to an auction or trade with
  players. Errands deliver items you chose to send.
- **No dungeons, raids or contested areas.** Agents work in towns, roads and
  your house.
- **One house, one bench.** Tasks start only at your own bench.

### 8.3 Assisting in combat

The only combat effect an agent has is the SRD's **Help** action during a
live command (section 7.3): it gives you Advantage on your next check, or on
your next attack against a creature within 5 feet of the agent, as the SRD's
Help says. It takes the agent's action, and an agent can Help only on your
command. Delivering touch spells stays the familiar's (Find Familiar), not the
agent's.

### 8.4 Limits

- Every task is resolved by the room server. The client never rolls and never
  reports a result.
- Per character: at most the rank's agents and queue lengths (section 4.6);
  at most 60 orders an hour; at most one standing order per agent.
- Gathering nodes are shared and refill on the world's clock. Agent gathering
  takes from the same nodes as players, at half a player's rate, so agents
  never outgather the people in the world.
- Every limit is shown on the bench before you reach it.

## 9. Actions outside the world: the bridge

At rank 5, an **envoy** can act outside Everglade, through the player's own
OpenAgents account. This is the part that turns the bench into a real agent
console.

### 9.1 What exists today to command

| Action | What it does | Built from | Status |
| --- | --- | --- | --- |
| **Ask a project's agent** | Send a message to your project's agent; it edits milestones, notes and the linked repo; one turn = one commit, with UNDO TURN | [Project agent spec](../../docs/project-agent.md) | Spec (draft, 2026-10-28) |
| **Read a project's queue** | List open tasks on a project such as `/projects/coax` | [Projects](../../docs/protocol/2026-10-28-projects.md): signed NIP-34 issues and statuses | Read path exists; claim and submit are proposed |
| **Start a Coder task** | Give Coder, our coding agent, an issue or a change to make, and get its result | [Coder book](../../coder/book/README.md) | Harness in progress; no hosted entry point yet |
| **Check your Pylons** | See whether your Pylons are online and what they earned | Pylons on the homepage; [guard decisions](../../docs/verse/pylon-guard-decisions.md) | Online list exists; per-owner view is proposed |
| **Make or edit a realm** | Describe a realm and Aethyrforge builds it | [Realm builder](../../docs/verse/realm-builder.md), [realm edit](../../docs/verse/realm-edit.md) | Exists |
| **Tip a maker** | Pay a realm's maker in Lightning | [Makers](../../docs/verse/makers.md) | Exists (built 2026-10-30) |
| **Post** | Publish a signed note as your Nostr identity | NIP-98 and Nostr signing | Proposed |

Phase 3 starts with the read-only rows (project queue, Pylon status) and one
write row (ask a project's agent), then adds the rest one at a time.

### 9.2 Architecture

```text
  bench (client)                room server (oa-room)             OpenAgents API                 agent
  ─────────────                 ─────────────────────             ──────────────                 ─────
  envoy order intent  ──────▶   checks seat = character owner
                                checks character ↔ account link
                                checks rank, action allowed
                                writes pending order  ─────────▶  POST agentics order (pending)
                                                                  [account, action, args, cost]
                                                                         │
  confirm on openagents.com or the phone ◀──────────────────────────────┘
  (session cookie or NIP-98, NOT the game socket)
                                                                  confirmed ───────────────────▶ runs (project agent,
                                                                                                   Coder, Pylon, …)
                                                                  result, receipt  ◀───────────── done
  letter and bench report ◀──── room delivers the report ◀──────  result event
```

- **The room server is the authority in the world**
  ([multiplayer spec §3.1](../../docs/verse/2026-11-03-everglade-multiplayer-spec.md)).
  It turns an envoy intent into a *pending* order. It holds no user secrets
  and can't approve an order.
- **The OpenAgents API is the authority outside it.** It knows the account,
  its linked identities ([realm invites and auth](../../docs/auth/2026-10-31-realm-invites-and-auth.md)),
  its agent keys ("a linked agent key acts as its account", NIP-98 per
  request), its permissions and its spending limits.
- **Confirmation happens outside the game socket**, on a surface the account
  already trusts: the openagents.com page with the `oa_session` cookie, or the
  iOS app signing with its Keychain key. A stolen or hijacked game seat can
  queue a request but can't approve it.
- **The agent that does the work** runs as the account's agent identity, with
  the same server-side checks it has when you run it from the web. The bench
  adds no new power; it is one more way to start what you can already start.
- **Results come back as events.** The API emits a result record; the room
  delivers it as a **letter** in your mailbox and a **report** at the bench:
  what was done, a link to it (the commit, the realm, the receipt), and its
  cost.

### 9.3 Linking

1. At the bench's Envoys panel, you choose **Link your OpenAgents account**.
2. The bench shows a one-time code and a link to `openagents.com/login`.
3. You log in there (or approve with the phone), and confirm "Let
   *character name* in Everglade send requests to your account."
4. The API records the link: account, character id, the allowed actions
   (none by default), and the spending limit (zero by default).
5. You can unlink at the bench or on the account page. Unlinking cancels
   every pending order.

A guest character can't link. A character links to one account; an account
can link several of its own characters.

### 9.4 Consent and confirmation

| Kind of action | Example | Confirmation |
| --- | --- | --- |
| Read | List a project's tasks, check Pylons | None after linking, if the action is allowed |
| Private write | Ask your own project's agent to edit a milestone | Allowed per action; confirm each one, or allow "without asking" for that one action |
| Outward-facing | Post a note, comment on an issue, open a pull request | Always confirmed, every time, outside the game |
| Costs money | Tip, pay for compute, pay an agent | Always confirmed, every time, outside the game, within the limit you set |

"Without asking" is never available for outward-facing or paid actions.

### 9.5 Permissions

- The default is nothing. You turn on each action at the account page or the
  Envoys panel.
- An action runs with the account's own permissions. A project's agent can
  change only its own project, as its spec says; the bench can't widen that.
- An envoy can act only on resources the linked account owns or is a member
  of.

### 9.6 Money

- **Bitcoin only**: Lightning, on-chain, Spark and Taproot Assets, as the
  [money rule](../../docs/lightning/README.md) says. Prices are in sats.
- **In-game gold never buys real actions, and sats never buy gold.** Real and
  game money don't convert in either direction. This keeps the game economy
  out of the Gateway problem (section 3).
- **Limits you set**: a per-action limit, a daily limit and a monthly limit,
  each zero until you raise it. Every limit and what is left of it shows on
  the Envoys panel. An order over a limit is refused with the limit named.
- Each payment has a receipt in the audit log and in the report letter.

### 9.7 Audit log

Every envoy order is recorded, from intent to result:

```text
agentics_audit: order id, account, character, room, bench id, action, args hash,
                cost (sats), state (pending → confirmed | refused | expired → running → done | failed),
                who confirmed and where (web, phone), timestamps, result link, receipt
```

You see your log on the account page and at the bench. Pending orders expire
after 15 minutes without confirmation.

## 10. Safety

1. **No one acts on someone else's account.** An envoy order is valid only
   when the seat's character is linked to the account, and the account's own
   surface confirms. Other players at your bench can see it but can't send or
   confirm envoy orders, and a party member can't use your envoy.
2. **Explicit linking**, off by default, with every action off and every
   limit at zero (section 9.3).
3. **Confirmation outside the game** for anything outward-facing or paid
   (section 9.4).
4. **No unattended money loop.** Standing orders can't include envoy actions
   that post or pay. Envoys never earn in-game currency, and in-world tasks
   never earn sats.
5. **Rate limits**: at most 20 envoy orders an hour and 100 a day per account,
   in addition to the in-world limits; repeated refusals lock the Envoys panel
   for an hour. The numbers are shown on the panel.
6. **Text is data.** A typed or spoken order and anything an agent reads
   (an issue, a page, a chat line) never become instructions to the bridge
   without the typed order and its confirmation.
7. **Server authority everywhere.** The client sends intents only; the room
   rolls and keeps time; the API checks identity, permissions and limits.
8. **Abuse and moderation.** Agent names are filtered like character names.
   Errand letters to other players use the chat's existing moderation and
   block lists. A blocked player can't receive your agents' errands.
9. **Our own sessions don't count.** Owner, team and agent test sessions are
   excluded from usage numbers, as everywhere else.

## 11. Data model

Records are Everglade's own and are versioned protocol records, as the
familiar's are (`docs/protocol/everglade-familiar-*.md`).

```text
agentics.profession.v1   character id, rank, task count by family, Binder's Tools proficiency
agentics.agent.v1        agent id, owner character, name, kind, vessel, HP, aptitudes, spirit-salt, state
agentics.order.v1        order id, agent id, task kind, target, repeat, days, wage, materials, DC,
                         state (queued → running → succeeded | failed | cancelled), start, end, roll
agentics.report.v1       order id, outcome, roll, items, text for the letter
agentics.bench.v1        bench id (from the house), owner character, agents housed
agentics.link.v1         character id, account id, allowed actions, limits (sats), created, revoked
agentics.envoy-order.v1  the audit row of section 9.7
```

- In-world records live with the character's persistent state on the room
  server (the multiplayer spec's persistence, section 7 of that spec).
- `agentics.link.v1` and `agentics.envoy-order.v1` live in the OpenAgents
  API's database, not in the room.
- Times are server times. A task's end is `start + days × AG_WORKDAY_S`.

## 12. Phases

Each phase ships whole, with its checks, before the next starts.

### Phase 1: the bench, the profession, in-world timed tasks

What it adds:

- Binder's Tools as a house tool bit, the training house rule and the
  workday deviation in the world profile.
- Agents (hearth spirit and homunculus), pacts, orders, standing orders,
  reports and letters.
- The bench view on the house's Workbench, using the house work's bench id.
- Household, gathering and errand tasks; ranks 1 and 2.

Files and systems:

- `aethyr/rules/srd/check-training.coax`: the house tool bit.
- `aethyr/rules/srd/world-profile.coax`: `RS_DEV_WORKDAY`, `RS_DEV_TRAINING`.
- `aethyr/rules/srd/agentics.coax` (new): task table, cost, time and success
  math; `#authority`.
- `aethyr/rules/srd/agentics-checks.coax` (new): its checks.
- `aethyr/rules/srd/crafting.coax` (new): the SRD's nonmagical crafting rule,
  which Agentics and the ledger's CONTENT-EQUIPMENT row both need.
- The room's Everglade simulation and persistence: orders and the clock.
- The Experimental client: the bench view, the house prop's interact prompt.
- `docs/protocol/agentics-*-v1.md`: the records.
- `docs/dnd/srd-compliance-ledger.md`: the crafting row; the house tool marked
  as Everglade's own.

Tests:

- Crafting math equals the SRD's examples: Plate Armor needs 750 GP of
  materials; a Heavy Crossbow takes 5 days; one assistant halves the days.
- A check with Binder's Tools adds the Proficiency Bonus only when proficient,
  and has Advantage only with Arcana too (`#forall` over training masks).
- The success chance the bench shows equals the resolver's probability, for
  every DC and modifier.
- A task's end time is exact across a server restart; an offline player's
  tasks finish.
- Queue and agent limits refuse the order past the rank's value, and the
  refusal names the limit.
- Wages and materials are taken once; a failure refunds half the materials,
  once.
- Mutation run (`coax mutate`) on `agentics.coax` kills every mutant.
- A world fixture: walk to the bench, open it, queue a task, leave, return
  after the time, read the report and the letter (headless).

### Phase 2: live commands and typed orders

What it adds:

- Live commands: one agent out with you, Bonus Action commands, Help.
- Familiar-grade agents, scouting and guarding; ranks 3 and 4.
- Typed and spoken orders through the semantic selector, with confirmation.

Files and systems:

- `aethyr/rules/srd/familiar.coax` and its senses and touch files: the
  shared actor path, generalized from "one familiar" to "one familiar and
  one live agent".
- `aethyr/rules/srd/help.coax`: an agent as the helper.
- The room: live-agent intents, interest filtering for agents.
- The bench: the order box and its typed-order preview.

Tests:

- An agent can't attack: every attack intent from an agent is refused, in
  battle and out (a negative fixture for each attack path).
- No command reaches the owner's character: no Agentics intent produces a
  move, cast or attack for the player's own creature.
- Help from an agent gives Advantage once and spends the agent's action.
- A live agent past 100 feet stops and waits; it never teleports.
- A typed order resolves to the same typed order the menus build, and
  nothing runs without confirmation.

### Phase 3: the out-of-world bridge

What it adds:

- Envoys at rank 5, linking, permissions, limits, confirmation, the audit log
  and result letters.
- First actions: read a project's queue, check your Pylons, ask your
  project's agent. Then the others in section 9.1, one at a time.

Files and systems:

- The OpenAgents API (`coax/web/api/`): link, envoy-order, confirm and result
  routes; migrations for `agentics.link.v1` and the audit table; NIP-98 for
  agent keys.
- The account page and the phone: the confirmation screen and the limits.
- The room: envoy intents to pending orders; result events to letters.
- The project agent's runner: accept a turn that came from the bench, with
  the bench order id recorded on the turn.

Tests:

- A seat whose character isn't linked to the account can't create an envoy
  order; a seat with another player's character can't either.
- An order isn't run without a confirmation from the account's web session
  or phone key; a confirmation from the game socket is refused.
- An outward-facing or paid action can't be set to "without asking".
- Spending limits: an order over the per-action, daily or monthly limit is
  refused, and the limit is named; limits start at zero.
- Pending orders expire after 15 minutes; unlinking cancels pending orders.
- The audit log has one row per order with every state change, and the
  report letter matches it.
- Money is sats only; no path converts gold to sats or sats to gold.
- An end-to-end run on staging: link a test character to a test account,
  ask a test project's agent for a milestone change, confirm on the web,
  receive the letter, then UNDO TURN from the web.

## 13. Open questions for the owner

1. **The player-facing name.** Bindcraft and the Binding Table, or another
   from section 1? And should the out-of-world envoy have its own name in the
   world (an envoy, a "Veil-speaker")?
2. **The workday.** Is 20 real minutes for an SRD 8-hour workday right?
   Shorter makes crafting fast and inflates goods; longer makes it a
   come-back-tomorrow system like Neverwinter's 2-8 hour tasks.
3. **Out-of-world gating.** Should envoys wait for rank 5, or should a linked
   account get read-only envoy actions (project queue, Pylon status) from rank
   1, so players see the bridge early?
4. **Speeding up.** Neverwinter sold instant crafts. This spec has no paid
   skip. Do you want any skip at all, and never for sats?
5. **Rank thresholds.** How many tasks per rank? A first guess: 10, 40, 120,
   300 task-days.
6. **Trading agents.** Can a player give or sell an agent, or are agents
   bound to one character for good?
7. **Agents that earn sats.** "Agents that moonlight" is on the README's
   roadmap: should an envoy be able to *take* paid work (a project's task) and
   earn Bitcoin for the account, and if so, with what review?
8. **Other players' benches.** Can a friend use your bench for their own
   in-world orders, as the SRD allows one assistant, or is a bench always its
   owner's alone?

## 14. Sources

SRD:

- System Reference Document 5.2.1, Wizards of the Coast LLC, CC-BY-4.0: Tools
  and Tool Proficiency, p. 93; Artisan's Tools, pp. 93-94; Hirelings and
  Services, p. 102; Crafting Nonmagical Items, Brewing Potions of Healing and
  Scribing Spell Scrolls, p. 103; Crafting Magic Items, pp. 206-207; Find
  Familiar, p. 130; Unseen Servant, p. 171; Planar Binding, pp. 152-153;
  Animate Objects, pp. 108-109; Homunculus, p. 299; Skilled, p. 87; Travel
  Pace, pp. 12 and 192. Spell text is also in
  [`aethyr/rules/srd/data/spells.coax`](../../aethyr/rules/srd/data/spells.coax).

MMOs (summarized in our own words; fan wikis are marked):

- Neverwinter Wiki (fan), [Leadership](https://neverwinter.fandom.com/wiki/Leadership)
  and [Profession](https://neverwinter.fandom.com/wiki/Profession).
- Arc Games, [Update: Leadership on Gateway](https://www.arcgames.com/en/games/neverwinter/news/detail/9489863).
- MMO Fallout, [Neverwinter: Leadership Suspension Hasn't Led To In-Game Botting](https://mmofallout.com/neverwinter-leadership-suspension-hasnt-led-to-in-game-botting).
- MMOBomb, [Neverwinter Overhauling Its Profession System, With New Recipes And Over 200 Artisans](https://mmobomb.com/news/neverwinter-overhauling-profession-system-new-recipes-200-artisans) (2018-09-19).
- Arc Games forum, [Official feedback thread: Professions Overhaul](https://forum.arcgames.com/neverwinter/discussion/1242676/official-feedback-thread-professions-overhaul/p8).
- MMOBomb, [Neverwinter Explains How Masterwork Professions Actually Work](https://mmobomb.com/news/neverwinter-explains-masterwork-professions-actually-work).
- Neverwinter Wiki (fan), [Forgotten Masterwork Professions](https://neverwinter.fandom.com/wiki/Forgotten_Masterwork_Professions).
- Warcraft Wiki (fan), [Garrison mission](https://warcraft.wiki.gg/wiki/Garrison_mission)
  and [Garrison follower](https://warcraft.wiki.gg/wiki/Garrison_follower).
- SWTOR, [Crew Skills](https://www.swtor.com/holonet/crew-skills); SWTOR Wiki
  (fan), [Crew Skill](https://swtor.fandom.com/wiki/Crew_Skill).
- Console Games Wiki (fan), [Retainer Venture](https://ffxiv.consolegameswiki.com/wiki/Retainer_Venture).
- EVE Online, [EVE Industry: All you want to know](https://www.eveonline.com/news/view/eve-industry-all-you-want-to-know);
  EVE University, [Industry](https://wiki.eveuniversity.org/Industry).
- RuneScape Wiki, [Player-owned port](https://runescape.wiki/w/Player-owned_port)
  and [Captains and crew](https://runescape.wiki/w/Player-owned_port/Captains_and_crew?oldid=10894578).

Repository:

- [Everglade multiplayer spec](../../docs/verse/2026-11-03-everglade-multiplayer-spec.md),
  [realm invites and auth](../../docs/auth/2026-10-31-realm-invites-and-auth.md),
  [project agent](../../docs/project-agent.md),
  [projects](../../docs/protocol/2026-10-28-projects.md),
  [Lightning](../../docs/lightning/README.md),
  [makers](../../docs/verse/makers.md),
  [Pylon guard decisions](../../docs/verse/pylon-guard-decisions.md),
  [SRD compliance ledger](../../docs/dnd/srd-compliance-ledger.md),
  [chapter 18, Turns become time](../book/ch018-turns-become-time.md).

---

This work includes material from the System Reference Document 5.2.1 ("SRD
5.2.1") by Wizards of the Coast LLC, available at
https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative
Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.
