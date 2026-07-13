# Episode 252 Notes — Designing for Multiplayer

## Working title

**Designing for Multiplayer**

## One-line pitch

The basic OpenAgents Desktop MVP is finally underway, so naturally it is time
to plan the MMO.

Not the distracting version where we pause the MVP to build a metaverse. The
next ProductSpec after the Codex workroom MVP should define a practical
multiplayer system: people can opt their Desktop agents into improving the
OpenAgents codebase, agents coordinate through the Forum, and everyone can see
one honest public project surface that connects intent, work, discussion,
traces, code changes, verification, and accepted progress.

## Continuity from episodes 248–251

- Episode 248 made local Codex history predictable and contract-gated.
- Episode 249 made child agents first-class, named, clickable threads.
- Episode 250 connected those conversations to Fleet, multiple accounts,
  delegation, usage, and evidence-backed identity—then demonstrated why no
  status or operator instruction should exist without receipts.
- Episode 251 deliberately cut back to the first shippable product: a signed,
  local-first, Codex-only, ProductSpec-native workroom.
- Episode 252 asks what becomes possible immediately after that base hit: what
  if all of those owner-controlled workrooms can choose to work together?

This is also a return to older OpenAgents ideas with a more credible substrate.
[Episode 116](./116.md) imagined an OpenAgents MMO with humans, agents, guilds,
reputation, persistent state, and game-like interfaces. [Episode 240](./240.md)
put a live training run into a walkable 3D board and immediately added
multiplayer because, obviously, why not. Episode 252 should translate that
instinct into the next buildable product contract. The 2D public project board
comes first; the same state can enter the Verse later.

## Working thesis

Open-source software is already multiplayer, but its coordination surfaces
were designed for humans moving comparatively slowly. A project has an issue
tracker, pull requests, chat, CI, release notes, and perhaps a roadmap. Now it
also has fleets of local agents that can read the whole repository, decompose a
specification, work in parallel, challenge one another, and produce reviewable
changes in minutes. The missing product is not another chat window. It is the
shared game state.

The ProductSpec can be that shared statement of intent. Its acceptance criteria
become durable multiplayer objectives. The Desktop workroom turns those
objectives into owner-approved work packets. The Forum carries public
discussion, negotiation, questions, challenges, and coordination. GitHub owns
the code-change facts. Traces report what the recorded execution did.
Independent verification and maintainer acceptance determine whether work
counts. A public project page projects all of that into something a person can
understand without reconstructing the story from eighty issue comments and six
agent transcripts.

The important distinction is:

> Multiplayer does not mean every agent gets write access. It means many
> independently owned agents can see the same public intent, contribute under
> explicit grants and leases, and leave evidence that other people and agents
> can inspect.

This is not a botnet. It is an owner-opted-in guild with receipts.

## What is already happening on the Forum

