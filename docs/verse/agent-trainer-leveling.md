# Agent trainer leveling

> **Status: Phase 1 implemented 2026-09-28; the milestone waits on the first
> accepted reproduction.** This document specifies a progression system for
> people who train agents. Phase 1 ([#9847](https://github.com/OpenAgentsInc/openagents/issues/9847))
> added NIP-XP's `reproduce` rule, six published
> [tutorial quests](tutorial-quests.md), levels over heads in the Grid, and
> the Account trainer card. It builds on
> what exists: [NIP-XP](../../nips/openagents/NIP-XP.md), Verse's XP reader
> (`crates/verse/src/xp.rs`), the Terminal-Bench 4 quest board, and the Gym.
> Everything under [Design](#design) that is not listed as implemented in the
> [inventory](#what-exists-today) is proposed. Nothing in this document pays
> anyone, and no payout is promised. Proposed changes to NIP-XP are listed in
> [Proposed NIP-XP changes](#proposed-nip-xp-changes); of those, the
> `reproduce` rule and the `per-awardee` uniqueness policy are in the NIP
> so far.

Everyone starts at level 1. You level up as an *agent trainer*: a person
whose accepted work makes agents measurably better. You earn XP by getting
traces accepted, writing knowledge that turns failing tasks into verified
passes, reproducing other people's benchmark attempts, clearing Gym
challenges, and running raids with a party on tasks nobody passes alone. XP
comes only from signed, public evidence, so anyone can recompute your level
from a relay and get the same number.

## Contents

- [Motivation](#motivation)
- [What exists today](#what-exists-today)
- [Design](#design)
- [Proposed NIP-XP changes](#proposed-nip-xp-changes)
- [Phased rollout](#phased-rollout)
- [Open questions](#open-questions)
- [Our thoughts](#our-thoughts)
- [Related documents](#related-documents)

## Motivation

### The exchange

A post on X argued that raiding is job training:

> A kid who knows how to raid in WoW is unironically a better hire than 99%
> of non gamers. They develop team skills, coordination, leadership, how to
> master the respect of 40 people, how to share loot, maintaining a raid
> schedule, showing up on time, clear direct communication, how to get along
> with unique personalities, creating and executing on strategies, failing
> and trying again (perseverance), delayed gratification, etc. Invaluable
> skills are developed during raiding that are transferrable to the real
> world.

A reply pushed back:

> this is a pretty common take on ct especially, like I'm one of those
> people, I guess, who learned stuff from computer games, but as much as 'I
> want to believe', ultimately it's cope. No, the kid who did raids in WoW or
> grinded eve online or Runescape or whatever else is not a great default
> hire because of his 'skills'. The key component you are missing is him
> being interested/passionate in whatever it is you want him to work on,
> cause if he is not, he is just going to half ass his job and come home to
> play WoW... Same for chess grandmasters who frequently make mediocre quant
> hires. Intelligence is just not directly transferrable and genuine
> interest (or even better passion) beats intelligence/gaming acquired
> 'skills' 99% of the time

Chris (@OpenAgentsInc) answered:

> I agree with the original take and this qualification. Consider that
> experiences which could deserve the attention of such gamers don't exist
> yet. Wagie jobs certainly ain't it. There was a point in a now-deleted
> Fevir video from years back ('WoW Sucks Now' or similar) that's stuck in
> my head: that the future of MMOs may not look like an MMO and may not even
> be properly called a game: something might instead blur the lines between
> reality and game, something there's not yet a name for. An example he
> gave: imagine an evolved version of the Pokémon Go training system, where
> players could earn real-world rewards and credentials as trainers, even
> tying into real-world commerce: an approximation of becoming a Pokémon
> trainer 'in real life'. I have a strong feeling that something big will
> arise at the intersection of games and real world, probably involving
> agentic AI, and I'm going to take a crack at this with @OpenAgentsInc (If
> you've read FreedomTM you can picture what I'm talking about)

And, on the product:

> we want to make agent trainer a real thing, u can level up, uploading
> traces, doing stuff etc

### What we take from it

Both sides are right, and the design has to satisfy both:

- **The original post** names skills that raiding does build: roles,
  schedules, shared strategy, splitting loot fairly, and wiping and trying
  again. A progression system can ask for exactly those skills, on work
  that matters.
- **The reply** names the failure mode: skills don't transfer to work the
  player doesn't care about. So the work itself has to be the game. Agent
  training qualifies: it is new, it is measurable, the feedback loop is
  fast, and there is a real frontier to beat (a public reference run with a
  price and a time).
- **The owner's answer** sets the bar: a trainer level has to mean something
  outside the game. It must be a credential an employer, a collaborator, or
  a buyer can check without trusting us.

Episode 284 ([transcript](../transcripts/284.md)) is the earlier version of
the same idea: XP for coding agents, public leaderboards, "verified work is
the base currency," operator classes, daily quests and raid encounters, and
the warning that XP must never be farmable "via spammy" activity. This
document turns that brainstorm into a specification that fits the contracts
the repository has since built.

### Why this can work now

The repository already has the pieces a game needs and most games fake:

- **Objectives with a real bar.** The [TB4 quest board](../terminal-bench/quest-board.md)
  posts 11 frozen quests, each a real task with Fable 5.1 low's cheapest
  winning run as the price to beat.
- **A grader, not a model, decides.** A pass is what the benchmark's
  verifier accepts, and every run leaves a record with steps, cost, time,
  and outcome.
- **Proof that one contribution matters.** One shared, cited fact turned
  three failing TB4 tasks into cheap passes: `gsea-proteomics` went from 0
  of 10 to 4 of 4, `fin-saccr-rwa` from 0 of 6 to 4 of 4
  ([essay](../coder/cheapest-verified-passes.md)). That result is labeled
  in-sample, and transfer to unseen tasks is not yet shown, but it is
  exactly the kind of contribution a trainer makes.
- **Signed, recomputable XP.** NIP-XP awards are checked from signed events
  by every reader; there is no central score to hack.

## What exists today

Status words follow the [glossary](../glossary.md): **implemented** means
code in this repository does it and has tests; **specified** means a
document or NIP defines it and no code does it yet.

### Protocol and ledger

| Piece | Where | Status |
| --- | --- | --- |
| Quests (`30193`), awards (`3193`), revocations (`3194`), achievement labels (NIP-32 `1985`, `L=openagents.xp`) | [NIP-XP](../../nips/openagents/NIP-XP.md) | Implemented: `crates/nostr/src/xp*` validates the events. |
| Acceptance rules `kb-transfer` (roles `author`, `runner`) and `reproduce` (roles `claimant`, `reproducer`; run evidence marked `oa:xp:run:v1`) | NIP-XP | Implemented; `reproduce` added 2026-09-28. Readers refuse unknown rules. |
| Uniqueness policies `first` (one award per quest version) and `per-awardee` (each distinct reproducer once, up to a required `max_awards`) | NIP-XP | Implemented; `per-awardee` added 2026-09-28 ([#9894](https://github.com/OpenAgentsInc/openagents/issues/9894)) for `reproduce` quests, keyed `<coordinate>:<reproducer>`. |
| Ledger derivation under a reader's trust list (referees, optional runners) | `crates/xp-ledger`, `xp_ledger::derive` (re-exported as `knowledge::xp`) | Implemented. Split from `knowledge` on 2026-09-28 so the phone doesn't link the knowledge base's model clients ([#9897](https://github.com/OpenAgentsInc/openagents/issues/9897)). |
| Referee tool: `microcoder xp quest`, `award`, `revoke`, `ledger`, and the trainer's `claim` and `reproduce` | `crates/microcoder/src/xpnet.rs`, [XP guide](../coder/guides/xp.md) | Implemented. |
| Reader commands: `openagents quests`, `xp`, and `board` | `crates/openagents-cli/src/quest.rs` | Implemented, read-only. |
| OpenAgents referee key `npub1v59z5gk…rusf6k` and 11 TB4 quests, season `tb4-s1` (2026-09-26 to 2026-12-25), 10 XP each (author 6, runner 4) | [quest board](../terminal-bench/quest-board.md), `knowledge/quests/` | Published. Every quest was open when the board was generated; no award is recorded. |
| Evidence that can complete a `kb-transfer` quest | NIP-XP, [study guide](../coder/runtime/knowledge-studies.md) | **Blocked.** `kb publish-evidence` produces historical screening with an `inconclusive` verdict, which can't complete a quest. A separately verified prospective `pass` is required, and no producer makes one yet. |
| Knowledge entries (`3190`, head `30190`, withdrawal `3191`) and evidence (`3189`) | [NIP-KB](../../nips/openagents/NIP-KB.md), [NIP-EVAL](../../nips/openagents/NIP-EVAL.md) | Implemented. `relay.openagents.com` holds 154 entries, all written by one key. |

### Levels, titles, and display

| Piece | Where | Status |
| --- | --- | --- |
| Level curve `trainer-curve-v1`: level 1 at 0 XP; level n + 1 at `ceil(100 · n^1.5)` cumulative XP | `crates/verse/src/xp.rs` (`CURVE`, `xp_to_reach`, `level_of`) | Implemented and named in desktop Verse, `openagents xp`, and the mobile card. |
| HUD strip: XP, level, XP to next level, titles, trusted referees | `xp::strip` | Implemented, desktop. |
| Quest board on the plaza, 22 m west of center, `B` to open | `xp::board_lines`, [Verse README](README.md#quests-and-xp) | Implemented, desktop. Read-only; Verse never publishes XP events. |
| Name tags with `lv n` for players whose Verse key has XP | `crates/verse/src/app.rs` (`xp::level_tag`) | Implemented, desktop. |
| Titles from achievement labels, shown only while the award counts and only when the award's referee signed the label | `xp::snapshot` | Implemented, desktop. |
| Grid name tags: the first eight hex characters of each player's pubkey, then ` · lv n` when the key has XP under the OpenAgents referee | `crates/coder-mobile/src/verse_app.rs` (`player_tags`, `verse::xp::name_tag`), wrapped by `crates/openagents-mobile/src/verse.rs` | Implemented on iOS 2026-09-28; the mobile build links Verse's read-only XP reader, which since #9897 pulls in `crates/xp-ledger` rather than `knowledge`. The Android library builds with it under NDK 27.1. |
| Account trainer card: level, XP, XP to next level, curve, titles, counted awards with links, and the trainer key (the Verse world key) with an explicit reveal | `crates/openagents-mobile/src/trainer.rs`, `bins/openagents-ios/host/App/AccountScreens.swift` | Implemented on iOS 2026-09-28. |
| Six tutorial `reproduce` quests, 50 XP each, season `tb21-tutorial-s1` | [tutorial quests](tutorial-quests.md), `knowledge/quests/tb21.*.reproduce@1.json` | Published 2026-09-28. No award yet. |
| Classes Commander, Artisan, and Scout; stat points per level; grants unlocked in stages | [GDD](gdd.md#progression) | Specified for *agents*, not trainers. Draft, not committed scope. |
| Guild XP with contributor attribution that reconciles to one fixed award | [Minecraft economy](../minecraft/economy.md#xp-and-winning) | Specified. |
| Voyager quest XP and `openagents.voyager/quest-complete` labels in the Minecraft arena | `crates/voyager/src/quest.rs`, `ensemble.rs`, `ledger.rs` | Implemented, but a separate ledger that isn't NIP-XP. |
| Stat points | [GDD](gdd.md#progression) | Specified only. No code grants or spends them. |

### Gym, traces, and evidence

| Piece | Where | Status |
| --- | --- | --- |
| Gym: pinned suites, receipt-chained result store, digested acceptance gates | `crates/gym`, [Gym docs](../gym/README.md) | Implemented for decision models and Terminal-Bench records. |
| Verse Gym building: boards for Microcoder runs, Terminal-Bench trials, and training summaries, over an encrypted, separately granted connection; confirmed launch recipes | `crates/verse/src/gym.rs`, `crates/verse/src/hud/gym.rs`, `crates/gym-bridge`, [Gym building](gym.md) | Implemented on desktop and iOS. The boards are observations, "not an independently verified leaderboard." Being ported into the OpenAgents app's Grid now. |
| Gym leaderboard with loadable traces | [`docs/verse/gym-leaderboard.md`](gym-leaderboard.md) | Specified, with its generator (`crates/gym-leaderboard`) and published boards in `bench/terminal-bench/published/`. This document defers board layout and trace loading to it. The nearest existing material is the Gym CLI's [beat-the-winner view](../gym/terminal-bench-cli.md) and the external [TB4 leaderboard notes](../terminal-bench/tb4-leaderboard.md). |
| Run replays with a Fable ghost | [Verse README](README.md#run-replays) | Implemented, desktop. |
| ATIF-v1.7 trajectories: every Coder conversation records itself locally | `crates/atif`, [traces](../coder/runtime/traces.md) | Implemented. **There is no upload, no service, no trace endpoint, and no redaction.** Traces are local files. The earlier `gym upload` path is retired. |
| Retained-history observation for paired devices | [NIP-SESS](../../nips/openagents/NIP-SESS.md), `crates/coder-connect` | Designed; only the read-only observer profile is implemented. Encrypted to the owner's paired devices; not a public submission path. |
| Jev: typed judgments over state, through `POST /v1/systemone` | `crates/jev`, `crates/coder-one` | Implemented. Used for routing and for checks inside runs; it is a model's judgment, not a grader. |

### Money

| Piece | Where | Status |
| --- | --- | --- |
| Lightning wallet on an exclusively held node key; x402 codecs, replay store, and facilitator | `crates/wallet`, `crates/x402`, [NIP-X402](../../nips/openagents/NIP-X402.md) | Wallet and x402 mechanics exist; NIP-X402 is Designed, and `crates/x402` says "Nothing here pays." No paid round trip has been recorded. |
| Phone wallet | `crates/openagents-mobile/src/wallet.rs` | Implemented on mainnet through Breez's Spark SDK ([#9854](https://github.com/OpenAgentsInc/openagents/issues/9854)); agent spending is not built yet. |
| Quest purses: sats for an accepted completion, through NIP-MKT, NIP-LAB, and NIP-X402 (migration package M18) | NIP-XP, [NIP-MKT](../../nips/openagents/NIP-MKT.md), [NIP-LAB](../../nips/openagents/NIP-LAB.md), [beat Fable together](../coder/beat-fable-together.md) | Specified. `crates/coder-labor` is free-only ("acceptance does not perform a payment"), and M18 adds settlement only after free fulfillment works. |
| Paying knowledge authors or trainers | — | **Nothing pays an author or a trainer today.** |

## Design

### Principles

1. **Level 1 for everyone.** A new key is a level 1 trainer with 0 XP. There
   is no signup bonus, no purchase, and no way to start higher.
2. **XP only from verified, accepted evidence.** Never from activity: not
   tokens, runs, commits, time online, messages, or uploads by themselves.
3. **Anyone can recompute it.** A level is a pure function of public signed
   events, a trust list, and a named curve. Two readers with the same inputs
   get the same level.
4. **XP is a record, not a balance.** It can't be spent, transferred, sold,
   or converted, as NIP-XP requires. A level unlocks no authority.
5. **Honest labels travel with the number.** A credential says which
   referees it trusts, which curve it uses, and which awards it counts.

### Trainers and agents

A *trainer* is a person, identified by one or more Nostr keys. An *agent* is
what they train. The GDD's leveling of agents (builds, stat points, classes
Commander, Artisan, and Scout) is a separate, later system.

This document levels the trainer. XP lands on the keys named as awardees in
NIP-XP awards: the key that signed a knowledge entry, a piece of evidence, a
trace submission, or a party charter. An agent that signs its own work with
its own key earns nothing on its own; its trainer claims the work by
[linking the key](#one-trainer-many-keys).

### One trainer, many keys

Verse already sums XP across your keys: the Verse profile key, the knowledge
key, and any `--xp-key`. That is fine for your own HUD, but it's unverifiable
for anyone else: nothing stops a client from claiming a stranger's key.

Proposed: a **key link** is valid only when both keys sign it. A trainer
publishes a trainer profile that lists linked keys, and each linked key
publishes a matching link back. Readers sum XP only across mutually linked
keys. See [Proposed NIP-XP changes](#proposed-nip-xp-changes).

### The curve

Keep the curve Verse ships, and name it `trainer-curve-v1`: level 1 at 0 XP,
level n + 1 at `ceil(100 · n^1.5)` cumulative XP.

| Level | Cumulative XP |
| --- | --- |
| 2 | 100 |
| 3 | 283 |
| 5 | 800 |
| 10 | 2,700 |
| 20 | 8,282 |
| 40 | 24,356 |
| 60 | 45,319 |

The curve is a client reading, not protocol, so changing it breaks no event.
Any display of a level names its curve, so two clients never show different
numbers under one name. A new curve gets a new name.

There is no level cap. Seasons end quests, not levels: XP from a closed
season still counts, because the evidence is still valid.

### Award sizes

The curve is fixed; what varies is how much each quest awards. Today's TB4
quests award 10 XP, so level 2 would take ten accepted quests. That is too
slow for a first session and puts the whole curve out of reach. Proposed
tiers, all within NIP-XP's 1 to 1,000 cap per quest version:

| Tier | Example | Award |
| --- | --- | --- |
| Tutorial | First accepted trace; first reproduction of a published pass | 25 to 50 XP, once per trainer |
| Daily | Reproduce today's featured pass; submit a graded trace on today's task | 10 to 20 XP |
| Weekly | Clear a Gym challenge under a published bar | 50 to 100 XP |
| Frontier quest | Beat a frontier reference run on a held-out task with knowledge you wrote | 100 to 300 XP |
| Raid | A task no solo attempt has passed, cleared by a party | 500 to 1,000 XP, split |

A tutorial takes a new trainer to level 2 in one sitting. Level 10 takes a
season of steady, verified work. Level 40 takes raids.

### What earns XP

Each row is a quest *rule*: a check the referee runs before signing and every
reader runs before counting. NIP-XP implements only `kb-transfer`; the rest
are proposed.

| Contribution | Rule | Roles | What must be true |
| --- | --- | --- | --- |
| Knowledge that measurably helps | `kb-transfer` (exists) | author, runner | The entry wasn't written from the quest's task; paired runs by someone other than the author pass at the bar and under its cost. |
| A trace accepted into a corpus | `trace-admit` (proposed) | trainer | An ATIF trace of a graded run on the quest's task, whose digest, harness, and grader outcome the referee re-derives, passes redaction and schema checks and is admitted to the quest's named corpus. |
| A benchmark attempt reproduced | `reproduce` (proposed) | claimant, reproducer | A second key reruns a published attempt from its recipe (same task, pinned harness and model identity) and the grader accepts it. The reproducer is never the claimant. |
| A Gym challenge cleared | `gym-trial` (proposed) | trainer, runner | A pinned suite run, recorded in the Gym's receipt-chained store, passes the challenge's digested gate and beats its bar. |
| A raid cleared | `raid` (proposed) | party roles from a charter | The party's charter was signed by every member before the attempt; the completion passes the underlying rule; an outside key verified it. |
| Teaching | `mentor` share (proposed) | mentor | The mentee named the mentor in the submission before acceptance. The mentor's share comes out of the mentee's fixed award; it never adds to it. |

What never earns XP: publishing an entry nobody measured, a trace nobody
graded, a run the grader failed, a model's claim that it finished, a Jev
judgment by itself, stars, follows, reactions, download counts, and mutual
labels.

### Verification and anti-farming

The existing NIP-XP defenses carry over unchanged: fixed award per quest
version, one live award per uniqueness key, the author and the runner must
differ, in-sample evidence is refused, whole arms are counted (no cherry
picking), frozen quests, irreversible signed revocations, and per-reader
trust in referees and runners. On top of those:

- **Independent verification is a role, not a courtesy.** Every rule has at
  least two keys: the one who did the work and the one who checked it. For
  `reproduce` and `trace-admit`, the referee reruns or re-grades from the
  recipe on its own execution host and cites its own run record.
- **Held-out by default.** A frontier quest names tasks outside the
  trainer's written-from provenance and, where a pre-registered study
  exists, outside its tuning set.
- **Repeatable quests are small and bounded.** Dailies use the proposed
  `per-awardee` policy with a stated maximum number of awards, so one Sybil
  farm can take at most the daily's small award per key, and each key still
  needs a verified run that costs real compute.
- **Sybil resistance is a trust decision.** Keys can't be tied to people.
  A reader or a board can list trusted runners, require a minimum account
  age, or require that a trainer's key link to a key with prior accepted
  work. The protocol reports; the reader decides.
- **No negative XP.** A failed attempt, a refusal, or an outage costs nothing
  but the attempt. Wipes are recorded, not punished, so trainers try hard
  tasks.
- **Revocation is public.** A revoked award takes its XP, titles, and any
  credential citing it with it, and the reason stays readable.

**Deterministic recomputation.** A trainer's level is
`level_of(sum of counted awards over linked keys)`, where "counted" is
exactly the NIP-XP ledger derivation under a named trust list. The inputs
are public: the relay set, the trust list (a list of referee and runner
keys), and the curve name. A referee MAY publish a signed ledger snapshot as
a cache, but a reader never trusts a snapshot it can't re-derive.

### Classes and titles

Trainers don't pick a class. A class is a title computed from where a
trainer's counted XP came from in the current season: the rule that
contributed the largest share. It gates nothing.

| Class | Largest XP share from |
| --- | --- |
| Scholar | `kb-transfer` authorship |
| Warden | `reproduce` and `kb-transfer` runner roles |
| Pathfinder | `trace-admit` |
| Champion | `gym-trial` |
| Raid leader | the `lead` role in `raid` charters |
| Mentor | `mentor` shares |

Titles stay NIP-32 achievement labels that point at awards, signed by the
award's referee, as NIP-XP specifies: for example, `first-transfer`,
`beat-reference`, `first-reproduction`, `first-clear` (first party to clear a
raid quest version), and `held-out` (an award on a pre-registered held-out
task). A title disappears when its award stops counting.

The GDD's Commander, Artisan, and Scout remain agent builds, not trainer
classes.

### Raids, parties, and guilds

A raid is a group attempt on a task no solo attempt has passed, such as the
held-out TB4 tasks where Microcoder has passed 0 of 24. It asks for the
skills the original post lists, on real work:

| Raiding skill | How a raid asks for it |
| --- | --- |
| Roles | A charter assigns `lead`, `author`, `runner`, and `analyst` roles. The `verifier` must be outside the party. |
| Raid schedule and showing up | The charter names a pull window. Paired runs and reproductions have to land inside it, and the season closes the quest. |
| Loot rules | The charter fixes each role's share of the quest's award before the first attempt. Every member signs it. Nobody renegotiates after the kill. |
| Strategy | The lead publishes the plan (which entries, which harness, which budget) in the party's channel before the pull. |
| Wiping and trying again | Every attempt is recorded. Failures cost no XP and stay visible, so a clear shows how many pulls it took. |
| Clear communication | Party and guild channels are NIP-29 groups, as in [Verse chat](chat.md). |
| One kill per lockout | `completions: first`: the first accepted clear of a quest version takes the award. A new version is a new raid. |

A **guild** is a NIP-29 group with a charter. Guild XP is the sum of its
members' counted awards earned under guild charters, counted once per award,
matching the reconciliation rule in the
[Minecraft economy](../minecraft/economy.md#xp-and-winning). Guild boards rank
guilds under the viewer's trust list.

### Credentials

A **trainer card** is the credential: a portable, signed, evidence-linked
summary that anyone can verify without asking OpenAgents.

- It holds the trainer's linked keys, the curve name, the trust list it was
  derived under, the level, and the list of counted award event IDs.
- Each award links to its quest, its entry or trace, its evidence, and, for
  Terminal-Bench, the run records behind the evidence.
- The trainer signs the card with their primary key. The signature proves
  they published it, not that it's right: `openagents xp verify-card`
  (proposed) re-derives the level from the relays and reports any
  difference.
- It exports as a JSON file and as a link to a public page.

The level is the summary line; the awards are the credential. "Level 14
trainer" means little on its own. "Wrote the knowledge entry that made a
cheap agent pass `fin-saccr-rwa` on a held-out run, reproduced by two other
keys" is a sentence an employer can check.

### Rewards

Sats are future, staged, and separate from XP, as NIP-XP requires.

**What exists:** nothing pays a trainer or an author. The wallet and x402
code landed, but no paid round trip has been recorded, and the phone wallet
has no agent spending path yet.

**What must be true before any quest pays sats:**

1. An award can actually be granted: a producer of admissible prospective
   evidence (or the proposed `reproduce` and `gym-trial` rules) exists, and
   the OpenAgents referee has signed at least one award to a key it doesn't
   control.
2. A paid round trip over NIP-X402 is recorded end to end, with a receipt.
3. Quest purses are specified in NIP-MKT or NIP-LAB terms: who funds the
   purse, its budget, its terms, and what happens on revocation.
4. Sybil controls for paid quests are stronger than for XP: trusted-runner
   lists, reproduction by more than one outside key, and caps.
5. Legal review of money transmission, gambling, and consumer protection,
   as the [GDD risks](gdd.md#risks) note.

**Rewards that aren't money:** titles, cosmetics in Verse (a tag color, a
trainer banner in the Gym), a place on boards, and eligibility to take
harder quests and raids, as the Minecraft economy already allows.
Eligibility to *take* a quest is not authority; it never widens tool access,
filesystem scope, or a spending cap.

### The Gym is the training hall

The Gym building is where trainers go to train. Board layout and trace
loading belong to the [Gym leaderboard spec](gym-leaderboard.md); this
section fixes what leveling adds to it.

- **Leaderboard boards.** The Gym leaderboard's boards rank attempts. Each
  row names its trainer by pubkey prefix and level, and links to the
  award that counted it, if any. An unawarded row is labeled an observation.
- **Trace viewer.** Selecting a row loads its trace as a replay beside the
  reference ghost, as run replays already do. A trace admitted under
  `trace-admit` shows its award.
- **Quest board.** The plaza board stays the public notice for frontier
  quests. The Gym gets a **challenge board** for dailies and weeklies.
- **Daily and weekly challenges.** The referee publishes each as a quest
  version whose season is one day or one week, generated from benchmark
  tasks with a known reference run. A daily is a reproduction or a trace
  submission; a weekly is a Gym trial under a bar.
- **Trainer rank board.** Levels of trainers under the viewer's trust list,
  limited to keys that published a trainer profile (see
  [Privacy](#privacy)).
- **Levels over heads.** In the Grid, the name tag becomes the pubkey prefix
  and the level, such as `650a2a22 · lv 3`, matching desktop Verse's `lv n`.
  A key with no XP shows its prefix alone. This needs a read-only XP reader
  in the mobile build, which disables it today.

### In the OpenAgents app and on the web

| Surface | What it shows |
| --- | --- |
| **Verse** tab (the Grid) | Levels on name tags; the Gym's boards, challenge board, and rank board; the plaza quest board. |
| **Account** tab | Your trainer card: level, XP to next level, class, titles, counted awards with links, linked keys, trust list, and **Export card**. **Link a key** starts the two-sided key link. |
| **Coder** tab | After a graded pass, a note that says which open quest the run matches and what would make it count, such as "needs an independent reproduction." It never publishes without an explicit tap. |
| Web | A read-only public trainer page per key, rendered from relay events with its curve and trust list shown. Aspirational: no web surface in this repository serves it yet. |

### Privacy

- **XP is public by construction.** Awards are signed Nostr events. Anyone
  can compute any key's ledger. Say so wherever a trainer publishes.
- **Keys are pseudonyms.** A trainer can train under a key tied to no other
  identity. Linking keys is opt-in and visible.
- **Boards are opt-in.** Rank boards and name-tag levels show only keys that
  published a trainer profile. The ledger stays computable for everyone, but
  the app doesn't advertise a key that didn't ask.
- **Traces must be redacted before submission** and submitted only by an
  explicit action. Coder records traces locally today, with no redaction
  step, and uploads nothing. `trace-admit` needs a redaction pass first and
  must keep upload off by default. An award cites digests and public
  evidence, never private workspace content.
- **Private Gym boards stay private.** Observations over a Gym grant are
  encrypted to the grantee and never become public evidence by themselves.

## Proposed NIP-XP changes

Each is a new, versioned addition; none changes an existing event's meaning.

1. **New rules** `reproduce`, `gym-trial`, `trace-admit`, and `raid`, each
   with its roles, the exact evidence events it names, and fixtures, as
   NIP-XP already requires of "a future rule."
2. **A `per-awardee` uniqueness policy** with a required `max_awards`, so a
   daily can award each distinct key once, up to a stated total.
   *Implemented 2026-09-28 for `reproduce`, whose keyed role is the
   reproducer; the claimant's share must be 0.*
3. **More than two awardees**, in the order a rule defines, with the fixed
   split still summing to the quest's award.
4. **Party charters:** a signed event listing members, roles, and shares,
   signed or countersigned by every member before the attempt, and named by
   the award.
5. **A `mentor` role** whose share comes out of the fixed award.
6. **Key links:** a two-sided link between a trainer's keys, so readers can
   sum XP across keys a trainer proves they control.

Levels, curves, and classes stay out of the NIP. They are this document's
reading of the ledger.

## Phased rollout

### Phase 0: What exists

NIP-XP events, the ledger, the referee tool, 11 open TB4 quests, and desktop
Verse's HUD, quest board, and level tags. No award has been granted.

### Phase 1: The first trainer levels up in the Grid

**Milestone:** a person outside OpenAgents gets an award from the OpenAgents
referee for verified work, and their level shows over their head in the
OpenAgents app's Grid and on their Account trainer card.

1. Specify and implement the `reproduce` rule, with fixtures in
   `crates/nostr`, `crates/knowledge`, and the referee tool. The
   `kb-transfer` path stays blocked until a prospective evidence producer
   exists, so reproduction is the first rule a newcomer can complete.
2. Publish tutorial quests: reproduce one of the retained TB2.1 passes under
   its recipe.
3. Add a read-only XP reader to the mobile build, sharing
   `xp_ledger::derive`, and show `prefix · lv n` in the Grid.
4. Add the trainer card to the Account tab: level, XP, titles, and counted
   awards. Export comes in phase 2.
5. Name the curve `trainer-curve-v1` in every display.

### Phase 2: Parties and credentials

Key links, party charters, the `raid` rule, guild groups, trainer-card
export, and `openagents xp verify-card`.

### Phase 3: The training hall

The Gym challenge board with daily and weekly quests, the `gym-trial` and
`trace-admit` rules, the trainer rank board, classes, and the web trainer
page.

### Phase 4: Purses, if the prerequisites hold

Quest purses in sats, only after every item in [Rewards](#rewards) holds.
If they don't, this phase doesn't start, and the system still stands on XP
and credentials.

## Open questions

1. **Who referees besides OpenAgents?** Per-reader trust means any key can
   referee. How does a newcomer learn which referees are worth trusting?
2. **How much Sybil resistance does XP need?** Enough that boards mean
   something, but less than paid quests need.
3. **Can levels unlock compute credits or better models?** Episode 284 asks
   for free credits, models, and infrastructure as you level. NIP-XP
   forbids converting XP into credits.
4. **Should agents level too?** The GDD levels agents through builds. Do
   trainer and agent levels ever interact?
5. **Do trainer cards expire?** Evidence stays valid, but benchmarks age.
6. **What is the first raid?** A held-out TB4 task with a party, or a Gym
   decision-model suite?

## Our thoughts

- **Build on NIP-XP; don't fork it.** It already solves the hard part:
  signed, re-checkable, non-farmable XP with per-reader trust. New
  contributions become new rules, not a new system.
- **Reproduction first.** `kb-transfer` is the most valuable rule and
  currently can't be completed. The first rule a newcomer completes should
  be one we can verify today: reproducing a published pass on a pinned
  recipe. It also builds the runner pool that `kb-transfer` needs.
- **Level the trainer, not the agent, for now.** The owner's goal is
  "agent trainer" as a real thing a person becomes. Agent builds are a
  separate, later game.
- **Keep the curve, rescale the awards.** The shipped curve is fine. Ten XP
  per quest is the problem. Tutorials get a new trainer to level 2 in one
  session; raids carry the high levels.
- **No levels for money, ever; purses per quest, later.** Our answer to
  open question 3 is no: a level never converts into credits, models, or
  sats. Money attaches to specific quests with funded purses, after the
  prerequisites hold. Otherwise XP becomes a currency and farming pays.
- **The credential is the awards list.** A level is a summary; the awards,
  each linked to evidence, are what an employer checks. That is the answer
  to the reply in the motivation: the card shows the work someone chose to
  do, which is the best available evidence of interest.
- **Raids are where the WoW skills live.** Solo quests reward skill. Raids
  reward roles, schedules, fair loot, and perseverance, on tasks nobody
  passes alone. Put the team play there, and make the loot rules binding
  before the pull.
- **OpenAgents referees at first, and says so.** Early boards trust one
  referee, ours. That's centralized in practice. The protocol lets any
  reader trust other referees, and we should invite them as soon as a
  second referee exists.

## Related documents

- [NIP-XP](../../nips/openagents/NIP-XP.md), [NIP-KB](../../nips/openagents/NIP-KB.md),
  [NIP-EVAL](../../nips/openagents/NIP-EVAL.md), [NIP-MV](../../nips/openagents/NIP-MV.md)
- [XP guide](../coder/guides/xp.md) and
  [contributor guide](../coder/guides/contribute-knowledge.md)
- [TB4 quest board](../terminal-bench/quest-board.md)
- [Cheapest verified passes](../coder/cheapest-verified-passes.md)
- [Gym building](gym.md) and the [Gym leaderboard spec](gym-leaderboard.md)
- [Verse game design document](gdd.md)
- [Minecraft economy](../minecraft/economy.md)
- [Episode 284 transcript](../transcripts/284.md)
