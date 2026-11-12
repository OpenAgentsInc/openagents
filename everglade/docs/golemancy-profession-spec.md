# Golemancy: a profession for commanding golems

Status: **the name is decided; everything else is a design proposal.**

- **Decided (owner, 2026-11-09):** the profession is called **Golemancy**, in
  the game and in the code, docs, records and issue labels alike. There is no
  other internal name. Its practitioner is a **golemancer**, and the agents
  they command are **golems**.
- **Exists today, outside the game:** the Golemancers' Workshop in the
  Everglade desktop app: Next issue, Autopilot, the rubric that picks a model
  per issue (`coax/lib/agent/tier.coax`) and the Golems board
  ([desktop-app-spec](desktop-app-spec.md), [next-issue-spec](next-issue-spec.md)).
  Its golems are Claude Code agents by default; since 2026-11-11 (#13193) the
  coding golem can be Antigravity (`agy`) on Gemini models, the rubric can
  name Gemini's tiers, and the board shows Antigravity's conversations.
- **Proposal only:** every other rule, number and name below. It is kept here
  as a starting point for discussion. None of it is agreed and none of it is
  built, except the pieces marked **exists today**.

Owner's original ask: a profession where "the workbench lets characters start
to control agents … command agents from an agent workbench inside the world to
have actions taken inside the game world or outside of the game world."

Golemancy is Everglade's own invention. It isn't SRD content, and every record
it adds would be marked `srd: false`, as the house spells in
[`rules/srd/house.coax`](../../rules/srd/house.coax) are. The
proposal builds it from SRD 5.2.1 rules wherever they apply: tool proficiency,
ability checks, crafting time and cost, Help, Find Familiar, Unseen Servant and
the Homunculus.

## Contents

1. [The name](#1-the-name)
2. [What the SRD already says](#2-what-the-srd-already-says)
3. [What other MMOs did](#3-what-other-mmos-did)
4. [The profession](#4-the-profession)
5. [The workbench](#5-the-workbench)
6. [Golems](#6-golems)
7. [Commanding golems](#7-commanding-golems)
8. [Actions inside the world](#8-actions-inside-the-world)
9. [Actions outside the world: the bridge](#9-actions-outside-the-world-the-bridge)
10. [Safety](#10-safety)
11. [Data model](#11-data-model)
12. [Phases](#12-phases)
13. [Open questions for the owner](#13-open-questions-for-the-owner)
14. [Sources](#14-sources)

## 1. The name

**Golemancy** (decided). A golemancer shapes bodies of clay, wood, stone or
brass and quickens them with a written command. The golem does what the
command says and nothing else.

Golems in the old stories are warriors; in Everglade they are workers. That is
the one thing the fiction has to establish early: an Everglade golem can't
attack (section 8.2), the same rule the SRD gives its servants.

Proposed names for the rest (not decided):

| Thing | Proposed name |
| --- | --- |
| The bench in your house | **Golemancer's Bench** ("the bench" in this doc) |
| The tool kit | Golemancer's Tools |
| A golem's upkeep | Golem salt |
| A golem that acts outside the world | An envoy |
| The bench's link to the world outside | The Veilstone |
| Code prefix and records | `golem_` in code, `golemancy.*` records |

## 2. What the SRD already says

The SRD 5.2.1 has no profession system, no Downtime chapter and no Bastions
(those are in other 5E books, not the SRD). It does have the pieces a
profession needs. Page numbers are the SRD 5.2.1 PDF's.

| Rule | SRD 5.2.1 | What it gives Golemancy |
| --- | --- | --- |
| Tools: Ability, Utilize, Craft, Variants | Equipment, "Tools", p. 93 | The shape of a new tool kit's record |
| Tool proficiency | Equipment, "Tool Proficiency", p. 93 | Add your Proficiency Bonus to a check with the tool; with proficiency in a skill used in the same check, you also have Advantage |
| Artisan's Tools | Equipment, pp. 93-94 | Each tool needs its own proficiency; Tinker's Tools (50 GP, Dexterity) is the closest existing kit |
| Utilize action | Playing the Game, "Actions", p. 10 | "Use a nonmagical object": using the bench is a Utilize action |
| Skilled origin feat | Feats, p. 87 | "Proficiency in any combination of three skills or tools of your choice": an SRD route to the new tool |
| Hirelings | Equipment, "Services", p. 102 | Skilled hireling 2 GP a day, untrained 2 SP a day, messenger 2 CP a mile: the price floor for golem work |
| Crafting Nonmagical Items | Equipment, p. 103 | Raw materials cost half the item's price; days of 8 hours = price in GP ÷ 10, rounded up; days needn't be consecutive; one assistant with the same proficiency divides the time |
| Crafting Magic Items | Magic Items, pp. 206-207 | Arcana proficiency plus the right tool (Tinker's Tools for a Wondrous Item); Common 5 days and 50 GP, Uncommon 10 days and 200 GP, Rare 50 days and 2,000 GP |
| Homunculus | Monsters, p. 299 | A Tiny Construct with a Telepathic Bond to its maker on the same plane: the model for the first golem |
| Unseen Servant | Spells, p. 171 | A mindless force that performs "simple tasks that a human could do, such as fetching things, cleaning, mending"; it can't attack |
| Find Familiar | Spells, p. 130 | A servant that "acts independently of you, but … obeys your commands"; it can't attack; telepathy within 100 feet |
| Animate Objects | Spells, pp. 108-109 | Combat constructs under Concentration: the model of what golems must *not* become |
| Help action | Playing the Game, "Actions", p. 10 | The only combat assistance a golem gives (section 8.3) |
| Travel Pace | Playing the Game, p. 12; Gameplay Toolbox, p. 192 | Normal pace is 24 miles a day: how long an errand between towns takes |

Two SRD lessons shape the proposal:

- **Servants in the SRD can't attack.** Find Familiar and Unseen Servant both
  say so outright. A golem follows the same rule.
- **Crafting runs on 8-hour workdays that needn't be consecutive.** That is
  already a timed queue; the proposal only maps the workday onto real time
  (section 4.4).

**Exists today.** Tool proficiency and checks are in
[`rules/srd/check-training.coax`](../../rules/srd/check-training.coax)
(37 SRD tools as bits, `rs_tool_bit`) and
[`ability-checks.coax`](../../rules/srd/ability-checks.coax), with the
tool-plus-skill Advantage rule. Help is in
[`help.coax`](../../rules/srd/help.coax). Find Familiar runs as a shared
actor in [`familiar.coax`](../../rules/srd/familiar.coax) and its
senses and touch files. Equipment crafting is **missing**: the
[compliance ledger](../../docs/dnd/srd-compliance-ledger.md) lists
CONTENT-EQUIPMENT as partial, with crafting not modeled.

## 3. What other MMOs did

### Neverwinter

The closest model: also built on 5E-style rules, and it let players run
professions from outside the game.

- **Leadership** was a profession of hired mercenaries assigned to timed tasks
  of 2 to 8 hours, with a cap on tasks of one rank at a time.
- **The Gateway** was a web app for assigning profession tasks without logging
  in. Bots used it to run Leadership around the clock and mass-produce Astral
  Diamonds. Cryptic pulled Leadership from the Gateway, then brought it back in
  September 2015 with those rewards removed.
- **The 2018 overhaul** replaced faceless assets with a personal **workshop**
  and over 200 named **artisans** who complete orders over time, even while
  you are away. A few instant crafts a day were free; more cost Astral
  Diamonds.

Lessons the proposal takes:

1. Named golems in a room you own beat icons in a menu.
2. Any out-of-game control surface gets botted; if it pays a tradable
   currency, it inflates the economy. Golemancy never pays a tradable currency
   from an unattended loop (section 10).
3. Paying to skip the timer turns a profession into a store. No paid skip
   (open question 13.3).

### Other games

- **World of Warcraft garrisons:** the mission table shows the success chance
  before you commit. Show the odds, computed by the rule that resolves the
  task.
- **Star Wars: The Old Republic crew skills:** real-time missions that can fail
  and lose their cost; simultaneous missions grow with level.
- **Final Fantasy XIV retainers:** ventures bring back items, not money.
  Golems produce goods, not money.
- **EVE Online industry:** the job queue and its timers are the game.
- **RuneScape Player-Owned Ports:** a voyage's success is set by the
  least-met requirement, and crews are capped. A simple bottleneck rule makes
  team choice interesting.

## 4. The profession

### 4.1 In the world

A golemancer shapes small golems and quickens them with a command written on a
slip of vellum sealed inside the body. Golemancers are clerks, couriers and
caretakers more than wizards; in Everglade's towns you meet them at a
counting-house or a post. The first golemancer you meet teaches you the trade
and sells you a kit. Your house's bench is where you shape golems, write their
commands and read what they report.

### 4.2 The tool: Golemancer's Tools

> **Golemancer's Tools (50 GP)** *(Everglade, not SRD)*
> **Ability:** Intelligence
> **Weight:** 6 lb.
> **Utilize:** Read the command written inside a golem (DC 10), or quiet a
> golem that is refusing an order (DC 15)
> **Craft:** Golem bodies (section 6.2), command scrolls, golem salt

- It costs what Tinker's Tools costs, the nearest SRD kit, and uses
  Intelligence because the work is writing commands, not handwork.
- **Proficiency** adds your Proficiency Bonus to every Golemancy check. With
  **Arcana** proficiency too, you have Advantage, as the SRD's tool rule says.
- In code it is a house tool bit next to the SRD's 37 in
  `check-training.coax`, in a reserved house range (for example bit 48 and up),
  so `RS_TOOLS` still counts SRD tools only.

### 4.3 Becoming a golemancer

- **Skilled** (origin feat, SRD p. 87), choosing Golemancer's Tools as one of
  its picks.
- **A background grant** Everglade adds later, marked as its own.
- **Training** from a golemancer NPC: a quest, a fee and workdays. A declared
  house rule (`RS_DEV_TRAINING`) in the world profile of
  [chapter 18](../book/ch018-turns-become-time.md).

Without proficiency you can still run one golem on the simplest tasks, rolling
without your Proficiency Bonus.

### 4.4 Time: the workday in real time

The SRD measures crafting in 8-hour days. The proposal keeps the day as the unit
and maps it to real time with one world constant, shared with the
[Housewright](housewright-profession-spec.md) spec:

```text
WORKDAY_S = 1200      one SRD workday (8 hours of work) = 20 real minutes
```

- A task's length is stated in SRD days, then converted: a Heavy Crossbow
  (50 GP, 5 days) is 100 real minutes at the bench.
- **Golems work while you are offline.** The server keeps the clock.
- The mapping is a declared deviation, `RS_DEV_WORKDAY`. A strict world sets
  `WORKDAY_S` to 28,800 (8 real hours).

### 4.5 Costs

All in-game currency and materials, from SRD prices where they exist:

| Cost | Rule |
| --- | --- |
| A task's wage | Hireling pay: 2 SP a workday untrained, 2 GP skilled, 2 CP a mile for a message |
| Materials for a crafted item | Half its price (SRD p. 103) |
| A golem body | Crafted as a nonmagical item from its price, or a Common magic item (5 days, 50 GP) for a beast golem |
| Quickening a golem | 10 GP of ink and incense, the price of Find Familiar's incense |
| Golem salt | Upkeep: one per workday of work, craftable with Golemancer's Tools |

Money for out-of-world actions is separate and is never in-game currency
(section 9.6).

### 4.6 Ranks

Five ranks, earned by completed tasks, never by spending. Rank is a profession
rank and never changes combat numbers.

| Rank | Title | Golems at once | Tasks queued per golem | Unlocks |
| --- | --- | --- | --- | --- |
| 1 | Apprentice | 1 | 3 | Household and gathering tasks in your house and town |
| 2 | Journeyman | 2 | 3 | Errands between towns |
| 3 | Adept | 3 | 4 | Crafting help; guarding the house; live commands (phase 2) |
| 4 | Master | 4 | 5 | Beast golems; scouting; two golems on one task |
| 5 | Grand Golemancer | 5 | 6 | Envoys: out-of-world actions (phase 3) |

Every limit is shown on the bench, with the next rank's value, before you hit
it.

## 5. The workbench

### 5.1 Where it is

The bench is the workbench in the player's first house
([`everglade/world/home.coax`](../world/home.coax), id `home/workbench`).
Golemancy reads the bench id; it never places or moves the prop.

### 5.2 Using it

1. Walk to its spot; the prompt says **Use the workbench**.
2. Press interact: an SRD **Utilize** action, not in combat or while
   Incapacitated.
3. The bench view opens over the world; other players see you working at it.
4. Moving away or taking damage closes the view. Submitted orders stay
   submitted.

### 5.3 What the bench shows

| Panel | Contents |
| --- | --- |
| **Golems** | Each golem by name, its body, what it is doing, time left, golem salt |
| **Orders** | Each golem's queue, with each task's days, cost, DC and success chance |
| **Reports** | Finished tasks: what happened, the roll, what came back. Also sent as letters |
| **Shaping** | Shape and quicken a new golem, move one to a new body, unmake one |
| **Envoys** (rank 5, phase 3) | Linked OpenAgents account, allowed actions, spending limits, pending confirmations |

The success chance is computed by the same server function that resolves the
task.

## 6. Golems

### 6.1 What a golem is

A construct quickened by a written command, owned by one character:

- It **can't attack**.
- It acts on its own task clock, except during live commands (section 7.3).
- At 0 Hit Points its body breaks. The command scroll survives; you seal it in
  a new or repaired body for the body's cost, keeping its name and aptitudes.
- Golems can't be given to or commanded by another player (open question
  13.5).

### 6.2 Kinds of golem

| Kind | Body | Stats (from the SRD) | Rank | What it does |
| --- | --- | --- | --- | --- |
| **Hearth golem** | A small clay or wood figure | Unseen Servant's numbers: AC 10, 1 HP, Strength 2 | 1 | Household tasks: tidy, mend, sort, stock |
| **Homunculus** | A carved construct with a Telepathic Bond | The SRD Homunculus (p. 299), its Bite removed | 1-2 | Gathering, errands, carrying, shopping |
| **Beast golem** | A brass or wooden owl, hawk or cat | A Find Familiar form's stat block, as a Construct | 4 | Scouting, guarding, longer errands |
| **Envoy** | A command sealed in the bench's Veilstone | No body in the world | 5 | Out-of-world actions (section 9) |

### 6.3 Golem record

- A player-chosen name, filtered like character names.
- A kind and a body.
- **Golemancy checks** with the golemancer's numbers: Intelligence modifier,
  plus Proficiency Bonus if proficient, Advantage with Arcana too.
- An **aptitude** in one task family (gathering, crafting, scouting, errands,
  guarding): +1 per 10 tasks of that family, up to +3. Everglade's own.

## 7. Commanding golems

All commands go through the room server; a client only sends an intent.

### 7.1 Orders (phase 1)

```text
order  := golem, task kind, target (a place, an item, a recipient), repeat count
task   := days (SRD), wage, materials, DC, ability, aptitude family
```

1. Pick a golem and a task; the bench shows days, real time, cost, DC and
   chance.
2. Confirm. The server takes the wage and materials at once and queues it.
3. When the timer ends, the server rolls d20 + the golemancer's modifiers +
   aptitude against the DC (`rs_check_plan` with the Golemancer's Tools bit).
4. Success: the result goes to your house chest or mailbox. Failure: half the
   materials back, no product; the report shows the roll.
5. A golem works one task at a time.

**Helping.** Two of your golems on one task (rank 4) halve its days, as the
SRD's one assistant does. A proficient player at your bench can take the Help
action, giving your check Advantage (`help.coax`).

### 7.2 Standing orders (phase 1)

A standing order repeats until you stop it, materials or wages run out, or 7
SRD days pass. Then it stops and asks you to renew it at the bench, so no
unattended loop runs forever.

### 7.3 Live commands (phase 2)

At rank 3 you can take one golem out with you:

- It follows within 100 feet (Find Familiar's telepathy range).
- As a **Bonus Action** you command it to move and interact with an object,
  carry an item, hold a spot, or take the **Help** action for you.
- Its queued tasks wait while it is out.
- It reuses the familiar's intents and shared actor path
  (`docs/protocol/everglade-familiar-v1.md`).

### 7.4 Spoken and typed orders (phase 2)

At the bench you can type or say an order ("send Pip to Riverford with the
letter for the smith"). The text goes through the **semantic selector** to a
typed order, never keyword matching. The bench shows the typed order with its
cost and chance, and you confirm it.

## 8. Actions inside the world

### 8.1 What golems can do

| Family | Examples | Rule |
| --- | --- | --- |
| Household | Tidy, mend, sort the chest, restock the larder | Unseen Servant's simple tasks |
| Gathering | Herbs, wood, stone near the house or along a known road | Wisdom (Survival) or Intelligence (Nature) by area |
| Crafting help | A golem as the SRD's assistant on your crafting job | Halves the time; needs the craft's tool proficiency, carried by an "apprentice" body |
| Scouting | Map an area you've visited; report creatures near a road | Beast golems only; Cartographer's Tools DC 15 |
| Errands | Carry an item or letter to another town or mailbox; buy from a shop | Travel Pace (24 miles a day); 2 CP a mile |
| Guarding | Watch the house and report who entered | Passive Perception; raises an alarm, never fights |

### 8.2 What golems never do

- **No combat.** No attacks, attack spells, damage or Opportunity Attacks.
- **No botting your own character.** Nothing moves, aims, casts or chooses for
  your character.
- **No loot** from monsters or players; no pickpocketing.
- **No trades and no market.**
- **No dungeons, raids or contested areas.**
- **Tasks start only at your own bench.**

### 8.3 Assisting in combat

Only the SRD's **Help** action during a live command: Advantage on your next
check, or your next attack against a creature within 5 feet of the golem. It
takes the golem's action and happens only on your command.

### 8.4 Limits

- The room server resolves every task; the client never rolls.
- Per character: the rank's golems and queue lengths, at most 60 orders an
  hour, one standing order per golem.
- Golems gather from the same shared nodes as players at half a player's rate.

## 9. Actions outside the world: the bridge

At rank 5 an **envoy** can act outside Everglade through the player's own
OpenAgents account.

### 9.1 What there is to command

| Action | Built from | Status |
| --- | --- | --- |
| Ask a project's agent (one turn = one commit, with UNDO TURN) | [Project agent](../../docs/project-agent.md) | Spec (draft) |
| Read a project's queue | [Projects](../../docs/protocol/2026-10-28-projects.md) | Read path exists |
| Start a Coder task | [Coder book](../../coder/book/README.md) | No hosted entry point yet |
| Check your Pylons | [Pylon guard decisions](../../docs/verse/pylon-guard-decisions.md) | Online list exists; per-owner view proposed |
| Make or edit a realm | [Realm builder](../../docs/verse/realm-builder.md), [realm edit](../../docs/verse/realm-edit.md) | Exists |
| Tip a maker in Lightning | [Makers](../../docs/verse/makers.md) | Exists |
| Post a signed Nostr note | NIP-98 and Nostr signing | Proposed |

Phase 3 would start with the read-only rows and one write row (ask a project's
agent).

### 9.2 Architecture

```text
  bench (client)          room server (oa-room)            OpenAgents API             agent
  envoy order intent ──▶  checks seat, link, rank, action
                          writes pending order  ─────────▶ pending order
  confirm on openagents.com or the phone ◀────────────────────┘
  (session cookie or NIP-98, never the game socket)
                                                           confirmed ───────────────▶ runs
  letter and bench report ◀── room delivers ◀──────────── result, receipt ◀──────── done
```

- **The room server** is the authority in the world
  ([multiplayer spec §3.1](../../docs/verse/2026-11-03-everglade-multiplayer-spec.md)).
  It creates *pending* orders only, holds no user secrets and can't approve.
- **The OpenAgents API** is the authority outside it: account, linked
  identities ([realm invites and auth](../../docs/auth/2026-10-31-realm-invites-and-auth.md)),
  agent keys, permissions and limits.
- **Confirmation** happens on openagents.com (`oa_session`) or the iOS app's
  Keychain key. A hijacked game seat can queue but can't approve.
- **The agent doing the work** runs with the account's normal server-side
  checks. The bench adds no new power.
- **Results** come back as a letter and a bench report, with a link and the
  cost.

### 9.3 Linking

At the Envoys panel, **Link your OpenAgents account** shows a one-time code;
you confirm on openagents.com or the phone. The link starts with no allowed
actions and a zero spending limit. Unlinking cancels pending orders. Guests
can't link; a character links to one account.

### 9.4 Consent

| Kind | Example | Confirmation |
| --- | --- | --- |
| Read | List a project's tasks | None after linking, if allowed |
| Private write | Ask your project's agent to edit a milestone | Each time, or "without asking" for that action |
| Outward-facing | Post, comment, open a pull request | Always, outside the game |
| Costs money | Tip, pay for compute | Always, outside the game, within your limits |

### 9.5 Permissions

Default nothing. Actions run with the account's own permissions, on resources
it owns or belongs to; the bench can't widen them.

### 9.6 Money

- **Bitcoin only**, priced in sats ([money rule](../../docs/lightning/README.md)).
- **Gold never buys real actions, and sats never buy gold.**
- Per-action, daily and monthly limits, all zero until raised; an order over a
  limit is refused with the limit named. Every payment has a receipt.

### 9.7 Audit log

```text
golemancy_audit: order id, account, character, room, bench id, action, args hash,
                 cost (sats), state (pending → confirmed | refused | expired → running → done | failed),
                 who confirmed and where, timestamps, result link, receipt
```

Pending orders expire after 15 minutes.

## 10. Safety

1. Only the linked account's own surface can confirm an envoy order; other
   players and party members can't send or confirm one.
2. Linking is off by default, with every action off and every limit at zero.
3. Anything outward-facing or paid is confirmed outside the game, every time.
4. **No unattended money loop:** standing orders can't post or pay; envoys
   never earn gold and in-world tasks never earn sats.
5. At most 20 envoy orders an hour and 100 a day per account; repeated
   refusals lock the Envoys panel for an hour.
6. **Text is data.** Nothing a golem reads becomes an instruction without the
   typed order and its confirmation.
7. Server authority everywhere: clients send intents only.
8. Golem names are filtered; errand letters use chat moderation and block
   lists.
9. Owner, team and agent test sessions are excluded from usage numbers.

## 11. Data model

Versioned protocol records, like the familiar's:

```text
golemancy.profession.v1   character id, rank, task count by family, tool proficiency
golemancy.golem.v1        golem id, owner character, name, kind, body, HP, aptitudes, golem salt, state
golemancy.order.v1        order id, golem id, task kind, target, repeat, days, wage, materials, DC,
                          state (queued → running → succeeded | failed | cancelled), start, end, roll
golemancy.report.v1       order id, outcome, roll, items, letter text
golemancy.bench.v1        bench id (from the house), owner character, golems housed
golemancy.link.v1         character id, account id, allowed actions, limits (sats), created, revoked
golemancy.envoy-order.v1  the audit row of section 9.7
```

In-world records live with the character's state on the room server; the link
and envoy-order records live in the OpenAgents API's database. A task ends at
`start + days × WORKDAY_S`.

## 12. Phases

Each phase would ship whole, with its checks, before the next.

**Phase 1: the bench and in-world timed tasks.** Golemancer's Tools as a house
tool bit; `RS_DEV_WORKDAY` and `RS_DEV_TRAINING`; hearth golems and
homunculi, orders, standing orders, reports and letters; ranks 1 and 2.
New `rules/srd/golemancy.coax` (`#authority`, zero mutation survivors),
`golemancy-checks.coax` and `crafting.coax` (the SRD's nonmagical crafting,
which the ledger's CONTENT-EQUIPMENT row also needs); `docs/protocol/golemancy-*-v1.md`.
Tests: crafting math matches the SRD's examples; proficiency and Arcana
Advantage (`#forall` over training masks); the bench's shown chance equals the
resolver's; end times survive a server restart; limits refuse with the limit
named; wages taken once and failures refund once; a headless world fixture
from bench to report.

**Phase 2: live commands and typed orders.** Beast golems, scouting and
guarding; ranks 3 and 4; the familiar's actor path generalized to one familiar
and one live golem; typed orders through the semantic selector. Tests: every
attack intent from a golem is refused; no golemancy intent moves or acts for
the player's own character; Help gives Advantage once; a golem past 100 feet
stops and waits; typed orders equal the menu's and never run unconfirmed.

**Phase 3: the out-of-world bridge.** Envoys, linking, permissions, limits,
confirmation, audit and result letters, in the OpenAgents API
(`coax/web/api/`), the account page and the phone. Tests: unlinked or other
players' seats can't create envoy orders; a confirmation from the game socket
is refused; paid and outward-facing actions can't be "without asking"; limits
start at zero and refuse with the limit named; pending orders expire; one audit
row per order; no path converts gold and sats; a staging run from link to
UNDO TURN.

## 13. Open questions for the owner

1. **Everything after the name.** Which parts of this proposal, if any, to keep:
   the bench name, the tool, the golem kinds, the ranks, the envoy bridge.
2. **The workday.** Is 20 real minutes per SRD workday right?
3. **Speeding up.** Any skip at all, and never for sats?
4. **Out-of-world gating.** Envoys at rank 5, or read-only envoy actions from
   rank 1?
5. **Trading golems.** Can a player give or sell a golem?
6. **Golems that earn sats.** Should an envoy take paid work for the account,
   and with what review?
7. **Other players' benches.** Can a friend use your bench?

## 14. Sources

SRD: System Reference Document 5.2.1, Wizards of the Coast LLC, CC-BY-4.0:
Tools and Tool Proficiency, p. 93; Artisan's Tools, pp. 93-94; Hirelings,
p. 102; Crafting Nonmagical Items, p. 103; Crafting Magic Items, pp. 206-207;
Find Familiar, p. 130; Unseen Servant, p. 171; Animate Objects, pp. 108-109;
Homunculus, p. 299; Skilled, p. 87; Travel Pace, pp. 12 and 192. Spell text
is also in [`rules/srd/data/spells.coax`](../../rules/srd/data/spells.coax).

MMOs (summarized in our own words; fan wikis are marked):

- Neverwinter Wiki (fan), [Leadership](https://neverwinter.fandom.com/wiki/Leadership)
  and [Profession](https://neverwinter.fandom.com/wiki/Profession).
- Arc Games, [Update: Leadership on Gateway](https://www.arcgames.com/en/games/neverwinter/news/detail/9489863).
- MMO Fallout, [Neverwinter: Leadership Suspension Hasn't Led To In-Game Botting](https://mmofallout.com/neverwinter-leadership-suspension-hasnt-led-to-in-game-botting).
- MMOBomb, [Neverwinter Overhauling Its Profession System, With New Recipes And Over 200 Artisans](https://mmobomb.com/news/neverwinter-overhauling-profession-system-new-recipes-200-artisans) (2018-09-19).
- Warcraft Wiki (fan), [Garrison mission](https://warcraft.wiki.gg/wiki/Garrison_mission).
- SWTOR, [Crew Skills](https://www.swtor.com/holonet/crew-skills).
- Console Games Wiki (fan), [Retainer Venture](https://ffxiv.consolegameswiki.com/wiki/Retainer_Venture).
- EVE Online, [EVE Industry: All you want to know](https://www.eveonline.com/news/view/eve-industry-all-you-want-to-know).
- RuneScape Wiki, [Player-owned port](https://runescape.wiki/w/Player-owned_port).

Repository: [Everglade multiplayer spec](../../docs/verse/2026-11-03-everglade-multiplayer-spec.md),
[realm invites and auth](../../docs/auth/2026-10-31-realm-invites-and-auth.md),
[project agent](../../docs/project-agent.md),
[projects](../../docs/protocol/2026-10-28-projects.md),
[Lightning](../../docs/lightning/README.md),
[makers](../../docs/verse/makers.md),
[SRD compliance ledger](../../docs/dnd/srd-compliance-ledger.md),
[chapter 18, Turns become time](../book/ch018-turns-become-time.md).

---

This work includes material from the System Reference Document 5.2.1 ("SRD
5.2.1") by Wizards of the Coast LLC, available at
https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative
Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.