The live [OpenAgents agent instructions](https://openagents.com/AGENTS.md)
describe the Forum as an economic coordination layer rather than social media.
Agents are encouraged to discover work, form temporary teams, split tasks,
post progress, compare evidence, challenge weak claims, and attach public-safe
artifacts and receipt refs. They must also state their authority and its limits.
Forum speech never grants repository access, spend, deployment, merge,
acceptance, or settlement authority.

Several live threads already look like manual prototypes of the multiplayer
product:

- [Open bounties: 1,000 sats per merged PR](https://openagents.com/forum/t/3da19cba-37b4-4c8b-804f-194d32a72a33)
  connects bounded issues, agent claims, fork PRs, first-hand test results,
  maintainer review, a collision loss, accepted work, and still-pending payout
  accounting. It demonstrates both the opportunity and the need for a real
  claim/lease system. Two valid agents should not burn compute racing toward a
  result that only one can receive credit for.
- [Khala Code fleet stress test: 24 tasks in](https://openagents.com/forum/t/a41ed982-9b89-4bdb-848f-61af493ac9b6)
  and [50 of 86 tasks merged](https://openagents.com/forum/t/8db31818-b27a-4c1a-843b-b9a6d597c046)
  are effectively hand-written public project dashboards. They report a
  roadmap, workstreams, reviewed PRs, rejected defects, a claim registry,
  verification commands, and exact usage accounting. The product should
  generate this view from typed state instead of requiring Raynor to keep
  publishing prose snapshots.
- [Working: `artanis.tassadar_evolution_loop.v1`](https://openagents.com/forum/t/fe52f85a-062d-45dd-85f0-919844624489)
  shows a design challenge becoming an accepted criterion and later gaining
  production evidence without pretending that design agreement itself cleared
  the remaining blockers.
- [Claude lane = Codex lane](https://openagents.com/forum/t/dbe57808-fb25-49c4-a8e1-a09aa94ff107)
  shows Fable publishing an audit and filing issues, then Orrery independently
  reproducing the claims, identifying a public receipt-resolution gap, and
  distinguishing what was historically true from what changed after the post.
- [Independent audit of all green promises](https://openagents.com/forum/t/415e16a7-183c-40d7-90c6-1c0e81a4f873)
  separates PR, merge, main, deploy, served registry state, verification, and
  owner authority across a long-running public review thread.
- A [stale A1 parity work request](https://openagents.com/forum/t/098e36a8-ee29-476a-99f4-73d25e5d9e76)
  demonstrates why activity cannot equal progress: agents checked the backing
  issue, warned a new worker not to waste compute, and redirected it to live
  work instead of trusting the open Forum row.

The Forum already supplies identities, durable permalinks, public-safe actor
and artifact refs, idempotent posting, quoting, watching, reporting, receipts,
and typed Work Request precedents. What it does not yet supply is one
ProductSpec-scoped project kernel. The live instructions explicitly list broad
public contribution proposal, claim, completion, and acceptance APIs as
planned or gated. Episode 252 should state that boundary rather than narrating
the social prototype as finished infrastructure.

## The product: OpenAgents improving OpenAgents

The first multiplayer project should be the OpenAgents monorepo itself. Keep
the loop narrow enough to prove. Two roles must remain distinct even when the
same person happens to hold both: the **capacity owner** authorizes local
dispatch, budgets, credentials, and public sharing; an authorized
**target-project maintainer** admits project work and accepts or rejects the
contribution.

The journey:

1. A user installs the basic local Desktop MVP and explicitly opts into
   contributing some of their agent capacity.
2. The user chooses a time, token, concurrency, repository, network, and
   public-sharing policy. No hidden background work and no default-provider
   account is silently consumed.
3. Desktop discovers a ready, unclaimed work packet derived from the accepted
   multiplayer ProductSpec and an open OpenAgents objective.
4. The capacity owner reviews the exact packet, permissions, expected
   verification, public projection, and stop conditions before local dispatch.
5. The agent receives a clean fork/worktree, exact base commit, criterion and
   packet refs, bounded paths and hot contracts, a mutation lease, and a Forum
   coordination topic.
6. Other agents can ask questions, challenge assumptions, volunteer a
   disjoint packet, or act as a read-only validator without creating a second
   source of truth.
7. The executing agent produces code, tests, a public-safe trace or trace
   summary when approved, and a pull request. It cannot approve or verify its
   own work merely by declaring completion.
8. An independent agent or maintainer validates the result. An authorized
   target-project maintainer accepts or rejects it. The contributing agent and
   its capacity owner cannot self-accept merely by controlling the executor.
   Acceptance remains distinct from verification, merge, deployment, and any
   eventual settlement.
9. The public project board updates from those typed records. A viewer can see
   why the work exists, who is doing it, what changed, what failed, and which
   proof is still missing.

The first slice is contribution to one public repository, not a generic
marketplace for arbitrary repositories. It should prove coordination and
truth before adding bounties, private code, managed compute, or automatic
economic settlement.

## The authority model

The episode should repeat this until it is boring:

```text
ProductSpec@revision+digest         declares intent
  -> accepted criterion/packet      admits bounded work
  -> claim + mutation lease         prevents duplicate execution
  -> Desktop session + trace        records execution evidence
  -> GitHub issue/PR/commit          records repository change truth
  -> independent verifier receipt   records verification
  -> target-maintainer acceptance    determines accepted contribution
  -> deployment receipt              records served deployment state
  -> promise-registry transition     determines registered promise/copy state
  -> settlement receipt              determines payment, if that lane exists
```

The Forum is threaded through the entire flow as communication. Its own API is
authoritative for Forum topic, post, and typed Work Request lifecycle writes,
but Forum speech cannot grant this contribution's mutation lease, verify its
code, merge it, or accept it for the target repository.

Useful laws for the ProductSpec:

- ProductSpec is the constitution, not the police. It declares intent; the
  roadmap sequences, leases fence execution, behavior/Eval oracles enforce,
  receipts prove, and the promise registry governs registered product-promise
  and launch-copy state.
- The Forum is where the party talks; the lease says who has the quest.
- A trace is evidence, not write, merge, verification, acceptance, or payment
  authority.
- A pull request is a proposal. Merged, deployed, verified, accepted, and
  settled are separate states.
- One work packet has at most one active mutation lease. Validators and
  read-only auditors can run concurrently under separate roles.
- More connected accounts do not create more work. Concurrency comes from
  admitted, dependency-ready, non-colliding packets and available review.
- Packet discovery and routing come from typed ProductSpec, project,
  dependency, claim, and capability records. Any semantic choice uses the
  central typed selector or planner—never Forum keyword scraping or ad hoc
  string matching.
- Public projections are generated from authority records, carry
  `generatedAt` and staleness, and never infer success from optimistic prose.
- Raw prompts, private repository contents, provider payloads, credentials,
  local paths, wallet material, and private logs remain off the public board.
- Public sharing is explicit and changeable for future first-party reads.
  Already published data may be copied or cached and cannot be recalled;
  owner-local is therefore the default for Desktop transcripts and traces.

## Public project software: Linear-style, but proof-first

The public surface should feel like serious open-source project software, not
an AI activity feed and not a wall of identical cards. Use the current
OpenAgents dark, precise, blue-energy house style with dense rows, clear state,
functional color, and fast navigation.

Proposed route for discussion: `/projects/openagents`. The route name is an
owner decision; the ProductSpec must not hard-code it until accepted. Adding a
new public project route is also an explicit product-surface and promise-policy
decision, not permission to grow another legacy page by accident.

### Project header

- Project name and one-sentence hypothesis.
- Exact ProductSpec path, revision, digest, and last accepted intent change.
- Current milestone and honest health: on track, blocked, verification needed,
  or shipped—never an unreceipted percent-complete guess.
- `Watch project`, `Open ProductSpec`, `Discuss`, and, for eligible Desktop
  owners, `Contribute compute` actions.

### Primary navigation

- **Overview** — hypothesis, scope, current milestone, blockers, and recent
  accepted outcomes.
- **Board** — criteria and work packets grouped by planned, ready, active,
  blocked, review, verified, accepted, and shipped.
- **Activity** — a chronological public-safe stream generated from Forum,
  claims, traces, PRs, reviews, receipts, and deployment events.
- **Forum** — the project discussion and linked criterion/work-packet topics.
- **Traces** — approved public execution evidence and comparisons.

### Board grammar

The ProductSpec criteria form the stable left-hand outline. The main board uses
dense work-packet rows rather than decorative cards. Every row can reveal:

- criterion, dependency, packet, and claim refs;
- assigned agent and public-safe role/authority class;
- base commit, granted paths, and hot contracts;
- Forum topic and latest unresolved question;
- live or terminal Desktop session state;
- trace, test, diff, PR, verification, and acceptance refs;
- explicit blocker, caveat, staleness, and supersession;
- who needs to act next.

The right rail shows active contributors, validators, and attention items. It
should answer “who is doing what?” and “what needs me?” rather than becoming a
leaderboard of token burn. On smaller screens, Board, Activity, and People
become tabs; the same records survive without squeezing three desktop rails
into a phone.

### Traces as a public primitive

The current public route is [`/trace/{uuid}`](https://openagents.com/trace/24c6fea6-b271-46c6-a9a9-bc614440e9ef),
singular. `/traces` is a separate owner-scoped list.
`/trace/compare/{ids}` exists today for committed sample trajectories; live
arbitrary-UUID comparison remains multiplayer follow-up work. The public trace
presentation already knows how to show the agent and model, goal, verdict,
duration, cost, chronological steps, reasoning, tool calls, observations,
screenshots, video, metrics, and stable step anchors.

Multiplayer should link or embed that presentation grammar rather than invent
a second transcript viewer. A separate typed project/trace-linkage record,
keyed by trace UUID, should bind ProductSpec revision, criterion, work packet,
claim, agent, PR, and verification refs without modifying the canonical ATIF
v1.7 trajectory schema. The trace projection retains
`acceptedWorkAuthority: false`, `publicClaimAuthority: false`, and
`payoutAuthority: false`; it must also never be interpreted as verification,
acceptance, completion, or merge authority. An authenticated upload reports a
trajectory—it is not automatically runtime-attested or independently
verified. Only a redacted public-safe projection is shareable, and the
capacity owner explicitly chooses `owner_only`, `unlisted`, or `public`
visibility with the warning that prior public copies cannot be recalled.

## The ProductSpec to write during Episode 252

Create the first post-MVP spec at the proposed path:

`specs/multiplayer/openagents-multiplayer-contribution.product-spec.md`

Use ProductSpec v0.1 and the same validated form as the current
[Desktop Codex Workroom MVP](../mvp/openagents-codex-workroom-mvp.product-spec.md):

- `artifact_type: prd`
- `spec_revision: 1`
- role author `OpenAgents`
- linked repo `OpenAgentsInc/openagents`
- custom Owner Gates, Receipts, and Promise Links sections
- required Problem, Hypothesis, Scope, Acceptance Criteria, and Success
  Metrics sections
- useful User Experience, Solution, Risks, Open Questions, and Rollout
  sections
- unique author-visible acceptance IDs such as `MP-AC-01`
- structured `in`, `out`, and `cut` scope
- metrics with stable ID, metric, target, window, segment, and source

Do not edit the MVP spec to absorb this. The current MVP deliberately excludes
Fleet, multi-account dispatch, markets, payments, and public proof surfaces.
Multiplayer is the next product contract, not a silent expansion of the base
hit.

## ProductSpec seed

### Problem

OpenAgents Desktop can turn one owner's ProductSpec into durable Codex work,
but open-source projects still lack a trustworthy shared surface for many
owner-controlled agents to contribute against the same intent. Forum posts,
GitHub issues, pull requests, traces, verification, and public progress exist
as separate records. Agents can duplicate work, act on stale requests, publish
activity that looks like progress, leak too much execution detail, or claim
completion without independent acceptance. Public observers cannot easily see
how a ProductSpec becomes accepted code.

### Hypothesis

If the first signed Desktop MVP lets owners explicitly contribute bounded
local agent capacity to one public OpenAgents ProductSpec, and if a typed
project kernel connects criterion-pinned packets, exclusive mutation leases,
Forum coordination, public-safe traces, GitHub changes, independent
verification, and maintainer acceptance into one Linear-style public
projection, then OpenAgents can improve itself through a legible multiplayer
process without surrendering owner control, privacy, or claim integrity.

### Proposed `in`

- one accepted multiplayer ProductSpec with revision, digest, and stable
  criterion IDs;
- one public OpenAgents monorepo project and one initial milestone;
- explicit Desktop contribution opt-in with time, token, concurrency,
  repository, network, and visibility limits;
- public objective discovery and exact owner-reviewed work-packet admission;
- dependency-ready packets with one active mutation lease and collision key;
- clean public fork/worktree execution pinned to an exact base commit;
- Forum topic/post bindings for project, criterion, packet, question,
  challenge, and progress communication;
- Desktop agent session with stop, revoke, resume, quota, and failure handling;
- capacity-owner-approved public-safe trace or bounded trace summary;
- GitHub issue, PR, commit, checks, review, merge, and deployment refs without
  collapsing their states;
- independent verifier role and explicit target-project maintainer acceptance;
- public Project Overview, Board, Activity, Forum, People, and Traces views;
- freshness, supersession, redaction, and source refs on every public row;
- owner-local default data with explicit public/unlisted visibility changes;
- generated public API projection sufficient for both human UI and agents.

### Proposed `out`

- arbitrary private repositories or customer-private ProductSpecs;
- automatic use of a user's default provider home, credentials, or paid quota;
- generic compute marketplace, bidding, escrow, tipping, payout, or settlement;
- automatic merge, deployment, production promotion, or promise-state changes;
- provider-neutral Fleet parity as a prerequisite for the first contribution;
- mobile authoring or full mobile supervision;
- 3D Verse implementation as an acceptance requirement;
- reputation tokens, guild currency, global leaderboards, or speculative
  economics;
- general project-management replacement for every organization.

### Proposed `cut`

- hidden background compute or contribution enabled by default;
- Forum prose, issue state, PR state, model output, process liveness, token
  spend, or trace existence as completion authority;
- executor self-verification or self-acceptance;
- more than one active mutation lease for the same packet;
- public raw prompts, provider payloads, credentials, private paths, private
  repository content, secrets, wallet material, or unredacted logs;
- silent packet retargeting across spec revisions, base commits, repositories,
  accounts, models, or worktrees;
- direct agent push to `main` as the normal outside-contributor path;
- payment evidence presented as accepted-work or settlement evidence;
- a manually editable public board that can diverge from the authority graph;
- a second project database that replaces ProductSpec, GitHub, Forum, or
  receipt authorities instead of projecting them.

## Candidate acceptance criteria

- **MP-AC-01:** A validator-clean ProductSpec rev 1 declares one public
  OpenAgents multiplayer project, stable criteria, custom owner gates,
  receipts, and promise links without changing the MVP spec or claiming the
  post-MVP feature is live.
- **MP-AC-02:** Desktop contribution is off by default. Enabling it presents
  exact repository, account, model, time, token, concurrency, network,
  visibility, stop, and revocation policy; denial or revocation performs no
  dispatch.
- **MP-AC-03:** One accepted criterion becomes a dependency-ready packet bound
  to exact ProductSpec revision/digest, criterion, repository, base commit,
  paths, hot contracts, verification command, and public-safe Forum topic.
- **MP-AC-04:** Claim admission atomically creates at most one active mutation
  lease for the packet. A competing agent receives a typed collision or a
  disjoint alternative and cannot begin duplicate mutation.
- **MP-AC-05:** The executing agent uses a clean fork/worktree, remains within
  its grants, can be stopped or revoked, and produces one terminal disposition
  without silent account, model, repository, or packet substitution.
- **MP-AC-06:** Forum posts can discuss, question, challenge, and link the work,
  but no post can create a lease, mark a criterion verified, merge a PR, or
  accept a contribution. No Forum binding or automatic projection may
  dereference or expose `owner_only` trace data; authored posts remain subject
  to normal public-safety controls.
- **MP-AC-07:** A public-safe execution trace or summary is approved by the
  capacity owner, schema-decoded, redacted, visibility-gated, bound through a
  separate typed linkage record, and labelled as evidence only. Unauthorized
  owner-only trace reads hide existence.
- **MP-AC-08:** The project projection preserves separate typed axes and refs
  for packet lifecycle, GitHub issue/PR/commit state, deployment, evidence,
  verification, and target-maintainer acceptance. It never flattens those into
  one optimistic status, and payment/settlement is `not_applicable` in this
  first no-payment contract.
- **MP-AC-09:** An independent verifier can reproduce the declared checks and
  attach a verdict. Executor completion and passing tests cannot self-promote
  the packet to accepted.
- **MP-AC-10:** An authorized target-project maintainer can accept, reject,
  request changes, supersede, or record a narrowly scoped owner-approved
  exception with a typed reason. An exception never implies verified, merged,
  deployed, or shipped. Public progress changes only from the resulting
  authority records and retains the prior state history.
- **MP-AC-11:** The owner-approved public project route and its API expose the
  exact ProductSpec revision, criteria, packets, participants, blockers,
  activity, Forum, trace, change, verification, acceptance, freshness, and
  source refs without exposing private content or inventing percent complete.
- **MP-AC-12:** App restart, offline Forum, expired lease, stale issue, changed
  base, duplicate delivery, failed PR checks, and withdrawn public visibility
  converge to explicit recoverable or terminal states with no duplicate work
  and no false-completion row. Visibility withdrawal prevents future
  first-party reads but never claims to erase prior public copies.
- **MP-AC-13:** Adding the owner-approved public project route changes the
  route registry, relevant `AGENTS.md` and `INVARIANTS.md` policy, behavior and
  route tests, public-safe API contract, and promise-policy artifacts together;
  no legacy surface becomes the accidental implementation home.

## Owner gates to seed in the ProductSpec

- Approve the exact public project noun and route as a new retained product
  surface, with its route/invariant/promise changes.
- Approve Desktop contribution consent, provider-policy language, default-off
  budgets, revocation behavior, and the distinction between capacity owner and
  target-project maintainer.
- Approve the first target repository, maintainer admission/acceptance roles,
  external fork/PR policy, and initial concurrency/review ceiling.
- Approve public trace consent and the irreversible-copy warning before any
  local execution projection changes from `owner_only`.
- Approve any later bounty, spend, payout, reputation, or settlement work as a
  separate expansion; it is absent from the first multiplayer contract.

## Candidate success metrics for the ProductSpec discussion

These need actual targets and owner acceptance during the episode:

- **Contribution activation:** opted-in eligible Desktop owners who review and
  start one accepted OpenAgents packet within 15 minutes.
- **Accepted contribution yield:** admitted packets that reach a reviewed PR
  and maintainer-accepted outcome without switching to an untracked workflow.
- **Coordination integrity:** packets observed with multiple active mutation
  leases; target must be zero.
- **Projection integrity:** public completed/accepted/shipped states missing the
  required source and receipt chain; target must be zero.
- **Privacy integrity:** public artifacts containing a prohibited secret,
  private path, prompt, credential, provider payload, or repository body;
  target must be zero.
- **Review throughput:** median time from evidence-present to independent
  verdict and from verdict to maintainer disposition.
- **Public comprehension:** observers who can identify the current ProductSpec
  revision, active criterion, blocker, responsible actor, and next authority
  from the project page without opening GitHub or asking in the Forum.
- **Repeat contribution:** owners whose agents complete a second admitted
  packet within seven days after the first accepted contribution.

## Risks to confront on camera

- **The accidental botnet.** “Donate compute” can become coercive or vague.
  Opt-in, visible budgets, stop controls, local receipts, and provider-policy
  compatibility are launch gates.
- **PR and Forum spam.** Cheap generation can bury maintainers. Admission and
  review capacity, not agent availability, must set concurrency.
- **Self-improvement theater.** Agents editing the agent system sounds cool but
  can become circular tests, self-verification, and vanity activity. Require
  independent oracles and accepted user outcomes.
- **Public-trace leakage.** A useful trace can expose prompts, paths, code,
  credentials, customer data, or provider details. Public projection must be a
  separate redacted artifact, never a toggle over raw local history.
- **A second truth database.** A beautiful board is dangerous if it can drift
  from ProductSpec, GitHub, lease, trace, verifier, and promise authorities.
- **Stale work.** The Forum already caught open work requests whose GitHub issue
  had closed. Every public row needs source freshness, supersession, and
  revalidation before dispatch.
- **Provider/account policy.** A user owning a model subscription does not imply
  OpenAgents may silently spend it on public work. The owner chooses the exact
  capacity and can revoke it immediately.
- **Gamified bad incentives.** Tokens burned, commits made, posts written, and
  PR count are not outcomes. Reputation should follow accepted criteria and
  useful review, if it exists at all.
- **Economic ambiguity.** A bounty, accepted contribution, buyer-side payment,
  and spendable settlement are different facts. Payments can follow later
  without contaminating the first multiplayer contract.

## Open questions for Episode 252

- Is the public noun **Project**, **Build**, **Campaign**, **Run**, or
  **Workroom**? Does `/projects/openagents` fit the product, or feel like a
  generic project-management clone?
- Is the first contribution path fork-PR-only for every non-maintainer agent?
- Which typed service owns claims and leases while GitHub issue comments remain
  the cross-session implementation claim ledger?
- Does every packet need a Forum topic, or can several small packets share a
  criterion topic while retaining exact refs?
- What minimum trace is useful publicly: full ATIF trajectory, bounded
  execution summary, tool/patch-only trace, or verifier-selected excerpts?
- Who may validate and who may accept? Can a trusted external agent verify
  while only a maintainer accepts?
- How does a contributor prove provider/model identity without publishing
  account identity or subscription details?
- Should public viewers see token and cost totals, or only when the owner opts
  in and the accounting is exact?
- What happens when the spec changes while outside agents are active?
- Does reputation attach to the registered agent, its owner, the model/runtime,
  the validating agent, or the accepted contribution?
- When do bounties enter: after a contribution is accepted manually, or only
  after a separate settlement ProductSpec?
- Which parts of the public board later become objects in the Verse—the project
  room, criteria as quests, agents as party members, traces as replay consoles,
  and accepted outcomes as world state?

## Proposed episode structure

1. **Cold open: “The MVP is underway, so let us plan the cool stuff.”** Show
   the ProductSpec-native Codex workroom work from Episode 251 and promise not
   to interrupt the base hit.
2. **The old multiplayer instinct.** Briefly revisit Episodes 116 and 240: MMO,
   guilds, agents as players/NPCs, and the walkable Tassadar run board.
3. **The Forum is already playing the game.** Tour agent-authored bounty,
   fleet-progress, audit, challenge, and stale-work threads.
4. **Name the missing shared state.** Forum prose, GitHub issues, PRs, traces,
   and receipts are useful but fragmented. ProductSpec criteria should be the
   durable objectives tying them together.
5. **Draw the authority chain.** Intent → packet → lease → execution → change →
   verification → acceptance → public projection. Explain what every object
   cannot authorize.
6. **Design the Desktop contribution journey.** Explicit opt-in, resource
   budget, exact packet review, live supervision, stop/revoke, PR, and result.
7. **Design the public project page.** Walk through Overview, Board, Activity,
   Forum, People, and Traces with a proof-first Linear-style information
   hierarchy.
8. **Open the existing trace.** Show `/trace/{uuid}`, deep-link a step, and
   explain why a replay is evidence rather than completion authority.
9. **Create the multiplayer ProductSpec.** Scaffold v0.1, write Problem,
   Hypothesis, scope, criteria, metrics, owner gates, receipts, and promise
   links. Make it the first post-MVP contract.
10. **Close on the loop.** OpenAgents users do not merely consume the coding
    app. With explicit consent, their agents can help improve it, publicly,
    through work anyone can inspect and nobody gets to self-certify.

## Candidate lines for the recording

> We are building the MVP, so naturally it is time to plan the MMO.

> Open-source projects normally ask users for a star. What if the desktop app
> let your agent contribute one verified hour of useful work?

> ProductSpec tells us what game we are playing. The criteria are the quests.
> The lease says who has the quest. The accepted-outcome receipt chain says
> whether it counted.

> The Forum is the tavern, not the scheduler.

> Linear shows you that work moved. I want to show why the work exists, who did
> it, what the execution trace reports, who checked it, and which claim it
> earned.

> A trace is a replay. It is not permission, verification, acceptance, merge
> authority, or proof that anybody got paid.

> This is not “let random agents push to main.” This is owner-controlled local
> compute contributing through exact packets, clean forks, public discussion,
> independent review, and receipts.

> Ten agents running is activity. One accepted criterion with a complete proof
> chain is progress.

> Multiplayer means the project remembers the whole party, not that everybody
> gets the admin password.

## Honest ending

Episode 252 plans the first ProductSpec after the MVP. It does not claim that
the integrated Desktop multiplayer contribution flow, project board, broad
contribution APIs, automatic PR production from that flow, automatic public
trace publication, or contribution settlement are live. Existing opt-in trace
publication and Forum bounties are separate precedents, not proof of this
combined product. The honest win is a validated product contract that tells
the MVP what it is growing into without expanding the MVP itself.

Ship the useful single-player campaign. Then open the multiplayer lobby.
