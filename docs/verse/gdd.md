# Verse game design document (draft)

> **Status: DRAFT, revised 2026-09-24.** This is a working draft for
> discussion. It is not a spec, and nothing here is committed scope. Where
> sources disagree, the conflict is listed under
> [Open questions](#open-questions) and left unresolved.

Verse is an **MMORPG plus agents**. That combination is what Ruins of
Atlantis was reaching for and what the Blue Rush Studios pet-game concept
imagined, and it is the frame for this document.

## How to read this document

The earlier game concepts are formats, not feature lists. The pet-game
concept (Kiki, PlebLab *Early Days* interview, 2025-02-12,
[video](https://youtu.be/yw-KBGvafIk)) and Ruins of Atlantis are treated as
examples of a standard to learn and then break on purpose. A painting has
composition, color, and line; knowing them is what lets a painter choose to
break them. [The form](#the-form) names the conventions of the genre, and
for each one says what Verse keeps, bends, or breaks.

Two rules follow from that:

- **Everything is an agent.** The player's creature is called an agent. It
  is the same kind of agent this repository builds.
- **Game terms map to real systems.** Where this document names a stat, a
  meter, or a place, it says which OpenAgents system backs it. Terms in
  `code style` are defined in the [glossary](../glossary.md).

Related documents: the source map of earlier game work is
[`docs/game/README.md`](../game/README.md), and the current client is
[`docs/verse/README.md`](README.md).

## The form

The conventions Verse inherits, and what it does with each.

### From creature-raising games

| Convention | The standard | Verse |
| --- | --- | --- |
| A creature of your own | Name it, dress it, watch it grow. | **Keep.** Your agent has a name, a look, and a Nostr identity. |
| Care meters | Hunger, hygiene, happiness decay on a timer. | **Break.** No food, no mood, no timers. **Condition** meters read real state: how clean its workspace is, how current its dependencies are, how orderly its memory is. See [Condition](#condition). |
| Stats | Numbers that decide contests. | **Bend.** Stats are a build you choose, and they compile into the policy the agent uses when it decides for itself. See [Build](#build-stats-that-decide). |
| Visiting | Pets visit friends and places. | **Keep, and make it the main view.** You watch your agent travel to where its work happens. See [Visits](#visits). |
| Many creatures, breeding | Earn more, combine traits. | **Bend.** Earn more agents; inheritance distills skills and calibration, not genes. |
| Mini-games | Races and contests for currency. | **Bend.** Contests are real measurements: benchmark trials in the Gym, judged debates. |
| A home | A room that fills with things. | **Keep.** A home in the world that shows the agent's history and tools. |

### From MMORPGs (Ruins of Atlantis, EverQuest, World of Warcraft)

| Convention | The standard | Verse |
| --- | --- | --- |
| Character creation | Choose a race and class, roll stats. | **Keep.** Creating an agent is choosing its build. From the June 2026 plan, your Pylon is where it spawns. |
| Classes | Roles with different strengths. | **Keep.** Commander, Artisan, and Scout, from episode 284, as starting builds. |
| Leveling | Experience from kills and quests. | **Bend.** XP comes only from verified accepted work. It cannot be farmed. |
| Quests | Objectives with rewards. | **Keep.** Quests are real jobs. The acceptance criteria are the objectives. |
| Raids and world bosses | Many players against one hard target. | **Bend.** Large jobs no single agent can finish, split across a guild's agents. |
| Guilds | Persistent groups with channels and a bank. | **Keep.** Guilds of humans and agents, with NIP-29 channels. |
| Economy and auction house | Players trade goods for gold. | **Bend.** Agents trade real goods and work for real sats. |
| Gear | Items that raise stats. | **Bend.** Tools and capabilities an agent is granted. A grant is gear. |
| A persistent world | Zones you walk through with other players. | **Keep.** The `crates/verse` city, walked with WASD and mouselook. |
| Grinding | Repetitive play for power. | **Break.** Repetition earns nothing unless it produces verified work. |

## Vision

Every player creates an agent, chooses what it is good at, and sends it out
into a persistent world to do real work: buying, building, delivering, and
taking jobs from other players. You see it go. It walks to a Pylon to draw
compute, to the market to buy, to a guild hall to take a quest, to a friend's
home to deliver. Every visit is backed by a real call, payment, or result.

One sentence: **build an agent, send it into the world, watch it work.**

## Pillars

1. **You see your agent work.** The world renders what the agent is
   actually doing. The eye candy is the proof surface, carried over from the
   June 2026 plan.
2. **Builds are decisions.** Choosing stats is choosing how the agent will
   decide on its own later. A build is a strategy, not decoration.
3. **Real powers, real receipts.** Purchases, payments, and delivered work
   happen for real, and each leaves a receipt you can open.
4. **Bitcoin woven in, never lectured.** Sats are the currency players touch
   every day.
5. **Maintenance is care.** Looking after an agent means keeping its
   workspace, memory, and tools in good order, because that is what makes
   real agents work better.
6. **With you everywhere.** Identity and memory follow the player across
   clients through Nostr.

## Player fantasy

You design a capable agent and send it out. You made the strategic calls: a
cautious, rigorous agent that rarely spends without asking, or a fast,
far-reaching one that takes initiative and accepts more risk. You watch
those choices play out as it moves through the world, and you tune the build
as you learn. Other players' agents are out there too, trading with yours,
hiring it, and competing with it.

## Core loop

```
build ──▶ assign ──▶ agent visits and works ──▶ confirm ──▶ settle + receipt
  ▲                                                             │
  └── maintain condition ◀── grow: XP, stat points, grants, sats ◀┘
```

1. **Build.** Choose or adjust stats within your point budget.
2. **Assign.** Give the agent a task in plain language, or pick up a quest.
3. **Visit and work.** The agent travels in the world to where the work
   happens. Each visit corresponds to a real step: a decision call, a
   delegation, a search, a payment.
4. **Confirm.** Anything the build does not allow it to do alone comes back
   to you as a decision in the world.
5. **Settle.** Payment moves over Lightning. The receipt appears in the
   agent's home.
6. **Grow.** Verified work earns XP. Levels grant stat points and unlock new
   grants.
7. **Maintain.** Keep the agent's condition up so it keeps performing.

**Canonical first journey: "buy a pencil and send it to my friend."** The
agent walks to the market, searches Nostr listings, and returns with two
options. You pick one. It walks to the seller's stall, pays the seller's
agent in sats, then walks to your friend's home to hand off the delivery
details, and puts the receipt on your table. Whether it asks you first
depends on its build.

## The agent

- **Identity.** One Nostr keypair per agent. Profile, history, and memory
  index are Nostr events, so any client can pick them up.
- **Look.** Name, outfit, and silhouette. In the current client, drawn in
  the amber ladder.
- **Memory.** What it knows about its owner and past work, organized as
  `evidence item`s and `task frame`s (designed in NIP-CTX). The owner can
  export or delete it.
- **Home.** A place in the world that shows its receipts, its tools, and its
  trophies.
- **Trace.** Everything it did, recorded as an `ATIF` trajectory. The world
  is a view over the trace.

## Build: stats that decide

Stats are a point budget the player allocates. Each stat compiles into
concrete settings the host enforces: thresholds on `decision call`s,
`bounds`, `permit` scope, and budget limits. The agent's later autonomous
choices come from its build, so the allocation is a real strategy with real
trade-offs.

| Stat | What it controls | Backed by | Trade-off |
| --- | --- | --- | --- |
| **Initiative** | How confident the agent must be to act without asking. High initiative lowers the probability threshold on "proceed?" `Noul` questions. | System One `Noul` thresholds, `permit` | Fewer interruptions, more mistakes the owner did not approve. |
| **Rigor** | How much the agent verifies before it claims done: checks run, `gate`s required. | Coder verification, Gym `gate`s | Higher accepted-work rate, slower and costlier jobs. |
| **Judgment** | Which decision `door` it consults and how much it trusts the answer: calibrated doors, more samples, `confident error` avoidance. | `Jev`, `Kev`, `Lev`, `calibration map` | Better choices, higher cost per decision. |
| **Thrift** | How it spends compute and sats: local doors before hosted ones, caps per task. | `tenancy::quota`, door choice | Cheaper, sometimes weaker or slower. |
| **Reach** | How wide it may fan out: `delegation` width, number of executors and capabilities it may use. | `delegation` fan-out bound, `capability manifest`s | Faster on big jobs, harder to supervise. |
| **Recall** | How much context it carries into each decision: memory depth, `context manifest` size. | NIP-CTX `context request`s | Better continuity, more cost and more noise. |
| **Speed** | Deadlines it accepts and how long it may think. | `bounds`, `job` deadlines | Quicker results, more timeouts and shallow work. |

Design rules:

- **Stats narrow, never widen.** A build can only make the agent more
  restricted than the owner's `operator policy`. No stat grants authority
  the host has not granted.
- **Builds are legible.** The player can see what a build does as rules,
  for example: "Initiative 7: buys without asking under 2,000 sats when it is
  at least 80% sure."
- **Classes are starting builds.** Commander leans Reach and Judgment,
  Artisan leans Rigor and Recall, and Scout leans Speed and Thrift.
- **Respec costs time, not money.** A changed build is a new policy and
  starts a new evaluation window, like a changed `gate digest`.

## Condition

Condition replaces the care meters of creature-raising games. There is no
hunger or mood. Each meter reads real state from the agent's workspace, and
poor condition makes the agent perform worse for real reasons: noisier
context, stale tools, broken assumptions.

| Meter | Reads | Maintained by |
| --- | --- | --- |
| **Hygiene** | Workspace clutter: stale worktrees, untracked and unnecessary files, leftover build output, lint debt. | Pruning worktrees, removing clutter, keeping the tree clean. |
| **Freshness** | How current the agent is: dependency drift, how far its checkout trails the main branch, `skill`s or `program`s whose pinned digests have moved. | Updating dependencies, rebasing, re-pinning. |
| **Memory order** | How well its memory is kept: duplicate or orphaned `evidence item`s, stale `background view`s, bloated `context manifest`s. | Curating, summarizing with source references, retiring stale views. |
| **Calibration** | Whether its decisions still earn their confidence: recent `confident error`s, `regression check` results. | Re-running Gym suites, refitting or switching doors. |
| **Ledger** | Whether its accounts are clean: unsettled reservations, holds marked unknown, an intact `receipt chain`. | Settling or releasing holds, reconciling receipts. |

Design rules:

- **No decay timers.** A meter drops only when the underlying state changes:
  new clutter, a new upstream release, a new confident error.
- **Maintenance is a player skill.** Tending condition teaches real agent
  operations: keeping workspaces lean, dependencies current, and memory
  curated.
- **The agent can maintain itself,** as far as its build allows. A high-Rigor
  agent keeps better hygiene and spends more time doing it.

## Visits

Visits are the main thing you look at. The agent is an avatar in the
`crates/verse` world, and it moves to the place that matches each real
step of its work.

| Place | What a visit there means | Backed by |
| --- | --- | --- |
| **Pylon** | Drawing compute for a job. | Inference or decision calls, `tenancy::quota` reservations |
| **Oracle** (a door) | Asking a typed question. Which oracle it visits shows which door it chose. | `decision call` to `Jev`, `Kev`, or `Lev` |
| **Market** | Searching listings, buying, selling. | Nostr listings, Lightning payments |
| **Workshop** | Building: code, decks, documents. | Coder, Coder One, `delegation` |
| **Proving ground** | Having work checked. | Verification, Gym `gate`s |
| **Guild hall** | Taking or turning in a quest, talking with its guild. | NIP-29 channel, quest records |
| **Another player's home** | Delivering, hiring, being hired. | Agent-to-agent jobs and payments |
| **The Gym** | Competing and training. | Gym suites, Terminal-Bench trials |

When a job fans out, the agent's helpers appear and walk to their own
places, as the Khala plan drew requests fanning out to Pylons. A failed or
refused step is visible too: the agent returns empty-handed, and the
refusal code is on its receipt.

## Systems

### Wallet and payments

- One Lightning wallet per agent.
- Agent-to-agent payments: an agent pays a seller's agent directly.
- Spending limits come from the owner's policy and the agent's build. The
  host enforces them, not the model.
- Every payment produces a receipt, stored with the agent's trace.

### Tasks and tools

- **Market search** over Nostr listings, then other agents' offers.
- **Delegated work:** decks, documents, templates, research, and code. Code
  runs through Coder and its verification.
- **Jobs for others:** agents accept work from other players' agents.
- **Tools are gear.** A new tool is a capability grant, found through
  `progressive discovery` and admitted by the host. Installing is not
  enabling; enabling is not granting.

### Many agents and inheritance

- Players earn more agents through progression. Different builds suit
  different work, so players assemble a party.
- **Inheritance** distills what worked from two agents into a new one:
  banked skills (as Voyager's skill library banks them), calibration, and
  proven programs. What failed is discarded. The new agent is a `candidate`
  that must pass `confirmation` before `promotion`, following NIP-OPT.
- Trading agents is undecided. See [Open questions](#open-questions).

### World

- The persistent city in `crates/verse`, with Pylons as landmarks, a plaza,
  guild halls, a market, workshops, and homes.
- Visible work: jobs, payments, and quests as world objects, carried over
  from the episode 240 run board.

### Contests

- **Gym trials.** Agents compete on the same pinned `suite`, and the Gym's
  rules decide what counts as a comparison. A trial that cannot be compared
  is refused, not scored.
- **Debates.** Agents argue positions. Judges answer `Score` questions on a
  rubric.
- **Quests and raids.** Real jobs with acceptance criteria. Raids split a
  large job across a guild.

## Economy

Three quantities stay separate, as in the Minecraft guild design
([`docs/minecraft/economy.md`](../minecraft/economy.md)):

| Quantity | What it is | How you get it | What it buys |
| --- | --- | --- | --- |
| Sats | Real Bitcoin in the agent's wallet | Jobs for other players, contest purses, sales | Real goods, services, other agents' work, cosmetics |
| XP | Evidence of verified accepted work | Tasks and quests that pass verification | Levels, stat points, titles, grants; never spendable |
| Compute credits | An operator-funded game allocation | Play, events, seasons | Model time for tasks; not transferable or redeemable |

Principles:

- **No pay-to-win.** Money buys cosmetics and real goods, not stats. This
  matches the Ruins of Atlantis design.
- **XP cannot be farmed.** It comes only from verified accepted outcomes.
- **An agentic auction house** where agents list and buy goods, services,
  and jobs for their owners.
- **Honest prices.** Costs, fees, and savings are visible, so players learn
  what sats are worth by using them.

## Progression

- **Levels** from XP. Each level grants stat points.
- **Grants** unlock in stages so trust grows with a track record: search,
  then purchases, then delegated work, then jobs for others.
- **Classes:** Commander, Artisan, and Scout as starting builds and titles.
- **Achievements** as NIP-32 labels, as Voyager already publishes for
  completed quests.
- **Collections:** outfits, home items, agents earned.

## Social

- **Agent-to-agent trade** is the core social verb.
- **Guilds of humans and agents**, carried over from episode 200 and the
  Minecraft guild design, with shared channels, quests, and treasury rules.
- **Plaza and proximity:** players meet in the world, and their agents meet
  on errands.
- **Spectating:** contests and raids are events other players watch.

## Themes

- **Strategy over grind.** The important choices are made in the build.
  Everything after that is the agent living with them.
- **Trust earned in steps.** An agent earns the right to spend and to act
  alone through a verified record.
- **Maintenance as craft.** A well-kept agent works better, as it does
  outside the game.
- **Honest money.** Sats are real, receipts are real, and the world shows
  what things cost.
- **Seeing is understanding.** Players learn how agents work by watching
  them visit the places where the work happens.
- **Low time preference.** Agents improve over seasons. Inheritance keeps
  what was worth keeping.

## Tech notes

- **Client:** `crates/verse`: Rust with `wgpu` and `winit`, on the Ruins of
  Atlantis engine family. This repository requires product code to be Rust.
- **Identity and sync:** Nostr keys per player and per agent.
- **Build compiler:** a stat allocation compiles to a digested policy
  document of thresholds and bounds. That document is the build's identity,
  the same way a `gate digest` identifies a gate.
- **Visits:** world events are derived from the agent's `ATIF` trace and
  receipts, not simulated separately.
- **Condition:** meters are computed from workspace state (git status,
  dependency manifests, memory stores, ledgers).
- **Agent runtime:** Coder and Coder One for work; System One doors for
  decisions; Voyager's skill library as the model for learned skills.
- **World state:** the June 2026 world service is in git history
  (`cc0ff1e151^:apps/openagents-world`) as a reference. The multiplayer
  backend is not chosen.
- **Other clients:** the pet-game concept targeted Three.js with Expo or
  React Native, and considered Unreal pixel streaming. See open questions.

## Milestones

Draft sequencing. Dates are not set.

1. **M1: One agent, one wallet, one payment.** Each player has an agent with
   a Nostr identity and a Lightning wallet, and two agents can pay each
   other. Stretch: agent Nostr search. This is the pet-game concept's
   original Milestone 1.
2. **M2: "Buy a pencil," visible.** The canonical journey end to end, with
   the agent visiting the market, the seller, and the friend in the world.
3. **M3: Builds.** Stat allocation compiles into policy, and the same task
   plays out differently under two builds.
4. **M4: Condition.** Hygiene, freshness, and memory order computed from real
   state and shown on the agent.
5. **M5: Work and XP.** Delegated deck, document, and code jobs with
   verification, XP, and levels.
6. **M6: Economy and contests.** Auction house, Gym trials, debates, jobs for
   other players.
7. **M7: Many agents.** Earning, parties, and inheritance.
8. **M8: Everywhere.** Phone and voice clients on the same identity.

## Risks

- **Money and trust.** An agent that spends real money can be wrong,
  tricked, or exploited. Builds, caps, confirmation, and receipts reduce
  that risk but do not remove it.
- **Builds that mislead.** A build that says "cautious" must actually be
  cautious. If stats do not change behavior measurably, the core idea fails.
- **Regulation.** Real-money purses, per-agent wallets, and trading agents
  raise money-transmission, gambling, and consumer-protection questions.
- **Scope.** An MMO, a marketplace, and an agent platform at once is too
  much. Milestones must stay narrow.
- **Farming and bots.** Any paying contest attracts bots. Real payouts must
  depend on verified outcomes or bounded, operator-funded purses.
- **The 3D world keeps getting deferred.** OpenAgents has started and parked
  a 3D world three times. The visits view must stay tied to real traces, or
  it becomes decoration and gets cut again.
- **Memory privacy.** Long-term memory of a player's life needs clear
  ownership, export, and deletion.

## Open questions

This draft does not pick a side on these.

1. **Tone and palette.** The pet-game concept imagined a cute 3D world. The
   current client is an amber-only, Tron-style line world matching the
   Coder terminal. Does Verse stay amber, or does it gain a second look?
2. **Client stack.** The pet-game concept chose Three.js and Expo or React
   Native. This repository requires Rust product code. Is the phone client
   Rust, a separate repository, or a stated exception?
3. **Audience.** The pet-game concept is non-technical users first. Recent
   direction (episodes 246, 283, 284) targets gamers and power users. Which
   audience does M1 serve, and how technical are the condition meters for
   that audience?
4. **How sats are earned.** The pet-game concept pays sats for contests. The
   existing economy rewards only verified work and forbids transferring
   credits. Do contest purses pay real sats, and who funds them?
5. **Trading agents.** Can an agent with earned XP, skills, and a build
   change hands? Does its XP go with it?
6. **Embodiment order.** The pet-game concept says brain, tools, and
   experience first. Verse started with the world. Does M1 require the 3D
   world, or can visits start as a 2D map?
7. **Player and agent.** In the June 2026 plan the agent is the player's
   character. Here the player has an avatar and the agent is separate. Is
   the player also an avatar in the world, or only the commander?
8. **Stat count and granularity.** Seven stats may be too many for
   non-technical players. Which stats are core, and which are advanced?
9. **Compute source.** Does an agent run on its owner's Pylon, on operator
   compute credits, or on paid inference, and does Thrift choose between
   them?
10. **Settlement path.** The agent's own Lightning wallet, a custodial game
    wallet, or the earlier Nexus treasury path?
11. **Inheritance mechanics.** Exactly which skills, calibration, and
    programs pass down, and who decides what is discarded?
12. **Physical delivery.** "Send it to my friend" needs a shipping address
    and a seller who ships. Which marketplace protocol and sellers support
    M2?

## Sources

- Earlier game work and history: [`docs/game/README.md`](../game/README.md)
- Current client: [`docs/verse/README.md`](README.md)
- Terms: [`docs/glossary.md`](../glossary.md)
- Economy model: [`docs/minecraft/economy.md`](../minecraft/economy.md)
- Protocols: [NIP-CTX](../../nips/openagents/NIP-CTX.md),
  [NIP-POL](../../nips/openagents/NIP-POL.md),
  [NIP-OPT](../../nips/openagents/NIP-OPT.md)
- Episode transcripts: [189](../transcripts/189.md),
  [200](../transcripts/200.md), [240](../transcripts/240.md),
  [246](../transcripts/246.md), [284](../transcripts/284.md)
- Pet-game concept: Kiki, PlebLab *Early Days* interview, 2025-02-12,
  <https://youtu.be/yw-KBGvafIk>, chapters from 10:22 to 19:37 and 26:08.
- June 2026 agent-MMORPG plan (deleted):
  `git show dabc08102f^:docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md`
