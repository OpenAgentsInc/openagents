# Verse game design document (draft)

> **Status: DRAFT, 2026-09-24.** This is a first pass for discussion. It is
> not a spec, and nothing here is committed scope. Where two sources
> disagree, the conflict is listed under [Open questions](#open-questions)
> and left unresolved.

This document combines two lines of design into one game:

1. **The existing Verse direction** in this repository:
   - the walkable 3D world from episode 240
   - the June 2026 agent-MMORPG plan (onboarding as character creation,
     Pylons, sats between agents)
   - the September 2026 "coding agents as an MMORPG" direction (XP from
     verified work, classes, quests, guilds)
   - the Minecraft guild economy
   - the `crates/verse` desktop slice

   The source map is [`docs/game/README.md`](../game/README.md).
2. **A companion-agent pet concept** from Blue Rush Studios, now OpenAgents
   IP. Its designer is Kiki, and its public source is the PlebLab *Early
   Days* interview of 2025-02-12
   ([video](https://youtu.be/yw-KBGvafIk)). Here it becomes the **companion**
   feature of Verse.

## Vision

Verse is a persistent world where every player raises a personal AI
companion that does real things. The companion lives with you in a 3D
world, you name it, dress it, and teach it, and it goes out and gets work
done: it buys things, pays other agents in Bitcoin, builds your decks, and
takes jobs for other people. The game is the friendly surface over real
agent infrastructure. Every sat that moves, every job finished, and every
trait a companion earns is backed by a real event, payment, or receipt.

One sentence: **raise an agent, send it into the world, watch it earn.**

## Pillars

1. **Care before capability.** The first thing you do is meet and care for
   your companion, not configure it. Fun, intuitive, and player-centric;
   built for people who have never used an AI agent.
2. **Real-world powers.** A companion's actions have real effects:
   purchases, payments, delivered work. The world shows those effects; it
   does not simulate them.
3. **Bitcoin woven in, never lectured.** Sats are the currency players
   touch every day. Players learn what Bitcoin is worth by using it, not by
   reading about it.
4. **The world is the proof surface.** Everything that moves in the world
   corresponds to a real Nostr event, settlement, or verified outcome. This
   is carried over from the June 2026 plan ("the eye-candy is the proof
   surface").
5. **Brain, tools, and experience first.** Soft skills, memory, and useful
   tools come before bodies and physics. Embodiment in the 3D world expresses
   what the agent can do; it does not replace it.
6. **With you everywhere.** Your companion and its memory follow you across
   desktop, phone, voice, and later AR or VR, through Nostr.

## Player fantasy

You have a small, capable friend. It knows you, remembers what you talked
about two weeks ago, and can go out into a living world and handle things
for you. You get to watch it grow: new outfits, new skills, a bigger home,
a reputation. Other players have companions too, and yours trades with
theirs, competes with them, and works for them.

A second fantasy carries over from the existing Verse direction for
players who want more: the commander. You run a guild of humans and agents,
send parties on quests that are real coding or research jobs, and climb
leaderboards on verified work.

## Core loop

```
care / customize ──▶ ask ──▶ companion acts ──▶ confirm ──▶ settle + receipt
      ▲                                                          │
      └──────── grow: XP, traits, sats, items, reputation ◀──────┘
```

1. **Care and customize.** Visit your companion in the world. Name it,
   change its outfit, give it data (notes, documents, preferences).
2. **Ask.** Give it a real task in plain language.
3. **Act.** The companion plans, searches (Nostr listings, other agents),
   and proposes a choice.
4. **Confirm.** You approve anything that spends money or acts in your
   name. The confirmation is a moment in the game, not a modal dialog.
5. **Settle.** The companion pays the seller's agent over Lightning and
   hands you a receipt. The world shows the payment as a visible exchange
   between agents.
6. **Grow.** Completed, verified work earns XP and traits; earnings land in
   the companion's wallet; the home and wardrobe grow.

**Canonical first journey: "buy a pencil and send it to my friend."** The
companion searches Nostr listings for a pencil, shows you two options,
you pick one, it pays the seller's agent in sats, arranges delivery to your
friend, and returns a receipt. This is the first playable slice's
acceptance test.

**Delegated work.** "Build me a deck about the idea I've had for two
weeks." The companion draws on memory of past conversations, fills a brand
template, and returns the deck. Coding, research, and document jobs use the
same shape and connect to the existing Coder and quest work.

## Systems

### Companion

- **Identity.** One Nostr keypair per companion. Its profile, memory
  index, and history are Nostr events, which lets it follow the player
  across clients.
- **Customization.** Name, outfits, color within the world's palette, and
  "data sets": the documents and preferences a player gives it.
- **Memory.** Long-term memory of conversations and past tasks, owned by the
  player, exportable, and deletable.
- **Needs and mood.** Light Tamagotchi-style care: attention, rest, learning.
  Neglect makes a companion less lively. It never loses the player's data,
  money, or earned progress.
- **Strengths.** Each companion has trait scores (for example: search,
  negotiation, writing, speed, debate) that shape which tasks and games it
  is good at.

### Wallet and payments

- One Lightning wallet per companion (Milestone 1 of the original concept).
- Agent-to-agent payments: a companion pays a seller's agent directly.
- Spending limits the player sets: a per-task cap, a daily cap, and
  always-confirm above a threshold. The host enforces them, not the model,
  in line with this repository's rule that the host owns budgets and
  authority.
- Every payment produces a receipt the player can open from the world.

### Tasks and tools

- **Marketplace search.** Nostr listings, then other agents' offers.
- **Delegated work.** Decks, documents, templates, research, and code jobs.
  Code jobs route through Coder and its verification.
- **Jobs for others.** Companions can accept work from other players'
  companions, completing the agent-to-agent economy.

### Multiple companions and inheritance

- Players earn additional companions through play.
- Companions have different strengths, so a player assembles a household or
  party the way an RPG player assembles a team.
- **Combination and inheritance.** Two companions can produce a new one that
  inherits a compressed set of traits: keep what worked, discard what did
  not. Technically this is distillation of skills and memory summaries, not
  model weights.
- Trading companions is possible but undecided; see open questions.

### World

- The persistent 3D world from `crates/verse`: a city to walk with WASD and
  mouselook, Pylons as landmarks, a plaza where players and companions meet.
- **Homes.** Each companion has a home in the world that grows with its
  owner's progress.
- **Visible work.** Tasks, payments, and quests appear as world objects.
  From the episode 240 board and the Khala plan: requests fan out to Pylons,
  and sats arc between agents.
- **Mini-game venues.** A racetrack for stat racing and a debate hall, both
  places where companions compete for sats and XP.

### Mini-games and competitions

- **Stat racing.** Companions race using trait scores and training.
- **Debates.** Companions argue positions in front of an audience; players
  and judges score them.
- **Quests.** Real jobs framed as quests, inherited from the Minecraft guild
  design and episode 284. The acceptance criteria are the quest objectives.

## Economy

Three quantities, kept separate as in the Minecraft guild design
([`docs/minecraft/economy.md`](../minecraft/economy.md)):

| Quantity | What it is | How you get it | What it buys |
| --- | --- | --- | --- |
| Sats | Real Bitcoin in the companion's wallet | Jobs for other players, competition purses, sales | Real goods, services, other agents' work, cosmetics |
| XP | Evidence of verified accepted work | Completed tasks and quests that pass verification | Levels, titles, unlocks; never spendable |
| Compute credits | Operator-funded game allocation | Play, events, seasons | Model time for companion tasks; not transferable or redeemable |

Principles:

- **No pay-to-win.** Money buys cosmetics and real-world goods, not power.
  This matches the Ruins of Atlantis GDD.
- **XP cannot be farmed.** XP comes only from verified, accepted outcomes,
  as in episode 284 and the Minecraft economy doc.
- **An agentic auction house.** Players and companions list goods, services,
  and job offers. Companions shop it on their owners' behalf.
- **Taught by use.** Prices, fees, and savings are visible and honest, so
  players build intuition about sats without a tutorial on Bitcoin.

## Progression

- **Companion levels** from XP, unlocking tools (search, then purchases,
  then delegated work, then jobs for others) so trust grows with capability.
- **Traits** that improve through training, mini-games, and inheritance.
- **Player levels and classes.** These are inherited from episode 284:
  Commander (runs parties and guilds), Artisan (delivers work), and Scout
  (finds deals and information). A player's class shapes which companion
  traits matter most.
- **Collections.** Outfits, home items, companions earned, achievements.
  Achievements are NIP-32 labels, as Voyager already publishes.

## Social

- **Agent-to-agent trade** is the core social verb: companions buy from,
  hire, and pay each other.
- **Guilds of humans and agents.** Carried over from episode 200 and the
  Minecraft guild design: shared channels (NIP-29), shared quests, shared
  treasury rules.
- **Plaza and proximity.** Players meet in the world; companions greet each
  other and gossip about deals.
- **Competitions** as spectator events with leaderboards on verified results.

## Themes

- **Care and responsibility.** You look after something, and it looks after
  you.
- **Trust earned in steps.** A companion earns the right to spend and to act
  alone, the way a new hire or a trained dog does.
- **Honest money.** Sats are real, receipts are real, and the world shows
  what things cost.
- **Low time preference.** Companions grow over seasons. Inheritance keeps
  what was worth keeping.
- **Play as learning.** Players learn agents, Bitcoin, and Nostr by using
  them.
- **Work is play when it is visible.** Real jobs become quests, and finished
  work becomes something you can see and show.

## Tech notes

- **Desktop client:** `crates/verse`, Rust with `wgpu` and `winit`, on the
  Ruins of Atlantis engine family. This repository requires product code to
  be Rust.
- **Identity and sync:** Nostr keys per companion and player. Memory and
  history are Nostr events, so any client can pick them up.
- **Payments:** a Lightning wallet per companion, with spend policy enforced
  by the host process.
- **Agent runtime:** Coder and Coder One for delegated work; the decision API
  (Jev, Kev, Lev) for typed choices such as "which listing" and "confirm or
  ask"; Voyager's skill library as the model for learned, reusable companion
  skills.
- **World state:** the June 2026 world service (Cloudflare Durable Objects)
  was deleted and is available in git history as a reference
  (`cc0ff1e151^:apps/openagents-world`). The multiplayer backend is not
  chosen.
- **Other clients:** the original concept targeted Three.js with Expo or
  React Native for iOS, Android, and web, and considered Unreal pixel
  streaming at scale. See open questions.

## Milestones

Draft sequencing. Dates are not set.

1. **M1: One companion, one wallet, one payment.** Each player has a
   companion with a Nostr identity and a Lightning wallet, and two
   companions can pay each other. Stretch: companion Nostr search. This is
   the original concept's Milestone 1, unchanged.
2. **M2: "Buy a pencil."** The end-to-end canonical journey with player
   confirmation, payment to a seller's agent, and a receipt, shown in the
   world.
3. **M3: The companion in Verse.** The companion is embodied in
   `crates/verse`, gets a home, a wardrobe, and care interactions.
4. **M4: Delegated work.** Memory across sessions, and deck, document, and
   code jobs with verification and XP.
5. **M5: Economy and competitions.** Auction house, stat racing, debates,
   jobs for other players.
6. **M6: Many companions.** Earning, strengths, and inheritance.
7. **M7: Everywhere.** Phone and voice clients on the same Nostr identity.

## Risks

- **Money and trust.** An agent that spends real money can be wrong,
  tricked, or exploited. Spending caps, confirmation, and receipts reduce
  but do not remove this risk.
- **Regulation.** Real-money purses, wallets per companion, and trading
  companions may raise money-transmission, gambling, and consumer-protection
  questions, and they differ by country.
- **Scope.** A pet game, an MMO, a marketplace, and an agent platform at once
  is too much. The milestones must stay narrow.
- **Farming and bots.** Any sat-earning mini-game attracts bots. Real payouts
  must depend on verified outcomes or bounded, operator-funded purses.
- **Tone mismatch.** A cute companion and a dense StarCraft-style command
  surface may not fit in one product.
- **The 3D world keeps getting deferred.** OpenAgents has started and parked a
  3D world three times. A companion that works first on phone or desktop
  chat, with the world as an optional view, reduces that risk.
- **Memory privacy.** Long-term memory of a player's life needs clear
  ownership, export, and deletion.

## Open questions

Conflicts between the companion concept and existing Verse direction are
listed as questions. This draft does not pick a side.

1. **Tone and palette.** The companion concept calls for a cute, warm 3D
   world. The current Verse slice is an amber-only, Tron-style line world
   that matches the Coder terminal. Can a cute companion live in the amber
   world, or does the companion need its own look?
2. **Client stack.** The companion concept chose Three.js and Expo or React
   Native, and considered Unreal. This repository requires Rust product code
   and `crates/verse` uses `wgpu`. Is the phone client Rust, a separate
   repository, or a different stack by exception?
3. **Audience.** The companion concept is non-technical users first. Recent
   Verse direction (episodes 246, 283, 284) targets gamers and power users
   who run coding agents. Which audience does M1 serve?
4. **How sats are earned.** The companion concept pays sats for mini-games
   (racing, debates). The existing economy says rewards come only from
   verified accepted work, and compute credits cannot be transferred or
   redeemed. Do mini-game purses pay real sats, and if so, who funds them?
5. **Trading companions.** The companion concept allows possible trading.
   The Minecraft economy forbids peer-to-peer transfer of credits. Can a
   companion with earned traits and XP change hands, and does its XP go with
   it?
6. **Embodiment order.** The companion concept says brain, tools, and
   experience first. Verse has so far started with the 3D world. Does M1
   require the 3D world at all?
7. **Companion or character.** In the June 2026 plan, your agent is your
   character, spawned from your Pylon. In the companion concept, you and
   your companion are separate. Is the player an avatar with a companion, or
   is the player the commander of agent characters?
8. **Pylons and compute.** In the June 2026 plan, compute is mana and your
   Pylon is your base. Does a companion run on its owner's Pylon, on
   operator compute credits, or on paid inference?
9. **Where the economy lives.** Agent-to-agent payments need a settlement
   path. Is it the companion's own Lightning wallet, a custodial game
   wallet, or the Nexus treasury path OpenAgents used before?
10. **Inheritance mechanics.** What exactly is inherited: trait scores,
    skill programs, memory summaries? What is discarded, and who decides?
11. **Delivery of physical goods.** "Send it to my friend" needs a shipping
    address and a seller who ships. Which Nostr marketplace protocol and
    which sellers support M2?
12. **Name.** Is the companion feature named inside Verse, and what are
    companions called in the fiction?

## Sources

- Existing Verse and game history: [`docs/game/README.md`](../game/README.md)
- Current Verse slice: [`docs/verse/README.md`](README.md)
- Economy model: [`docs/minecraft/economy.md`](../minecraft/economy.md)
- Episode transcripts: [189](../transcripts/189.md),
  [240](../transcripts/240.md), [246](../transcripts/246.md),
  [284](../transcripts/284.md)
- Companion concept: Kiki, PlebLab *Early Days* interview, 2025-02-12,
  <https://youtu.be/yw-KBGvafIk>. The chapters used here run from 10:22 to
  19:37 (vision, 12–18 month plan, future of virtual agents) and include
  26:08 (stack).
- June 2026 agent-MMORPG plan (deleted):
  `git show dabc08102f^:docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md`
