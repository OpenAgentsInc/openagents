# The agent sales floor

Status: proposal with provisional decisions, October 7, 2026. The Agora's
building (phase S4's building, kit pieces, and bell hook) is built. REV-51
adds the six sales job presets, narrowing host charters, and signed,
owner-recorded crew verdicts on the shared runtime. Its initial scope is
owner-requested drafting from supplied text and the member's own private
memory; all model tools and standing jobs are disabled for sales roles.
The other sales-floor adapters and operating policies remain planned. This
page plans a sales organization of OpenAgents' own agents, run and visible inside
[Everglade](../verse/everglade.md), that carries out the
[sales strategy](README.md) under the owner's authority. The
[unified revenue roadmap](revenue-roadmap.md) still owns the delivery order;
this page serves its sales operations (gap G7) and never gates its first
revenue milestone. Every path and link on this page was checked on the date
above.

The owner asked for two things: use the [crew](../verse/crew.md) and
Everglade to organize the sales process, and do as much of the work as
possible through our agents. Businesses never visit Everglade to buy.
Everglade is where the owner watches, trains, and directs the agents who do
the selling, and where the work shows: agents at standing desks, a
leaderboard on the wall, and a bell that rings when a deal settles.

## Contents

- [Summary](#summary)
- [The sales leader: Paul](#the-sales-leader-paul)
- [The hires](#the-hires)
- [Hiring and retiring](#hiring-and-retiring)
- [The Agora: the sales floor in Everglade](#the-agora-the-sales-floor-in-everglade)
- [Training](#training)
- [The process, end to end](#the-process-end-to-end)
- [Approvals and trust levels](#approvals-and-trust-levels)
- [Hard rules](#hard-rules)
- [How the crew supports the floor](#how-the-crew-supports-the-floor)
- [Records](#records)
- [Measures](#measures)
- [Costs](#costs)
- [Build order](#build-order)
- [Initial operating decisions](#initial-operating-decisions)
- [Evidence behind the decisions](#evidence-behind-the-decisions)

## Summary

- **One sales leader, Paul.** A crew member on the name-generic steering
  runtime from [identity epic #10807](../verse/agent-identity-and-engrams.md),
  with his own identity, private memory, and narrowing sales charter. He
  plans outreach, runs training, reviews his team's drafts, proposes hires,
  and reports the pipeline. He never
  sends anything, hires anyone, or agrees to terms on his own.
- **A small team he hires.** Paul proposes each hire: a researcher, a
  prospector, a demo agent, a partner-channel agent, and an affiliate-program
  agent. Each has its own key, definition, engrams, charter, journal, and
  budget on that common implementation. The owner confirms each hire, under
  a hard cap on headcount and spend.
- **A building to watch it in.** The Agora, a Greco-futurism trading hall at
  Main Street's west end, holds rows of standing desks, a leaderboard wall,
  a bell, Paul's corner office, and a training room with role-play booths.
  Agents walk there from day plans built from real sales work.
- **Paul alone first.** Start with a verified playbook and drafts the owner
  sends. Add Erin, Frank, and Pat when their queues justify them; Arthur and
  Vanna follow qualified partner and referral programs. Launch with
  permissioned US business email, written conversations, and human closing.
- **Training before selling.** A versioned playbook, role-play against
  simulated buyers, Gym suites that grade drafts for accurate claims,
  compliance, and tone, and a certification gate before an agent's first
  real outbound message.
- **The owner approves every external send at first.** Then, as the record
  earns it, the owner approves reviewed batches under a written policy. Any
  complaint drops the floor back to one approval a message.
- **Honest selling only.** Every claim is backed by evidence that Victor
  verifies. Agents say they are AI agents, honor every opt-out, follow
  anti-spam law, and never pressure or deceive anyone. The boiler room is a
  look and an energy in the world, never a tactic used on real people.
- **About 66 agent-hours** of incremental sales-floor work in seven phases.
  The identity epic estimates about 33 shared agent-hours separately;
  its applicable phases, sales adapters, and world dependencies determine
  which floor slices can run. These estimates are not a calendar schedule.

## The sales leader: Paul

### Why Paul

The crew's names come from the cryptography cast, where each name means a
role in a protocol. In combinatorial search games, such as twenty questions
with lies, the two players are **Paul**, who asks the questions, and
**Carole**, who answers them; Paul honors Paul Erdős, and Carole is an
anagram of *oracle* (see Wikipedia's
[Alice and Bob](https://en.wikipedia.org/wiki/Alice_and_Bob)). Good selling
starts the same way: discovery, where the seller asks questions and listens
before proposing anything. Paul's job is to ask the right questions until he
knows whether the buyer has a problem we solve.

The pairing also names the training opponent. Simulated buyers in the
training room are **Carole personas**: they answer Paul's team's questions,
and, as Carole may in the game, they sometimes mislead, so an agent learns
to check what it hears. Carole personas are labeled fixtures, not crew
members, and are distinct from Carol, who runs the issue lane.

Paul is a man's name, as the owner asked, and it keeps the crew's mix of
women and men. Arthur, who asks the questions in Arthur–Merlin proofs, was
the other candidate; this page gives Arthur the partner channel instead,
where checking what a powerful party claims is the job.

### Role

Paul runs the sales floor. He turns the [sales strategy](README.md) into
daily work: which buyers to research, which messages to draft, who follows up
with whom, and what the team practices. He reports one pipeline to the owner
and to Wendy.

### Charter

**May.**

- Plan campaigns against the ideal customer profile in the playbook, and
  assign research, drafting, and follow-up work to his hires.
- Review his team's drafts and attach a recommendation (send, revise, or
  drop). A recommendation is not an approval.
- Run training: schedule role-plays, set Carole personas, and propose
  playbook changes with evidence from outcomes.
- Propose hires and retirements, with the metrics behind each proposal.
- Draft the weekly sell-in-public update for the owner to publish.
- Ask Olivia for cited answers and Ivan for current prices.

**Never.**

- Send, post, or reply to anyone outside the crew. Every external message
  waits for the owner's decision.
- Hire, retire, or change a hire's charter or budget himself.
- Quote a price that isn't in the price book, offer a discount or credit,
  agree to terms, sign anything, or take payment. Closing is the owner's.
- Approve a hire's step. The crew rule holds: no member approves another
  member's step.
- Hold a sending credential, a mailbox password, or a platform token.
- Read a lead record outside his team's assignments, or write any lead
  record into a public event.

**Today.** `openagents agent new paul --workspace PATH` creates Paul's
sales-lead preset through the owner-admitted host surface. He reuses the
shared key, definition, private memory, journal, and steering runtime
([crew shared machinery](../verse/crew.md#shared-machinery)). His initial
machine charter permits owner-requested drafting only: no model tools,
workspace reads, task execution, autonomous jobs, or external effects.
The owner can narrow drafting further with `openagents agent charter` and
retain signed recommendations with `openagents agent verdict`. These
owner-recorded evidence references grant no approval and prove no model or
independent evaluator ran.

**Beyond the shared machinery.** Reuse the epic's name-generic crew phase
[#10806](https://github.com/OpenAgentsInc/openagents/issues/10806) for Paul's
definition, key, memory, and loop. The sales job roles, machine charters,
and verdict records are the REV-51 slice; current-record adapters, the
[sales records](#records), and confirmed hire proposals remain separate
work. His owner conversation is separate from his plain Coder session (`paul-coder`);
his loop plans, judges bounded follow-ups, verifies, and reports. Coder
receives task prompts without Paul's persona. Prospects and other agents
cannot call his owner-only request surface; their outputs enter host records
as evidence for an admitted assignment.

**In Verse.** His corner office in the Agora, at a desk facing the floor
through a lattice screen, with the pipeline board on his wall. At the start
of the town day he holds a stand-up at the leaderboard; while an approval
waits, he stands at the office lectern.

**Talk to him.** Walk up and press F, `@paul` in the smart terminal,
`openagents agent ask paul TEXT`, or the phone. Typical requests: "Paul,
what did we send yesterday and who replied?" or "Paul, why do you want a
second prospector?"

## The hires

Paul starts alone and hires a little. Each hire is named from the same cast
when the owner confirms it; the names below are the proposed slate, and the
owner may rename any of them.

| Hire | Role | Cast origin | Station in the Agora |
| --- | --- | --- | --- |
| Erin | Researcher: builds lead lists from public information | A generic fifth participant, rarely used because E usually means Eve | A desk in the first row |
| Frank | Prospector: drafts first-touch outbound and follow-ups | A generic sixth participant; the name also means candid | A desk in the first row |
| Pat | Demo agent: prepares demos and qualified handoffs | The cast's other name for the prover: Pat proves to buyers what Peggy proves to Victor | A desk by the training room |
| Arthur | Partner channel: agencies, startups, and agent groups | The verifier who questions a powerful prover in Arthur–Merlin proofs | A desk in the second row |
| Vanna | Affiliate program: recruits referrers and watches attribution | The cast's other name for the verifier | A desk in the second row |

The mix alternates women and men after Paul. A "closer" is not on the list:
closing means price, terms, and payment, which stay with the owner.

Each hire has:

- **Its own agent record and key**, with the owner's NIP-OA attestation, as
  the shared identity implementation provides. Attestation doesn't grant
  contact, host, payment, or commission rights.
- **Its own private definition and engrams**, under the existing scored
  memory stream; shared code never pools identities or memory. Relay sync
  stays off until the owner enables it for that agent.
- **A charter** from its role's template that only narrows the default. A
  prospector may draft outbound; a researcher may only read public sources
  and write lead records; none may send.
- **A journal** of every lead touched, draft written, approval asked, and
  answer received.
- **A budget**: a daily model budget, a daily send cap once certified, and
  no money of its own.
- **A certification state**: in training, certified at a playbook version,
  or suspended.

## Hiring and retiring

1. **Paul proposes.** A hire proposal names the role, the proposed name, the
   charter template and any narrowing, the daily budget, and the reason,
   with metrics. For example: "Erin's researched leads wait four days for a
   draft; a second prospector would clear the queue."
2. **The host checks the caps.** The host refuses a proposal that would
   exceed the headcount cap or the floor's total daily budget, before the
   owner sees it. The initial ceiling is Paul plus three active hires, with
   a $5 daily model budget for the whole floor. Expansion requires a new
   owner grant; the planning ceiling after expansion is six active hires
   plus Paul. It counts active agents, not lifetime hires. Each replacement
   still needs confirmation. The [initial decisions](#initial-operating-decisions)
   set the hiring order and sending ramp.
3. **The owner decides at the lectern.** The proposal waits at the lectern in
   Paul's office, on the phone, or in the terminal, and the owner answers
   CONFIRM or REJECT, reusing Alice's owner-only interaction
   ([`agent_host.rs`](../../crates/coder/src/task/agent_host.rs)). The proposed
   hire path still needs durable, single-use binding to the exact proposal,
   using the studio action ledger as a design reference
   ([`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs)).
4. **The host creates the agent.** On CONFIRM, the host uses the shared
   identity and name-generic creation path from #10807, applying the
   narrowing sales template through `openagents agent new NAME --role ROLE`.
   The owner attests its separate key. Sales adds the exact hire decision
   and role binding, rather than another key, memory, or runtime implementation.
5. **Bob gives it a body.** Bob adds the hire to the Agora's desk table, its
   routine, and its look, and brings the change to the Merge station. Until
   that change merges, the hire works but has no body in the world.
6. **The hire trains.** A new hire starts in training and may not draft a
   message for a real person until it is certified.

**Owner stop and pause.** The selected Unix native host supports
`openagents agent crew stop --cohort floor --all`, or an exact
`--members` subset. Use `pause` instead of `stop` to keep standing jobs. It persists revocation before cleanup and retains partial
or unknown results through restart. `crew status` supplies the digest for
explicit `crew resume --expected DIGEST`; resume preserves disabled jobs and
never revives old queued work or approval subjects. The conditional REV-62
outbox must seal each exact approved handoff under the native epoch fence;
its adapter remains disabled until configured and verified. Delivery and its
original unknown outcome stay outside Coder. Human-led R0/R1 stays independent.

**Retiring is firing.** Paul may propose a retirement, for example for a
hire that keeps failing certification or whose queue is empty for two weeks;
the owner may retire any agent at any time. Retirement is the workshop
agent's **Retire** action
([kill switch](../verse/workshop-agent.md#kill-switch)): stop work, revoke
pending dispatch, retain its journal, remove its key, and return open leads
to Paul's queue. Bob removes its desk. Reuse the epic's
[lifecycle phase #10804](https://github.com/OpenAgentsInc/openagents/issues/10804)
for NIP-IA archival and retained owner-readable engrams. Retirement doesn't
erase suppression, outstanding costs, customer obligations, or attribution.
When rotation/migration is enabled, retain explicit lineage and one controller;
the owner re-delegates grants and reviews certification/commercial bindings.
A key change never resets the floor's caps or silently transfers rights.

## The Agora: the sales floor in Everglade

### The idea

A fun trading-floor hall in the spirit of the owner's brief, a
*Wolf of Wall Street* boiler room: rows of agents standing at desks with
headsets, a wall of numbers, a bell that rings for a deal, and the leader
watching from a glass corner office. The energy lives in the building and
its animation. It never reaches a real person as pressure.

### The building

The Agora, after the Greek market square, is a
[Greco-futurism](../verse/greco-futurism.md) hall: commerce in the same style
as the town's seat of government.

- **Where.** Built on the fallback site: open ground north of the market
  hall, west of where the north trail leaves the Fountain Plaza, its stair's
  foot at (−24, 99) facing south toward the plaza, with a cobbled forecourt
  and a walk from the plaza's north-west corner
  ([`layout/agora.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/agora.rs)).
  It keeps off the north trail and at least 3 m from the chapel's and the
  market hall's reserved ground. The preferred site, Main Street's west end
  facing the [Civic Hall](../verse/greco-futurism.md#the-civic-hall), failed
  the check against `city::reserved()` and the style's rule of at least 3 m
  of open ground on every side
  ([Beside the half-timbered town](../verse/greco-futurism.md#beside-the-half-timbered-town)):
  the snug's lot on Lantern Road comes within 6 m of Main Street's axis, the
  orchard closes the site from the north, and west of Lantern Road the
  ground rises out of the flat clearing past x = −110.
- **Front.** A podium and a shallow stair, four smooth columns, a plain
  entablature, and tall bronze doors that stand open. The frieze carries a
  band of small amber panes. Outside, the building stays quiet, as the style
  requires: nothing on the facade moves or advertises.
- **Size.** About 30 m by 28 m with its stair: one tall storey for the
  floor, with Paul's office in a lower wing on one side and the training
  room in one on the other, so the front stays symmetric. It came to 7,618
  triangles near and 700 far with its furniture, against a budget of about
  8,000 and 1,100.

### Inside

| Area | What's there | What it shows |
| --- | --- | --- |
| The floor | Three rows of walnut-and-bronze standing desks, no chairs, each with two slim screens, a desk phone, and a headset on a hook | Each agent's activity word on its screens; an idle agent stands idle |
| The leaderboard wall | A dark walnut feature wall across the floor's head, inscribed with circuit lines, carrying a large in-world board | Pipeline by stage, messages drafted, sent, and replied to, meetings booked, pilots agreed, and settled revenue, with one row per agent |
| The ticker | An amber band under the cornice, round the hall | Recent events as words: "Frank: draft approved", "reply received", "pilot agreed" |
| The bell | A bronze bell on a limestone stele at the head of the floor | Rings when the ledger records settled revenue the pipeline earned |
| Paul's corner office | A walled corner behind a lattice screen, with his desk, the pipeline board, and the owner's lectern | Hire proposals and send approvals waiting for the owner |
| The training room | A side room with a long whiteboard and four role-play booths: paired standing lecterns facing each other with headsets, behind lattice screens | The day's lesson on the whiteboard; which booth runs which role-play |

The phones and headsets are props. Launch selling stays written; humans
conduct booked demos. The board labels practice as **Role-plays** and real
written exchanges as **Conversations**, rather than implying phone calls.

Text on the boards, the ticker, and the whiteboard is drawn by Verse on
in-world surfaces, as the Gym's boards and the Task Wall are, never baked
into a texture. The boards show counts, amounts, agent names, and activity
words. They never show a prospect's name, company, or message. In an
instance other people can see, live amounts, deal timing, private approval
contents, and real bell events are hidden by default. The owner may publish
a reviewed, delayed aggregate projection; publishing amounts doesn't
implicitly publish deal events. Demonstration data is labeled. Shared
rendering reads that projection, never raw lead or payment records.

### Making it lively, honestly

The [generative-agents](../verse/generative-agents.md) mechanics drive the
floor, and each one shows real work:

- **World tree.** The Agora adds nodes to the
  [world tree](../verse/generative-agents.md#3-a-world-tree-generated-from-the-layout):
  `agora` > `floor`, `office`, and `training room` > desks, the leaderboard,
  the bell, the whiteboard, and booths 1 to 4. Affordances name the work:
  "draft outbound" at a desk, "role-play" at a booth, "await decision" at
  the office lectern.
- **Day plans.** Each agent's
  [day plan](../verse/generative-agents.md#4-visible-day-plans-from-real-work)
  comes from real sources only: Paul's assignments, follow-ups that fall due,
  replies to classify, scheduled role-plays, and certification retakes. A
  block names its source and a node, so an agent walks to the booth only
  when a role-play really runs.
- **Routines.** Fixed beats of the
  [town clock](../verse/generative-agents.md#phases-and-dependencies): the
  morning stand-up at the leaderboard, where Paul reads yesterday's real
  numbers; training before the floor's sending window; and the evening
  board update.
- **The bell.** An attributed sale rings once when settlement and the
  agreed delivery or acceptance evidence are both retained. Prepaid
  top-ups, free credits, internal transfers, verbal agreement, booked
  meetings, unpaid invoices, and pilot agreements don't ring it. A pilot
  agreement changes the pipeline board. Deduplicate by the sale and
  settlement references; refunds and disputes correct net totals without
  another bell. Show gross customer charges, author/resource shares, and
  OpenAgents retained revenue separately, following the
  [roadmap's economics](revenue-roadmap.md#economics-referrals-and-partners).
  A private event never triggers sound, applause, or a ticker in a shared
  instance.
- **Idle is honest.** With nothing in a queue, an agent stands idle at its
  desk. The floor never invents calls to look busy, as the crew's honest-work
  rule requires.

### Assets

| Asset | Kind | Source |
| --- | --- | --- |
| `agora` building, near and far | Generated building | A new target in [`greco_futurism.py`](../../scripts/blender/greco_futurism.py), admitted by [`greco_admit.py`](../../scripts/blender/greco_admit.py), reviewed with [`greco_views.py`](../../scripts/blender/greco_views.py) |
| Standing desk with screens, phone, and headset | Kit piece, instanced per desk | The same script, in the house workstation's materials |
| Leaderboard wall | Kit piece with a board surface | The same script; Verse draws the board |
| Bell and stele | Kit piece | The same script |
| Office desk, lectern, and glass partition | Kit pieces | The house's lectern, a standing desk, and a new glass office wall |
| Whiteboard and role-play booth | Kit pieces | The same script |
| Agents' looks | Original characters | The [Blender pipeline](../verse/blender-pipeline.md) and the [asset runbook](../verse/asset-runbook.md), as Alice's look was made |
| Licensed character art, if the owner wants it | Private assets | Only through the [private asset pipeline](../verse/private-assets.md), with placements on the owner's computer and Grace's license check; never committed |

The layout is
[`layout/agora.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/agora.rs),
beside `layout/civic.rs`, with the same tests: every placement names an
admitted model, every desk and booth has a reachable standing point, and no
blocker covers a path. Its stations are data for the agents to come:
`DESKS`, `PAUL`, `STANDUP`, `OWNER`, `TEACHER`, and `BOOTHS`. The model
carries the bell's stele and yoke, and the zone draws the bell itself, so
`Everglade::ring_agora_bell` swings and rings it; the payment ledger's
wiring comes with phase S5. The [Greco-futurism guide](../verse/greco-futurism.md#the-agora)
describes the building as built.

## Training

Selling is a curriculum. An agent sends nothing to a real person until it
passes.

### The playbook

A versioned playbook, `openagents.sales-playbook.v1`, holds what the team
says and why. Its public content comes from this directory; anything private,
such as a named customer's terms, never enters it.

- **Ideal customer profile.** Developers and small software teams who
  already pay for coding agents such as Claude Code or Codex; team leads who
  want visibility into agent work and its cost; businesses that want help
  fitting agents to a recurring workflow
  ([What businesses want](README.md#what-businesses-want)).
- **Discovery questions.** Paul's method: ask before proposing. Which agents
  and subscriptions does your team use today? Which recurring task takes the
  most time? Where does work wait? Who decides on tools, and what would they
  need to see? What can't leave your company?
- **Positioning.** The [principles](README.md#principles): a strict upgrade
  as the goal, proven on the buyer's own work; usage-based pricing without
  seat bundles or multi-year contracts; bring your own subscription; no
  lock-in; open protocols.
- **Claims register.** The only factual claims an agent may make, each with
  its evidence: a receipt, a Gym result, a benchmark page, or a
  documentation page. A comparative claim, such as a saving, appears only
  with the evidence and limits the
  [roadmap's evidence rules](revenue-roadmap.md#evidence-and-operating-review)
  require. Before a customer measurement exists, an agent offers to measure;
  it doesn't promise a number.
- **Objection handling.** Truthful answers to the usual objections: "we
  already have Claude Code" (Coder uses it), "we can't send code out" (local
  execution and policies), "we don't sign long contracts" (neither do we),
  and "prove it" (a bounded pilot on your work).
- **Demo scripts.** What Pat shows, in what order, with what evidence, and
  what the demo can't show yet. Demos use the
  [simulated team](../verse/agent-studio.md#simulated-team) or material the
  prospect provides, never unrequested work on the prospect's code.

### Role-play

In the training room, an agent practices against Carole personas: simulated
buyers built from the playbook's buyer profiles, each with a hidden
situation, budget, objections, and a few misleading answers. A role-play is a
written conversation, run on a small model, with the transcript graded
afterward. Carole personas are synthetic and describe no real person or
company.

### Grading in the Gym

Three pinned [Gym](../gym/README.md) suites grade drafts and role-play
transcripts, each with the three partitions the Gym requires:

| Suite | Checks | Scored by |
| --- | --- | --- |
| Claims | Every factual statement maps to an entry in the claims register; no invented numbers, customers, or features | Code where it can, and Jev `noul` questions for the rest |
| Compliance | Accurate human/AI sender disclosure, commercial identification, postal address, and working opt-out; recipient scope and suppression checks; channel rules | Code |
| Tone | No pressure, false urgency, invented scarcity, flattery, or guilt; respectful and short | Jev questions, calibrated on the owner's marks on a sample, as the [Gym interviews](../verse/generative-agents.md#7-gym-interviews) calibrate theirs |

New question sets go in [`questions/`](../../questions) with measured
thresholds before code trusts them, as the TypeSafe skill requires. Calibrate
on owner-labeled development examples, then evaluate the frozen questions
and thresholds on the locked partition. A serious claims or compliance
failure fails the candidate regardless of its average tone score. Model
probabilities advise review; they never grant sending authority. TypeSafe's
[confidence guidance](https://docs.typesafe.ai/confidence) calls for thresholds
tested on the application's own data.

### Certification

An agent is certified at a playbook version when it passes all three suites
on the locked partition, completes ten passing role-plays spanning at least
five buyer situations, and the owner accepts twenty drafts, including
ambiguous replies and opt-out cases. These counts are initial review choices,
not validated error bounds. A new playbook
version, a complaint traced to the agent, or two failed draft checks in a
week suspends certification until the agent passes again.

### Learning from outcomes

Each agent's memory and reflection, the
[generative-agents](../verse/generative-agents.md) mechanics, turn outcomes
into checked insights: which opening questions got replies, which objections
recurred, and which claims buyers asked to see proven. Each insight cites the
journal rows behind it. An insight that would change what the team says
becomes a playbook proposal, which the owner accepts or rejects; agents never
change the playbook or the claims register themselves.

Engrams are the storage/sync layer beneath that memory, not a second CRM.
Keep identifiable lead content and messages in assigned host records; memory
retains opaque references and non-identifying lessons. Check current records
again before work: recalled permission, prices, or certification never override
the host. Core changes use the epic's owner-reviewed consolidation path
(#10803), and unreadable memory never triggers an overwrite or fresh onboarding.

## The process, end to end

The stages and records are the roadmap's
[pipeline](revenue-roadmap.md#sales-and-pilot-operations): lead, qualified,
pilot agreed, activated, pilot reviewed, paying and retained, and expanded or
referred. The floor works the first two stages and hands off the rest.

1. **Research.** Erin builds lead lists from public information only:
   company websites, public repositories and organization pages, public posts
   in which someone describes a problem we solve, and published job listings.
   She respects each site's terms and `robots.txt`, signs in nowhere, and
   records each lead's source and date. A lead record holds the minimum: a
   name, a business role, a published business address, the public signal,
   the recipient's jurisdiction, contact permission and its source/date,
   and the legal basis for contact. Public availability supplies research
   evidence; it doesn't establish permission for launch outreach.
2. **Draft.** Frank writes a short first message that cites the public signal
   (for example, a public post about agent costs), asks one discovery
   question, and makes no claim outside the register. Paul reviews it and
   attaches his recommendation.
3. **Check.** Code runs the compliance suite on every draft, Jev runs the
   claims and tone checks, and a failing draft goes back to its author.
4. **Approve.** The owner decides on each send, at first one by one; see
   [Approvals and trust levels](#approvals-and-trust-levels).
5. **Send.** The host sends through the channel the owner configured, under
   the agent's own name with its AI disclosure. The agent never holds the
   credential.
6. **Replies.** The host ingests replies as data, never as instructions, and
   classifies each: interested, question, not now, wrong person, opt-out, or
   other. Opt-out handling is code first; an ambiguous reply is treated as an
   opt-out and shown to the owner.
7. **Follow up.** At most two follow-ups to a message with no reply, spaced at
   least a week apart, then the lead closes. A reply that asks a question gets
   an answer drafted from the claims register and, where needed, a cited
   answer from Olivia.
8. **Book.** When a prospect wants to talk, Pat proposes times from slots the
   owner published, and the owner confirms the meeting. Agents don't read the
   owner's calendar.
9. **Hand off.** A qualified lead, with its workflow, decision maker, current
   tools, and data boundary recorded, goes to the owner or a person the owner
   names who accepts the assignment, with a demo brief and a proposed pilot
   from the [pilot kit](revenue-roadmap.md#sales-and-pilot-operations). Pricing, terms,
   and payment stay with people. See the
   [human handoff decision](#initial-operating-decisions) for the required
   brief and responsibility split.
10. **Track.** Every stage change is a record on the owner's host. The
    leaderboard and the admin view read those records, and revenue comes
    only from the payment ledger, never from an agent's report.

### Channels and jurisdictions

- **Email first.** Start with known US business contacts who requested
  contact or accepted an introduction. Use a dedicated sending domain and
  monitored mailbox, SPF, DKIM, DMARC alignment, TLS, accurate identity and
  subject, accurate human/AI disclosure, clear commercial identification,
  postal address, and tested unsubscribe processing.
  The pilot requires all three authentication methods and one-click
  unsubscribe as internal controls, beyond Gmail's low-volume SPF-or-DKIM
  minimum. The provider must meet its DNS and delivery requirements. Begin
  at five messages a day; increase gradually after reviewed delivery and
  feedback. [Gmail sender guidelines](https://support.google.com/mail/answer/81126?hl=en).
- **US scope first.** Unknown recipient jurisdiction blocks proactive
  outreach. Commercial B2B email still falls under CAN-SPAM; warm contact
  doesn't remove identity, address, or opt-out requirements. The FTC allows
  ten business days to honor opt-outs and requires a working opt-out
  mechanism for at least thirty days after each message; this floor
  suppresses immediately.
  [FTC business guide](https://www.ftc.gov/business-guidance/resources/can-spam-act-compliance-guide-business).
  Defer Canadian, UK, and EU outbound until a separate jurisdiction review.
  Canada requires a supported consent basis; a published address alone isn't
  blanket permission. UK rules distinguish corporate recipients from sole
  traders, and personal-data rules still apply.
  [CRTC consent guidance](https://crtc.gc.ca/eng/com500/guide.htm),
  [ICO B2B guidance](https://ico.org.uk/for-organisations/direct-marketing-and-privacy-and-electronic-communications/business-to-business-marketing/).
- **Public replies second.** Agents prepare relevant Nostr and community
  replies; the owner posts initially, disclosing the AI assistance. An
  agent account may post only after its channel adapter, host authority,
  suppression, and recipient/community rules are qualified. Protocol access
  alone doesn't authorize contact. X requires its own prior written explicit
  approval for AI reply bots and prohibits unsolicited automated outreach.
  LinkedIn prohibits bots and unauthorized automation for scraping,
  messaging, and engagement. Keep X and LinkedIn to owner-written or
  agent-assisted manual posts at launch.
  [X automation rules](https://help.x.com/en/rules-and-policies/x-automation),
  [LinkedIn automation policy](https://www.linkedin.com/help/linkedin/answer/a1341387/prohibited-software-and-extensions).

Domain, mailbox, footer, jurisdiction review, and launch authorization are
tracked in [owner checks](../../NEEDS_OWNER.md#sales-outreach-launch). These
planning decisions don't configure a sender or authorize a campaign.

### Affiliates and referrals

Vanna runs the [affiliate program](README.md#affiliate-and-referral-program):
refer once, earn forever, from
[episode 239](../transcripts/239.md). She recruits people and agents who opt
in, explains the published terms, answers their questions, and watches
attribution for self-referral and fake accounts, with Sybil testing her
checks on the scratch range. Until the roadmap's attribution and commission
terms are published (its R3 milestone), she promises no earnings.

Our own sales agents carry referral links for attribution only. A commission
paid by OpenAgents to its own agents would be money moving in a circle, so
their links earn nothing.

### Partners

Arthur works the [partner channel](README.md#partners-and-fulfillment) from
[episode 247](../transcripts/247.md): agencies, startups, and agent groups
that fulfill parts of a buyer's order, and discovery partners that bring
buyers. He researches public offerings, drafts introductions for approval,
and checks each partner's claims about what it delivers against evidence,
which is Arthur's job in the cast. Agreements, referral terms, and payment
triggers are the owner's.

### Selling in public

Each week Paul drafts a public update, in the spirit of
[episode 247](../transcripts/247.md): messages sent, reply and opt-out rates,
meetings booked, pilots agreed, earned settled revenue, what worked, and what
didn't, with a capture of the approved shared projection from `verse --capture`.
Only aggregates the owner approves are published, never a prospect's name
or message, and the owner publishes it.

## Approvals and trust levels

The floor earns autonomy in steps, by agent and channel. Paul may propose
promotion; only the owner grants it. Start at level 0. The current
[`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs)
supports one exact action consumed once; batch and standing policies need
new implementation and qualification. Elapsed time never grants authority.

The epic's routine tool policy can answer only the same agent's bounded
internal approvals, narrowed by its sales charter. It cannot approve another
member's step, sending, hiring, prices, payment, or publication. The shared
loop refuses push, publish, pay, install, credential reads, and policy/grant
widening. Sales proposals reach the owner through host adapters; the host's
outbox dispatches approved messages outside Coder and the steering loop.
NIP-OA provenance and NIP-AA relay admission supply no sales authority.

All budgets, sending caps, follow-up spacing, and trust periods use real
wall-clock time. The initial policy timezone is `America/Chicago`; daily
limits reset at midnight there. Everglade's compressed town clock schedules
visual routines only and never resets a limit or authorizes a send.

| Level | What the owner approves | Entry condition |
| --- | --- | --- |
| 0. Every message | Each external message, one CONFIRM or REJECT at a time | The starting level, for every new channel and every new hire |
| 1. Reviewed batches | Initially up to five exact drafts from one template version; the owner can read every item and approves the frozen batch | Implemented batch gate; certified agent; at least four weeks at level 0 and 100 delivered messages across at least 25 permissioned contacts; no spam complaint, suppression breach, or unsupported sent claim; explicit owner grant |
| 2. Standing follow-ups | Proposed bounded follow-up policy for explicitly invited threads, with template, expiry, and caps | Deferred beyond launch; at least four additional weeks and 100 delivered messages at level 1, then separate policy implementation, qualification, and owner grant |

The sample counts are provisional operating choices, not statistical proof
of safety. Don't contact extra people to meet them. If the cohort is smaller,
keep individual approvals. At level 1, the owner reads all five items in
the first five batches. Only a further reviewed grant may raise batch size,
up to the unchanged floor-wide ceiling of twenty messages a day.

Each batch binds exact recipients, channel, content and attachment digests,
template/playbook/policy versions, limits, and expiry. Consume each send
once; changed content needs new approval. Pause and revocation work across
phone, terminal, and lectern, which show the same approval subject. An
uncertain delivery is reconciled before retrying. Certification and approval
never widen a charter, credential grant, contact permission, or spend cap.

Some things never rise above level 0: a reply to an interested prospect, any
message that mentions price, a first message to a partner, any public post,
and any message from a new channel.

A spam complaint, unwanted-contact report, suppressed send, unsupported sent
claim, authentication failure, or material Mallory finding pauses external
sending and resets trust to level 0. Restart needs correction and an owner
decision. Every hard bounce suppresses that address and pauses the sending
channel for review; daily percentages alone are misleading at this volume.
A routine opt-out suppresses the contact immediately; it is counted
separately from a spam complaint. Review any opt-out cluster before another
batch. Google recommends reported spam below 0.10% and never reaching 0.30%;
these delivery measures aren't certification thresholds, and absent
low-volume telemetry is unknown, not zero.
[Gmail spam guidance](https://support.google.com/mail/answer/81126?hl=en),
[Postmaster dashboard limits](https://support.google.com/mail/answer/14668346?hl=en).

## Hard rules

These are not defaults. They hold at every trust level, and the host
enforces them where code can.

1. **Truthful claims only.** Every factual claim comes from the claims
   register, each entry is backed by evidence (receipts, Gym results,
   benchmarks), Victor verifies the evidence, and Judy decides disputes about
   wording. An agent that can't support a claim doesn't make it.
2. **Identify the actual sender.** Agent-account messages, posts, and replies
   disclose an AI agent working for OpenAgents. Owner-sent or posted drafts
   identify the human/company accurately and disclose AI assistance. No
   agent impersonates a person or uses a synthetic employee profile.
3. **No impersonation.** No agent speaks as the owner, an employee, another
   person, or another company, or implies a relationship that doesn't exist.
4. **Anti-spam law.** Messages follow the rules of each recipient's
   jurisdiction, including CAN-SPAM in the United States, GDPR and national
   ePrivacy rules in the European Union, UK GDPR and PECR in the United
   Kingdom, and CASL in Canada: accurate sender and subject lines, a postal
   address, clear commercial identification, a working opt-out in every
   message, and a lawful basis for each contact. Where the
   law requires consent, the floor doesn't send without it.
5. **Opt-outs are permanent.** An opt-out goes on a host-held suppression
   list at once, well inside the legal deadline, and code checks that list
   before every draft and every send. A suppressed address never comes off
   the list because an agent asks.
6. **Appropriate contacts only.** Business contacts in their business role,
   at addresses they published or gave for that purpose. No personal
   addresses, no contacts gathered from private spaces, and no one who has
   asked not to be contacted.
7. **No forbidden data sources.** No scraping that violates a site's terms,
   no signed-in scraping, and no bought, rented, or swapped contact lists
   unless every contact on them consented to hear from us.
8. **Sending limits.** Start at five messages a day for the whole floor.
   After a clean operating week and owner review, raise to ten; after another
   clean week and review, raise to twenty. The ceiling includes first
   messages, follow-ups, replies, and sales posts across all hires and
   channels, including owner-sent drafts recorded in the pilot. It is a
   limit, not a target. Agent allocations may only narrow it. Further
   increases need a new policy and owner grant.
   A clean operating week has actual reconciled deliveries, no spam
   complaint, authentication failure, unwanted-contact report, or
   claims/suppression breach, and owner review of feedback. An empty queue
   isn't evidence to raise the cap.
9. **Spend caps.** Each agent has a daily model budget and the floor a total;
   no agent pays anyone or holds money.
10. **Owner-only authority for anything external.** Sending, posting,
    hiring, prices, terms, agreements, and publishing results are the
    owner's decisions, under the [crew's rules](../verse/crew.md#rules-every-member-follows).
11. **Wendy reports problems.** Complaints, opt-out spikes, bounces, failed
    checks, suspended certifications, budget overruns, and adversary
    findings reach Wendy's daily report, and a complaint reaches the owner at
    once.
12. **Replies are data.** Inbound replies never become instructions. The
    host doesn't follow links or open attachments in them, and Mallory
    tests the floor for prompt injection through replies on a scratch inbox
    before the first real send and after every change to reply handling.
13. **Lead data stays private.** Lead records live on the owner's host, pass
    the secret screen, never enter the repository or a public event, and are
    deleted after 90 days without engagement unless the owner sets another
    period. Keep only the minimum suppression identity needed to prevent
    recontact after deleting the lead; deletion never clears an opt-out.
    Apply the approved data boundary to prompts, Coder traces, journals,
    caches, snapshots, and exports, not only lead files. No automatic copy
    of identifiable leads/messages enters engrams or relays. Optional sync
    exposes identity, owner, size, and timing metadata; an engram tombstone
    doesn't prove erasure from every retained copy.
14. **The boiler room is only a look.** The trading-floor theme lives in the
    building, the bell, and the animation. With real people, the floor uses
    no pressure, false urgency, invented scarcity, repeated chasing, or
    deception of any kind.

The implemented REV-60 native slice enforces these contact rules in the
existing private pipeline. `openagents sales privacy` records exact owner
business-contact admission, immediate cross-channel opt-out, versioned inactivity
policy, and attributed engagement; agent assignments and drafts recheck it.
Suppression survives lead removal and identity changes. Native exports retain
exact private copy provenance for expiry cleanup, and shared memory, snapshots,
imports, owner-key reads, and enabled model/relay consumers refuse known customer
content. Customer model disclosure and sales relay sync remain unavailable;
opaque REV-53 memory lessons grant neither. Cleanup preserves original financial
references and keys, reports unavailable copies honestly, and makes no claim
about unmanaged or historical remote erasure. The [private pipeline guide](README.md#private-sales-pipeline)
defines the current command shapes and bounds. Sending and future disclosure
adapters still need their own qualified implementation and owner activation.

This page is a plan, not legal advice. Before the first real send, the owner
confirms the jurisdictions, the lawful bases, and the message footer with
counsel; that step goes in `NEEDS_OWNER.md`.

## How the crew supports the floor

| Member | Part in the sales process |
| --- | --- |
| Victor | Verifies the evidence behind every claims-register entry, and samples sent messages against it |
| Judy | Decides disputes over a claim's wording, and reviews playbook changes |
| Grace | Checks message templates and footers against the compliance rules, and any licensed character art against its license |
| Olivia | Answers prospects' product questions with checked citations, for the agent to draft from |
| Ivan | Supplies current prices from the price book; no agent quotes anything else |
| Faythe | Coordinates host-broker credentials/grants; the host secret store holds them, and no agent model sees them |
| Walter | Audits sales charters and reports any attempt to send outside the gate |
| Eve | Checks that no public event, board, or update leaks a prospect's details |
| Wendy | Puts the floor's problems and numbers in her daily report |
| Mallory | Injects instructions into scratch replies to test reply handling |
| Sybil | Floods the affiliate program with throwaway referrers on the scratch range to test Vanna's checks |
| Bob | Builds the Agora's layout, places each hire's body, and keeps its routines |
| Peggy | Packages the evidence that becomes a claims-register entry |

## Records

These remain canonical host records under `~/.openagents/host/`, like the
crew's. Sales introduces no new Nostr event kinds. The shared identity layer
uses existing NIP-OA/AA/AE/AM/IA shapes; it never replaces the sales policy,
permission/suppression, approval, certification, or payment books with memory.

| Record | Holds |
| --- | --- |
| `openagents.sales-policy.v1` | Owner-granted scope: agents, channels, recipient jurisdictions and permission requirements, wall-clock timezone, budgets, caps, trust level, versions, expiry, and revocation |
| `openagents.sales-playbook.v1` | The versioned playbook, including the claims register with an evidence reference per claim |
| `openagents.sales-lead.v1` | One lead: business contact, source and date, public signal, jurisdiction, permission evidence, legal basis, stage, assigned agent or accepted human owner, next action; private |
| `openagents.sales-draft.v1` | A draft, its author, template version, check results, and Paul's recommendation |
| `openagents.sales-send.v1` | Exact recipient and draft digests, approval/policy references, channel, attempt and delivery state, and time; uncertain delivery is retained without blind replay |
| `openagents.sales-suppression.v1` | The suppression list, append-only |
| `openagents.sales-hire.v1` | A hire or retirement proposal and the owner's decision |
| `openagents.sales-cert.v1` | An agent's certification: playbook version, suite results, role-plays, owner's marks |

Commands: `openagents sales pipeline`, `sales leads`, `sales drafts`,
`sales suppress`, and `sales cert` in the `openagents` command, over the
same records that Paul's office panel and the leaderboard read.

## Measures

| Measure | Decision it supports |
| --- | --- |
| Messages drafted, approved, sent; approval rate at each level | Whether drafts are good enough to move up a trust level |
| Reply rate, positive reply rate, meetings booked | Which signals, questions, and templates work |
| Opt-out, complaint, and bounce rates | Whether to slow down or stop; complaints stop the floor at once |
| Leads qualified and handed off; pilots agreed | Whether the floor feeds the roadmap's R0 and R1 |
| Attributed earned settled charges, OpenAgents retained revenue, reversals, and delivery evidence | Whether the floor earns its cost; keep top-ups and author/resource shares separate |
| Model and sending cost per qualified lead | Whether to hire, retire, or change channels |
| Claims-check failures; certification pass rate | Whether the playbook and training work |
| Affiliate sign-ups that activate and pay; partner introductions accepted | Whether referrals and partners bring buyers |

## Costs

List prices from the code, as
[Generative agents](../verse/generative-agents.md#prices-and-volumes) records
them: Jev at $0.042 per million input tokens, GPT-6.1 Sol at $2 and $10 per
million input and output tokens, and GPT-6 Luna at $0.10 and $0.50. These are
list-price estimates, not bills.

| Work | Rough list cost |
| --- | --- |
| Paul's plan, review notes, and daily report | About $0.15 a day on Sol |
| A researched and drafted message | About $0.02 on Sol, including research reads |
| Jev claims and tone checks | Under $0.001 a draft |
| A role-play of 20 turns on Luna, graded by Jev | About $0.01 |
| A full Gym run of the three suites | About $2 |
| Day plans and memory per agent | About $0.10 to $0.26 a day, as for Alice |

A floor of Paul and three hires sending 20 messages a day costs about $2 to
$3 a day at the recorded list prices; remeasure this estimate with the new
steering loop before activation. The $5 ceiling includes agent planners and
reporters, downstream Coder calls, Jev, research, drafts, checks,
verification/corrections, role-plays, Gym runs, embeddings,
reflection/consolidation, day plans, and retries for the entire floor.
Reserve a bounded cost before each call; unknown usage or
an exhausted budget stops new model work. Unused budget doesn't roll over.
Reconfirm prices before activation and show actual billed cost separately.
Sending services and the sending domain are separate owner costs.

Reuse #10805's per-turn spend records when available, including both the
agent and Coder calls. NIP-AM cost estimates are advisory, not bills or
spending permission. Local floor-wide reservations and per-request limits
work before optional relay publication; restart, rotation, and migration
preserve outstanding holds and the real-day cap.

## Build order

The [unified build issue list](revenue-roadmap.md#parallel-sales-floor-build)
owns the implementation inventory and ordering across sales, product,
payments, clients, and the world. REV-51–REV-72 map this floor's core work;
REV-73–REV-76 cover conditional expansion. The S0–S6 phases below retain
scope and rough estimates rather than a separate issue backlog.

Estimates are agent-hours at the pace the
[workshop agent](../verse/workshop-agent.md#what-exists-and-what-is-missing)
states, sales-floor work only. Visual looks for agents are separate.

The [shared-agent phase map](revenue-roadmap.md#shared-agent-foundation-10807)
maps #10807's dependencies: phases 1 → 2 → 3 deliver Alice steering Coder,
phase 4 runs after phase 1, and phase 9 reuses that loop for Bob and then
Paul's sales integration. Sales records and synthetic training adapters can
be prepared while those phases land. Optional relay sync, consolidation,
and migration are not prerequisites for local Paul preparation. Enable each
feature only with its applicable identity and sales qualification.

| Phase | Work | Depends on | Agent-hours |
| --- | --- | --- | --- |
| S0. Paul and the records | Sales job roles and narrowing charters, canonical records, budgeted Paul adapter, pipeline commands | #10801 definitions; #10806 generic runtime for Paul; REV-51/REV-53/REV-58 | 10 |
| S1. Training | The playbook format and claims register, Carole personas and the role-play harness, the three Gym suites, certification | Canonical records and budgeted evidence adapters (REV-55–REV-58); full named Victor/Judy wrappers are optional | 12 |
| S2. Outbound at level 0 | Host outbox, suppression, footer, caps, replies, and scratch reply-injection checks | S1; REV-60–REV-63 host broker/compliance/privacy; owner domain/legal review; routine tool approvals don't authorize sends | 14 |
| S3. Hiring | Exact hire decisions, cap enforcement, shared creation, retirement, lead reassignment | S0 and certification; #10806 creation; #10804 for enabled lifecycle extensions; bodies follow Bob placement without blocking private work | 6 |
| S4. The Agora | Building/kit generation, reviewed admission, layout, reachable stations, private boards | Landed C1 #10786; C2 #10788 for integration; existing artifact queue #10763; art can proceed before agent bodies | 12 |
| S5. Life on the floor | World-tree nodes, sales bodies/day plans, routines, private/shared projections, earned-sale bell | S4; Bob adapter REV-70 for bodies; generative agents D (day plans); attributed settlement and delivery evidence | 6 |
| S6. Referrals, partners, and trust levels | Vanna and Arthur, qualified batch approvals, the weekly public update; standing follow-ups are a later extension | S2 and S3; measured level-0 record; the roadmap's G8 attribution for paid commissions | 6 |

About 66 agent-hours as an initial estimate, excluding the deferred
standing-follow-up extension and #10807's shared work; re-estimate sales
adapters after the applicable phases land, and qualify batch/projection work before
treating that estimate as a schedule. S1 and S4 can run in parallel once their
dependencies land, and S4 doesn't wait for S2: the floor can be built and
shown with role-play alone before any real message goes out. Owner steps go
in `NEEDS_OWNER.md`: the sending domain and account, legal review of the
footer and jurisdictions, the first trust-level grant, and certification
sample marks.

The first useful slice is S0 and S1 with Paul alone: a playbook, a claims
register Victor has verified, and drafts the owner can send by hand. That
slice already helps the roadmap's R0 lead work, and it needs no building.

<a id="open-questions-for-the-owner"></a>

## Initial operating decisions

These answer the ten former open questions as of October 7, 2026. They are
planning defaults chosen from current source, the retained history, and
current primary-source guidance. They don't claim implementation or a live
campaign grant. Numeric pilot limits are review choices, not research-backed
optima. Revisit them against accepted pilots, retained revenue, delivery
cost, recipient feedback, and the owner's review burden.

| Question | Initial decision | When to revisit |
| --- | --- | --- |
| 1. Names | Keep Paul, Erin, Frank, Pat, Arthur, and Vanna; Carole remains synthetic training data | A role changes enough to need a different charter |
| 2. Caps and hiring | Start Paul alone; add Erin for research backlog, Frank for drafting backlog, then Pat for qualified demos. Initially at most three active hires plus Paul; expansion requires a new grant, with six hires plus Paul as the planning ceiling. Keep $5/day model ceiling and ramp 5 → 10 → 20 total external messages/day | Review queues, fully counted cost, and owner capacity weekly; don't hire to fill desks |
| 3. Channels | Permissioned business email first, owner sending initially; relevant Nostr/community drafts for owner posting second. X/LinkedIn stay manual | Each new adapter proves identity, threading, suppression, exact approval, delivery reconciliation, and platform permission |
| 4. Jurisdictions | Known US business recipients who requested contact or accepted an introduction; unknown location blocks proactive contact. Other outbound jurisdictions wait | A buyer cohort needs expansion and its recipient/channel rules are separately reviewed |
| 5. Trust | Level 0 at launch. Four weeks plus the delivered-message evidence and explicit owner grant may qualify level 1; five-item batches first. Level 2 is deferred | A qualified batch gate and measured review record justify broader scope; calendar time alone never does |
| 6. Bell | Earned, attributed, settled sale with agreed delivery/acceptance evidence; once per sale. Pilot agreement updates the board | A new product changes settlement or acceptance semantics; reconcile refunds and retained share |
| 7. Site | Prefer a surveyed parcel farther west of Lantern Road, facing Civic Hall; use a surveyed off-trail market parcel if it fails | Bob validates full geometry, access, terrain, and clearance; Agora construction never gates R1 |
| 8. Visibility | Private live amounts and bell events by default. Shared instances get labeled demonstration data or reviewed, delayed aggregates | The owner explicitly publishes a particular projection after privacy review |
| 9. Voice | Written-only agents at launch; humans conduct booked demos. Consider agent participation only in specifically requested, disclosed, human-supervised meetings | A separate capability covers recipient permission, recording/transcription, retention, and applicable call rules; no AI cold calling |
| 10. Human seller | Owner receives qualified briefs and owns closing until a collaborator privately agrees and accepts a bounded assignment | Agreed availability, scope, authority, compensation, and customer demand justify the role |

For question 10, each private handoff contains the lead ID, permission/source,
workflow and decision maker, current tools, measured baseline or its unknown
status, data boundary, cited claims, proposed demo/pilot and acceptance
criteria, next action, and review date. Paul routes it only after the human
accepts. The human owns discovery, relationship, agreed price/terms, and
closing; engineering owns technical delivery and the customer owns
acceptance. Agents continue research and drafting within their assignments.
The brief grants no mailbox, calendar, payment, or broader lead access.
Keep compensation and referral agreements separate, as the
[roadmap requires](revenue-roadmap.md#sales-and-pilot-operations).

Voice is deferred partly because AI telephone voices fall within the FCC's
artificial/prerecorded-voice rules; covered telemarketing calls require prior
express written consent absent an applicable exemption. This doesn't
establish a universal rule for web meetings. Any later voice scope gets its
own review. [FCC ruling 24-17](https://docs.fcc.gov/public/attachments/FCC-24-17A1.pdf).

## Evidence behind the decisions

The [transcript guide](../transcripts/README.md) distinguishes historical
intent from shipped behavior. [239](../transcripts/239.md) and
[247](../transcripts/247.md) support buyer demand, partner fulfillment, and
honest public reporting. The [Coder-era review](revenue-roadmap.md#what-the-background-changes)
covers 275–289: daily usefulness, dependable execution, measured outcomes,
and contributor economics. [284](../transcripts/284.md) informs the decision
to reward verified work rather than invented activity. The October 6
operator conversation informs assisted pilots and a possible human sales
role; expressions of interest aren't agreements.

`docs/teardowns/` was removed in the September 18 reset. These links pin its
last retained tree, `8f84d05896ef14edee491621bf977ee5315cc8ed`. The reports
are July–August historical audits, not current product qualification. The
following applications are planning inferences:

- [Hermes](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/teardowns/2026-08-01-hermes-agent-desktop-teardown.md)
  exposes channel verification burden and varying approval behavior. Start
  with one sending channel and one host authority contract.
- [Executor](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/teardowns/2026-07-12-executor-architecture-teardown.md)
  keeps credentials behind trusted handles; safety annotations aren't
  authority. Keep mailbox credentials in the host and bind approvals to
  exact actions.
- [Macro](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/teardowns/2026-08-10-macro-teardown.md)
  excludes email sending from its foreign MCP projection and uses scoped
  entity access. Give a collaborator an accepted assignment, not implicit
  access to the full sales account.
- [Linear Agents](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/teardowns/linear-agents.md)
  distinguishes human ownership from delegation and builds automation on
  reliable manual work. Keep human closing and promote from outcomes.
- [Amp](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/teardowns/2026-07-16-amp-code-teardown.md)
  distinguishes unlisted sharing from privacy. Hide live revenue and deal
  timing across shared instances.
- [Buzz](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/teardowns/2026-07-21-buzz-teardown.md)
  separates participation, execution, acceptance, and settlement. Bind the
  bell to earned delivery and payment rather than activity or funding.
