# Operation Diamond Hands — the Bitcoin OSS hardening program as a Nostr-native public project

- Date: 2026-08-04
- Class: architecture spec and initial roadmap
- Status: stood down by owner direction on 2026-08-04. Preserved for reuse;
  authorizes no deployment, publication, admission, or continued forensics
  work.
- Owner: OpenAgents
- Companion reading:
  [`../loupe/2026-08-01-coordination-not-scanners.md`](../loupe/2026-08-01-coordination-not-scanners.md)
  (the argument), [`../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md`](../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md)
  (what to hunt), [`../nips/PROPOSED.md`](../nips/PROPOSED.md) (the All Work
  NIP program), transcripts [263](../transcripts/263.md),
  [264](../transcripts/264.md), [265](../transcripts/265.md).

## 1. What this document is for

> **Stand-down, 2026-08-04.** The owner cancelled Operation Diamond Hands and
> the surrounding forensics effort before production deployment or relay
> publication. The implemented OT/PG client, GPUI/wasm surface, browser network
> proof, and route infrastructure remain on `main` for later reuse. `/dh` must
> not be deployed and the roadmap below must not resume without a new owner
> decision.

Episodes 263–265 established three things:

1. A hardware wallet lost ~594 BTC to a defect that sat in public source for
   years, and nobody was systematically looking for that class of failure.
2. The best available scanner would have missed it in its default
   configuration, and did miss it in our pre-registered experiment's arm A —
   while finding it three times in arm B, where the submodules were on disk.
   **The finding was the divergence between two configurations.**
3. The missing layer is therefore not a better scanner. It is coordination:
   a public record of what has been examined and how, shared configurations,
   divergence detection, a room to work in, and somebody paying for compute.

This document specifies how to build that coordination layer as **Operation
Diamond Hands**, a public project living in our own Nostr relay, organized
with the OpenAgents NIP program, projected read-only onto the web, and worked
through Omega.

The first delivery target, stated as a demo rather than a claim:

> `/dh` shows a basic project page for **Operation Diamond Hands**: its signed
> project identity, current status, latest authored update, and recent public
> activity. The browser itself opens a Nostr WebSocket connection to
> `wss://relay.openagents.com`, reads the initial snapshot through `REQ` and
> `EOSE`, and stays subscribed for new events. The page does not need an
> OpenAgents HTTP projection, a private database, or a server-side WebSocket
> proxy to render its project data.
>
> **The surface is Omega's own GPUI components compiled to WebAssembly, not a
> separately authored web app.** Owner decision, 2026-08-04: the first
> deliverable is Rust and GPUI end to end — one component set, one client, one
> language, from the workbench to the browser tab. See
> [Addendum A](2026-08-04-gpui-on-web-addendum.md) §11 for the working test
> this decision rests on, and §12 for the costs it accepts.

## 2. What already exists

Facts, with paths, so the roadmap starts from the real substrate.

| Piece                      | State today                                                                                                                                                                                                                                        | Where                                                                                                     |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| The relay                  | **Live.** `relay.openagents.com` serves Immortal 0.0.1 as of 2026-08-04, NIP-11 reporting NIPs 1, 9, 11, 17, 29, 40, 42, 45, 50, 65, 70, 94 plus eleven Block extensions                                                                           | `~/work/immortal`, `PROVENANCE.md`                                                                        |
| Relay write policy         | `restricted_writes: true` — closed membership via `relay_member_pubkey`; agents can ride their owner's membership through Block NIP-AA                                                                                                             | `immortal/src/store/mod.rs:457`, `src/gateway/server.rs:718`                                              |
| Relay admin path           | NIP-86 management API **not configured in production** (86 absent from live `supported_nips`); no media endpoint configured either                                                                                                                 | live NIP-11                                                                                               |
| The NIP program            | 25 All Work NIPs drafted across five layers                                                                                                                                                                                                        | [`../nips/PROPOSED.md`](../nips/PROPOSED.md)                                                              |
| Signed workroom projection | **Implemented**, kinds 32150–32163 pinned, prepare/commit signing lane, persist-before-publish outbox                                                                                                                                              | `packages/all-work-contract/src/signed-workroom-*.ts`, [`../nips/WA.md`](../nips/WA.md)                   |
| Contract → SDK generator   | **Implemented**: one pinned JSON definition emits Effect Schema, a TypeScript client, Rust types, JSON Schema, fixtures, and a digest-bound compatibility manifest with drift checking                                                             | `packages/all-work-contract/scripts/generate.mjs`                                                         |
| Nostr client code          | `packages/public-nostr-chat` (relay client, subscribe/snapshot, remote signer); `nostr-effect` sibling repo is the shared Effect implementation                                                                                                    | `packages/public-nostr-chat/src/client.ts`, `~/work/nostr-effect`                                         |
| Forensic workbench         | Implemented in the **Omega** repo (Rust/GPUI) with an Effect Schema boundary and Loupe adapter in this monorepo                                                                                                                                    | `~/work/omega/crates/omega_forensics/`, `packages/forensic-contract/`, `packages/forensic-loupe-adapter/` |
| Coldcard evidence          | Pre-registered experiment, results, generator reproduction, evidence graph, historical fingerprint scan                                                                                                                                            | `docs/loupe/`, `docs/coldcard/`, `fixtures/forensics/coldcard/`                                           |
| Web app                    | Cloud Run Node monolith. Reusable `/dh` GPUI/wasm source, assets, route mapping, isolation headers, and browser proof landed locally and on `main`, but the owner stood the program down before production deployment.                             | `apps/openagents.com/apps/diamond-hands/`, `workers/api/src/cloudrun/start-ui.ts`                         |
| GPUI on the web            | **Proven 2026-08-04.** Omega's real `ui` design system and Aiur theme render in a browser through `gpui_web` + `gpui_wgpu` → WebGPU. Four defects found and worked around; filed as [omega#243](https://github.com/OpenAgentsInc/omega/issues/243) | `~/work/omega/crates/gpui_web/`, [Addendum A §11](2026-08-04-gpui-on-web-addendum.md)                     |
| Sats payout                | **No live rail.** MDK/Nexus money authority retired under VP-1; payout/L402/credit routes stripped from the served registry; LDK exists as typed readiness projections only                                                                        | root `INVARIANTS.md`, `workers/api/src/index.ts:13341`, `pylon-ldk-readiness-projections.ts`              |

Two of these are load-bearing constraints rather than conveniences, and the
roadmap in §8 is shaped around them: **the relay is closed-write**, and
**there is no live payout rail**.

## 3. Design principles carried in from the evidence

1. **Publish coverage, not just findings.** The Coldcard trap was invisible
   because no two scans were ever comparable. A scan that reports twelve
   findings on a program it read a third of is making a completeness claim it
   has not earned. Completeness is therefore a first-class, signed record —
   not a footnote in a report.
2. **Pre-register before running.** Our own experiment was credible because the
   scoring rubric was written, digested, and pushed _before_ the run. On a
   relay this becomes trivial and universal: publish the profile and rubric
   digest first, then the result. Goalposts cannot move.
3. **Divergence is the product.** Two runs over the same target that disagree
   are a lead. This is free signal today and nobody collects it, because
   nobody publishes runs.
4. **Attested absence is dual-use.** A public coverage map tells defenders
   where to look and attackers where nobody is looking. §7 treats this as the
   program's first hard design question, not a footnote.
5. **A signature proves bytes, never authority.** Inherited from the whole NIP
   program: relay acceptance is transport; findings are candidates until
   independently verified; verification is not acceptance; acceptance is not
   disclosure; disclosure is not payment.
6. **Money last, and never per-vulnerability by default.** The coordination
   analysis argued explicitly for keeping money out until someone can
   articulate why it must be in. §6.5 takes that seriously and proposes paying
   for _coverage and verification work_ rather than running a bounty market.

## 4. The program as All Work objects

The hardening effort is not a new object model. It is the existing All Work
model pointed at security work, which is why the NIP program was worth
drafting first.

```text
Organization: OpenAgents Hardening Program            (NIP-OT 32100)
└── Team: defenders / verifiers / maintainer-liaison  (NIP-OT 32101)
    └── Initiative: Harden Bitcoin OSS against AI-assisted attack
        └── Project: one per target ecosystem or repo  (NIP-PG 32222)
            ├── Work: one per assessment unit          (NIP-WK 32170)
            │   ├── Issue projection (list/board view) (NIP-PI 32200)
            │   ├── Code Context: repo + pinned commit + submodule set
            │   │                                      (NIP-CC 32310)
            │   ├── Repository Work Claim: who is scanning this now
            │   │                                      (NIP-RC 32301)
            │   ├── Agent Session + Activity: the scan run
            │   │                             (NIP-AS 32280 / NIP-AV 32290)
            │   ├── Evidence + Verification: what was produced and checked
            │   │                             (NIP-EV 32190 / 32191)
            │   └── Owner Disposition: accepted / rejected  (NIP-EV 32192)
            └── Workroom Binding → NIP-29 group          (NIP-OT 32104)
```

Concretely:

- **A target repository** is a NIP-34 repository announcement (`kind:30617`)
  plus a NIP-PG Project. NIP-34 already exists in the official lane and needs
  no relay support beyond ordinary event storage — Immortal serves it today.
- **A scan** is an Agent Session (NIP-AS) with its Coding Session companion
  (NIP-CC) pinning the exact commit and — critically — the exact materialized
  source set.
- **A candidate finding** is Evidence (NIP-EV `32190`); an independent
  second-opinion pass is a Verification Receipt (`32191`) whose signer must
  differ from the producer, which is exactly the Loupe verify-stage discipline
  expressed on the wire.
- **Claiming a target** so two contributors do not burn compute on the same
  scan is a Repository Work Claim (NIP-RC) — the same collision ledger, with
  its evidence heartbeats and 90-minutes-plus-audit takeover rule.
- **Coordination chat** is the NIP-29 group the Workroom Binding names, which
  is what Episode 263 committed to and what the relay already supports.
- **The public tracker view** is the NIP-PI Issue projection: any Nostr client
  can render the program as a board without knowing anything else.

What the Block lane supplies unchanged: **NIP-OA** owner attestations binding
each contributor's agents to them, **NIP-AA** so those agents can write to a
closed-membership relay under their owner's standing, **NIP-AE** for an
agent's private memory, **NIP-AO/NIP-AM** for live telemetry and per-turn cost,
**NIP-AP** for agent personas in the roster, **NIP-RS/ER/PL** for read state,
reminders, and mobile wake-ups.

## 5. Five new NIPs the hardening program needs

The All Work program covers planning, delegation, execution, evidence, and
outcomes. It does not cover five things that this specific program depends on.
The five application drafts reserve **32450–32499**, extending the OpenAgents
addressable neighborhood declared in `PROPOSED.md`, after collision review
against the official, Block, and existing OpenAgents lanes. A draft reserves
wire vocabulary; it does not admit implementation or a product claim.

| NIP    | Name                                     | Reserved block | Draft                    | Why it cannot be folded into an existing NIP                                                                                                  |
| ------ | ---------------------------------------- | -------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| NIP-SP | Scan Profiles and Pre-Registration       | 32450–32459    | [`SP.md`](../nips/SP.md) | A profile is a versioned, shareable, digest-bound _configuration_, and the rubric must be committed before the run                            |
| NIP-SC | Source Completeness and Coverage         | 32460–32469    | [`SC.md`](../nips/SC.md) | The Coldcard lesson: what was actually on disk is a claim that must be signed, checkable, and comparable across runs                          |
| NIP-FD | Findings, Verdicts, and Disclosure       | 32470–32479    | [`FD.md`](../nips/FD.md) | Findings need severity, CWE, embargo state, hash commitments, and coordinated-disclosure lifecycle that generic evidence records do not carry |
| NIP-SI | Security Invariants and Regression Watch | 32480–32489    | [`SI.md`](../nips/SI.md) | The decisive Coldcard control was a build-time assertion about the shipped artifact — a durable property, not a one-time finding              |
| NIP-BT | Bounties and Contribution Credit         | 32490–32499    | [`BT.md`](../nips/BT.md) | Funding pools, credit standing, and (later, if ever) payouts need their own records with hard boundaries against the disclosure process       |

### 5.1 NIP-SP — Scan Profiles and Pre-Registration

**Records.** A _Scan Profile_ (`32450`): named, versioned, digest-bound
configuration — source materialization rules (submodules, vendored trees,
lockfile-pinned deps), file selection, attack-surface ranking policy, hunt
classes, model and harness roles, budget bounds, and evidence requirements. A
_Pre-Registration_ (`32451`): published **before** a run, binding the target,
the pinned commit, the profile digest, the hypothesis, and the scoring rubric
digest, with an expiry.

**Why it matters.** Our own credibility in Episode 264 came from a rubric
frozen before the run. On a relay, pre-registration is one event and it is
verifiable by anyone: the run's later result event references the
pre-registration, and a rubric that changed between them is mechanically
detectable. It also makes profiles the transfer mechanism the coordination
doc asked for — "Bitcoin firmware: materialize submodules, rank entropy and
key-derivation paths first, hunt these classes" becomes a shareable object
with a version and an author, not tribal knowledge.

**Composition.** A profile is close kin to NIP-GB Guidance and NIP-SKL Skills;
the difference is that a profile is a _measurement instrument_ whose exact
bytes must be citable by a result. It references guidance and skills rather
than replacing them.

### 5.2 NIP-SC — Source Completeness and Coverage

**Records.** A _Materialized Source Set_ (`32460`): for one scan, the exact
tree that was actually readable — repository refs, pinned commit, submodule
paths declared versus populated, vendored trees, dependency lockfile digests,
file count and byte count analyzed versus skipped and why. A _Coverage
Attestation_ (`32461`): the durable public statement that this target, at this
commit, was examined with this profile to this depth — including
`completeness: complete | partial | degraded` and the reason.

**Why it matters.** This is the single highest-value record in the program,
and it comes straight from the measurement: `git clone --bare` cannot fetch
submodules, the string `submodule` appeared nowhere in Loupe's crate tree, and
an operator scanning Coldcard the week before the theft would have gotten a
productive-looking twelve findings over a third of the program with **nothing
in the output saying so**. A signed completeness record makes that failure
loud, comparable, and — once several parties publish — socially obvious long
before it is technically obvious.

**Hard rule the NIP must state.** A result event that does not reference a
Materialized Source Set is not a scan result; it is an anecdote. Clients render
it as such.

**Divergence.** Because attestations are comparable by
`(target, commit, profile)`, divergence detection is a query, not a product:
two attestations over the same target with different completeness or different
finding sets is a lead, published as a _Divergence Note_ (`32462`) that names
both runs and what differed.

### 5.3 NIP-FD — Findings, Verdicts, and Disclosure

**Records.** A _Finding Commitment_ (`32470`): a digest-bound commitment
published at discovery, revealing nothing. A _Candidate Finding_ (`32471`):
target, mechanism, severity proposal, CWE, evidence boundary, assumptions —
encrypted to the disclosure audience while embargoed. A _Finding Verdict_
(`32472`): an independent judgment (`confirmed`, `refuted`, `inconclusive`)
from a signer other than the producer. A _Disclosure State_ (`32473`):
reported-to-maintainer, acknowledged, fix-available, published, or withdrawn,
with embargo expiry.

**Why the commitment is load-bearing.** It is the primitive that makes
cooperation cheaper than racing. Two teams working the same bug learn they are
on the same bug — because the commitments match on reveal — **without either
revealing it first**, which solves credit and duplicated spend with no trusted
intermediary. One event, and it is the difference between a coordination group
and a race.

**Boundaries the NIP must fix.** Publication is gated: a Candidate Finding
becomes public only through the disclosure state machine, never because an
embargo timer expired on its own, never because a model sounded confident, and
never on a public relay before the maintainer path has been attempted. This is
the mechanism that separates this program from the "point Kimi at a repo and
post the list on X" behavior Episode 265 criticized.

### 5.4 NIP-SI — Security Invariants and Regression Watch

**Records.** A _Security Invariant_ (`32480`): a named property that must hold
across source, configuration, build, artifact, and runtime, with its failure
consequence, required witnesses, and falsifiers. An _Artifact Provenance
Witness_ (`32481`): evidence binding a built artifact to the exact source,
symbols, or configuration an invariant requires. A _Regression Watch_
(`32482`):
a recurring check that re-evaluates invariants against new target revisions,
with freshness and stopping rules.

**Why it matters.** The decisive Coldcard control was not a source finding at
all. It was the vendor's eventual build-time assertion — run `nm` on the
compiled objects, fail unless the board's `rng_get` won the link. That is an
invariant about a shipped artifact, and it is exactly the class of check that
"scan the source once" can never express. Codifying invariants makes the
program's output durable: a finding fixes one bug, an invariant prevents its
whole family across every revision that follows, and a watch tells you when a
target regresses.

**Seed set.** The ten hunt classes in the hardening analysis — entropy
provenance, nonce generation, silent security downgrades, uncalled security
code, build-versus-source divergence, parser memory safety, signature and
validation skips, update and downgrade paths, side channels, and dependency
substitution — are the first invariant families, ranked by the free-oracle
property rather than by CVSS.

### 5.5 NIP-BT — Bounties and Contribution Credit (drafted, postponed)

**Records.** A _Funding Pool_ (`32490`): a sponsor's committed budget for a
named campaign, scope, and period. A _Contribution Credit_ (`32491`): the
receipt-backed record that a contributor produced a coverage attestation, an
independent verdict, a reproduction, or a confirmed finding — the standing
that the coordination analysis identified as sufficient incentive for a large
class of contributors. A _Payout Reference_ (`32492`): if and only if a
settlement rail is admitted, the reference binding a Contribution Credit to
settlement evidence.

**Why this shape and not a bounty market.** The prior analysis argued for
keeping money out until someone can articulate why it must be in, and it named
the risk precisely: mixing money into disclosure changes the legal and social
shape entirely, and a per-vulnerability bounty rewards racing and volume — the
two behaviors most likely to destroy the program's reputation in its first five
disclosures. The proposal here is deliberately different:

- **Pay for coverage and verification, not for vulnerabilities.** A signed
  coverage attestation over an unscanned target, an independent verdict that
  _refutes_ a candidate, a negative control, a reproduction — these are the
  scarce goods, they are cheap to verify, and paying for them creates no
  incentive to publish prematurely or to inflate severity.
- **Credit before cash.** Contribution Credits work with no rail at all and
  are what §8 Phase 1 ships.
- **Payouts stay gated.** As §2 records, this repository has no live payout
  rail: MDK/Nexus money authority is retired under VP-1, payout routes are
  stripped from the served registry, and LDK is a readiness projection. Any
  sats flow is therefore an **owner decision plus a rail decision**, and this
  NIP therefore defines a complete credit path that does not require a payout
  rail.

**Settlement evidence, when it exists.** NIP-57 zap receipts (`kind:9735`) and
NIP-61 nutzaps are the obvious Nostr-native evidence carriers, and both are
ordinary events Immortal already stores. A zap receipt is evidence that a
payment happened; it is not authority to pay, and NIP-OC keeps the accounting
boundary.

**Roadmap disposition.** NIP-BT remains a protocol draft, but Funding Pools,
Contribution Credits, and Payout References are outside the first pass. The
program will first prove the project page, browser relay connection,
joinability, coverage, disclosure, and invariant paths. BT work requires a new
owner sequencing decision after those paths have evidence.

## 6. The three surfaces

### 6.1 The relay is the backend

`relay.openagents.com` holds the program. Everything below is a projection of
it. Two facts shape the work:

**Closed writes.** `restricted_writes: true` means "point your agent at our
relay and join" needs an admission path that does not exist today. Options,
in increasing order of effort:

1. **Enable NIP-86 management** on the production deployment (set the
   management pubkey) so an operator can `allowpubkey` a contributor. Smallest
   change; makes joining a human-in-the-loop step.
2. **A public join surface**: an `/api/public/*`-style endpoint that accepts a
   NIP-98-authenticated join request and calls the management API under
   policy. Self-serve, still gated, auditable.
3. **NIP-43 relay access requests** implemented in Immortal so admission is an
   on-wire protocol rather than an out-of-band API. Correct long-term; a real
   relay feature with its own fixtures and conformance work.

Whichever is chosen, contributors' **agents** ride Block NIP-AA: once a human
is a member and has published a NIP-OA attestation for their agent key, the
agent authenticates and writes under the owner's standing, with owner-
aggregated rate accounting. That is the "point your agents at our relay"
mechanism, and it already works in the deployed relay.

**Kind admission.** The program's kinds must be admitted by relay policy. The
new hardening kinds and the All Work kinds need an explicit allow decision per
`AGENTS.md` rule 8 — a protocol change without a fixture is not complete.

### 6.2 The Rust relay client for Immortal

Nothing today lets a client talk to our relay with the OpenAgents kinds
decoded. Under the 2026-08-04 owner decision the first client is **Rust**, so
that Omega, the browser surface, and any contributor's agent share one
implementation rather than three.

Proposed crate: `immortal-client` (workspace member; consumed by Omega
natively and by the wasm surface through the same source).

| Layer          | Contents                                                                                                                                                                                                                                                               |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Transport      | Native and browser transports; connect, REQ/EVENT/CLOSE/AUTH, reconnect, NIP-42 auth, subscription lifecycle, EOSE and live handoff with explicit gap reporting. The wasm build uses the browser's WebSocket API and connects directly to `wss://relay.openagents.com` |
| Relay features | NIP-11 capability read, NIP-45 COUNT, NIP-50 search, NIP-29 groups, NIP-17 private messages, NIP-70 protected events, Blossom when configured                                                                                                                          |
| Kinds          | Typed encoders/decoders for the All Work kinds and the hardening kinds, with unknown-kind preservation                                                                                                                                                                 |
| Identity       | Local signer, remote signer (NIP-46), Block NIP-OA attestation helpers, NIP-AA auth flow                                                                                                                                                                               |
| Discipline     | Every decoder fails closed on malformed input; every projection carries freshness and completeness; the client never asserts authority a record does not carry                                                                                                         |

Two boundaries from day one: it is a **client**, not an authority — it decodes
and publishes, it does not admit anything — and it must build for both the
native Omega target and `wasm32-unknown-unknown`, which is a real constraint on
its dependency choices (see the `settings`-crate lesson in omega#243: one
transitive `errno`/`polling` dependency is enough to lose the browser target).

The first client proof is read-only and runs inside `/dh`. Browser developer
tools and an automated browser test must show the page opening the relay
WebSocket itself, sending bounded Nostr `REQ` filters, receiving `EOSE`, and
continuing to receive live events. Fetching a JSON copy from an OpenAgents API,
embedding a build-time project document, or terminating the relay subscription
on the application server does not satisfy this proof.

**Retired to backup: the TypeScript SDK.** The earlier plan was a generated
`@openagentsinc/immortal-sdk` emitted from a pinned contract definition,
reusing the `packages/all-work-contract` generator. That plan is **not
cancelled, it is deferred** — it remains the right answer for third-party
JavaScript integrators, for a DOM projection if one is later admitted, and for
anyone who wants relay access without a Rust toolchain. It should be revisited
once the Rust client's wire behavior is stable, so the TypeScript surface is
generated from proven semantics rather than designed in parallel. Nothing in
this program depends on it for the first deliverable.

### 6.3 Operation Diamond Hands at `/dh`

The first projection is a basic read-only Project page. It renders these
public signed records directly from the relay:

- the NIP-OT Organization (`kind:32100`) named by the Project's `org` tag,
  including its public name and authority refs;
- the NIP-PG Project (`kind:32222`): name, owner and lead refs, teams,
  initiative, dates, progress claim, document and workroom refs;
- its Project Status definition (`kind:32223`): label, lifecycle category,
  and position;
- its latest Project Update (`kind:32226`): authored health, body, evidence
  refs, author, and publication time; and
- a bounded recent-activity feed of public events that reference the exact
  Project address, with kind, author, event id, timestamp, and decoded summary
  when a supported decoder exists.

The route shell can contain the route, loading states, and decoder rules. The
project name, status, update body, activity, and timestamps must not be baked
into the bundle. Unsupported or malformed records remain visible as unknown or
invalid records and never become project truth.

**How it is built (owner decision, 2026-08-04).** The projection is Omega's
GPUI components compiled to WebAssembly and rendered on a canvas through
WebGPU — the same component set and the same Rust client the workbench uses.
This overrides [Addendum A](2026-08-04-gpui-on-web-addendum.md)'s earlier
Tier-1 recommendation of a DOM surface. The addendum's costs are not disputed
and are accepted knowingly: no accessibility adapter on web, no search
indexing, no text selection or Ctrl-F, no link previews, roughly 70% browser
reach, and a multi-megabyte bundle. §12 of the addendum records what that
means and which of them can be bought back later.

A DOM projection remains the **backup path**, not a cancelled one: if the page
must be indexable, quotable, or reachable by a screen reader before those gaps
close, a later read-only `/api/public/hardening/*` projection can feed one, and
the deferred TypeScript SDK (§6.2) is how a third party would consume it.

**Route decision.** The owner admitted `/dh` as the public project-page shape
on 2026-08-04 and named the project Operation Diamond Hands. This settles the
route question for the first slice. It does not authorize unsupported coverage,
security, participation, or payment claims; public copy still carries source,
freshness, and authority labels.

**Browser data path.** On mount, the wasm client opens
`wss://relay.openagents.com`, subscribes to the exact Project coordinate and
the bounded set of Project Status, Project Update, and project-referencing
activity kinds, folds the snapshot only after `EOSE`, and then keeps the
subscription live. The page displays `connecting`, `live`, `reconnecting`,
`stale`, and `unavailable` states with the last observed relay event time.
Reconnect must establish a new bounded snapshot/live boundary without silently
dropping or duplicating activity.

The initial `/dh` route must make no `/api/public/hardening/*` data request.
Those projections remain a later compatibility and accessibility fallback,
not the first page's source. This direct browser connection is part of the
feature, not an implementation detail.

The page must state what it is: a projection with freshness, not a claim.
Coverage counts inherit the completeness of their inputs, and a target with no
attestation renders as _never examined_ — which is the honest and uncomfortable
default for most of the ecosystem.

### 6.4 Omega as the working surface

Omega already has the forensic workbench (`crates/omega_forensics`), the
entropy campaign dashboard, and a NIP-29 chat. What this spec adds is that the
workbench's outputs become **signed events on the relay** rather than local
state: a scan produces a Materialized Source Set and a Coverage Attestation;
findings enter the commitment-then-disclose lifecycle; claims prevent
collisions across contributors; and the campaign room is the Workroom the
program's NIP-OT binding names.

That is the eventual division of labor: **Omega runs the work, the relay holds
the record, the web surface shows the record, and the Rust client lets anyone
else join.** The first slice proves the middle of that chain before it adds
write participation: signed Project records on the relay appear live inside
the browser through the same Rust client and GPUI component set.

### 6.5 Other possibilities worth naming

- **Attested absence as a public good.** The coverage map's most valuable
  number is how many high-value targets have _never_ been examined. That
  number is also the thing most likely to attract funding.
- **Profile diversity as a security property.** Once profiles are shareable
  objects, a campaign can deliberately require N runs under M distinct
  profiles, making monoculture blindness measurable.
- **Negative controls on the wire.** Publishing a run that found nothing, over
  a target known to contain a seeded defect, is how a harness earns trust. The
  NIP-SP pre-registration makes negative controls first-class rather than
  embarrassing.
- **Maintainer-side subscriptions.** A project maintainer can subscribe to
  invariants and watches for their own repository — the program becomes useful
  to the people whose code it examines, which is the difference between
  "bad cop" and an adversarial-but-welcome relationship.
- **Cross-ecosystem reuse.** Nothing here is Bitcoin-specific. The same
  records work for any ecosystem where a defect is checkable against a public
  oracle.

## 7. The first hard design question

A public coverage map tells attackers exactly where nobody is looking.
"No one has ever examined this wallet's entropy path" is actionable for both
sides, and the coordination analysis flagged this as the question to settle
_before_ the ledger exists rather than after.

The proposed starting position, offered as a design to be attacked:

- **Aggregate coverage is public**: counts by ecosystem, class, and period.
  This is the number that makes the funding case and leaks the least.
- **Specific gaps are group-visible**: readable inside the coordination
  membership, which is cheap to obtain but not anonymous — the relay's closed
  membership is exactly this control, and NIP-29 audience scoping enforces it.
- **Findings are embargoed by default** with commitments published at
  discovery so credit never depends on speed.
- **Targets can opt into more disclosure, never less.** A maintainer who wants
  their coverage public can say so; nobody else can say it for them.

This is not obviously right, and the spec should not pretend otherwise. It is
the one decision that should be settled by the owner and the first outside
participants together, in the open, before Phase 3 ships.

## 8. Initial roadmap

Phases are ordered by what unlocks the most downstream work, with the demo cut
line explicit. Nothing here is admitted work: each phase becomes issues under
ordinary authority.

**Stand-down status:** Phases 0–4 are cancelled. Phase 0's reusable technical
foundation is preserved, but its relay publication, live-event exit proof, and
production deployment did not occur. Phase 1 never began. Restarting any phase
requires a new owner decision.

### Phase 0 — Operation Diamond Hands project page (first visible result)

All Rust. No TypeScript or OpenAgents projection API is on the critical path.

1. Review, pin, and admit the minimum read contract with fixtures: NIP-OT
   Organization (`32100`), NIP-PG Project (`32222`), Project Status (`32223`),
   Project Update (`32226`), and the exact public activity refs admitted for
   the recent feed.
2. Build the smallest `immortal-client` Rust/wasm slice needed for browser
   WebSocket connection, NIP-11 discovery, `REQ`/`EOSE`/live subscription,
   bounded reconnect, and typed decoding of those records.
3. Land the GPUI/wasm prerequisites from
   [omega#243](https://github.com/OpenAgentsInc/omega/issues/243): the
   `settings` wasm gate, embedded assets, and the actual Aiur theme. The page
   must not silently fall back to a generic theme.
4. Publish the signed program Organization, Project Status definition,
   Operation Diamond Hands Project, and initial Project Update records on
   `relay.openagents.com` from the admitted program authority. Publish a small
   sequence of public-safe project activity so the recent feed has source data.
5. Mount the GPUI/wasm surface at `/dh`. Show project identity, status, latest
   update, relevant refs, recent activity, relay connection state, and data
   freshness. Keep loading, empty, malformed, reconnecting, stale, and relay-
   unavailable states explicit.
6. Prove in an automated browser and its network trace that `/dh` opens the
   Nostr WebSocket from inside the browser, renders only after the snapshot
   boundary, receives a newly published event without a page reload, and does
   not call an OpenAgents project-data API.

**Exit:** a browser opens `/dh`, connects directly to
`wss://relay.openagents.com`, renders Operation Diamond Hands from signed
Project records, and appends one newly published project event live.

### Phase 1 — Make the relay joinable

7. Decide and implement the contributor admission path (§6.1): NIP-86
   management on production, the gated public join endpoint, or NIP-43 in
   Immortal.
8. Admit the remaining program event kinds in relay policy, with fixtures per
   `AGENTS.md` rule 8.
9. Publish the remaining root records: Team, Initiative, and the NIP-29
   Workroom binding, signed by the program authority key.
10. Verify a second, non-owner identity can authenticate, publish, and be read.

**Exit:** an outside human joins, attests an agent with NIP-OA, and that agent
writes one event that a third party reads back.

### Phase 2 — Coverage ledger and the expanded GPUI surface

11. Review and pin the drafted **NIP-SC** and **NIP-SP** contracts with fixtures
    and an implementation decision — coverage and profiles are the two records
    the demo needs, and they are the two the evidence most directly supports.
12. Expand `immortal-client` with NIP-42 auth, program-kind decoders, and the
    write path shared by native Omega and the wasm surface.
13. Seed the ledger from work already done: publish Coverage Attestations and
    Materialized Source Sets for the Coldcard experiment's two arms, and for the
    Omega self-scan. The divergence between arm A and arm B becomes the first
    published Divergence Note — the program's founding data point is a result we
    already have.
14. Expand `/dh` with targets, coverage completeness, profiles, and divergence
    while preserving the direct browser-to-relay source path.
15. Omega publishes attestations from real workbench runs rather than local
    state.

**Exit:** `/dh` expands from project information into a coverage ledger; an
outside contributor using the Rust client adds a Coverage Attestation that
appears live.

### Phase 3 — Findings and disclosure

16. Review and pin **NIP-FD** with fixtures and an implementation decision;
    implement commitments, encrypted candidate findings, the independent-
    verdict path (producer ≠ verifier, enforced), and the disclosure state
    machine.
17. Settle §7 in the open with the first outside participants; encode the
    answer in the audience rules rather than in prose.
18. Build the maintainer contact path: encrypted, per-project, with
    acknowledgement tracking. Slow and correct beats fast and voluminous — the
    first five disclosures set the program's reputation permanently.

**Exit:** one real finding travels commitment → verdict → maintainer →
acknowledgement → publication without any private material reaching a public
relay.

### Phase 4 — Invariants and regression watch

19. Review and pin **NIP-SI** with fixtures and an implementation decision;
    encode the first invariant families from the hunt-class list, starting
    with entropy provenance and build-versus-source divergence, because those
    are where the free-oracle property concentrates.
20. Implement artifact provenance witnesses — the `nm`-on-objects class of
    check — and bind them to invariants.
21. Regression watches over target revisions, with freshness and explicit
    stopping rules.

**Exit:** a target regression is detected by a watch rather than by a human
noticing.

### Deferred — Funding and credit

NIP-BT stays drafted and unimplemented. Funding Pools, Contribution Credits,
and Payout References are not part of the first pass and have no scheduled
phase. Reconsider them only after Phases 0–4 have evidence and the owner makes
a new sequencing decision. No payment rail is assumed.

## 9. Acceptance and falsification

| Claim the program will want to make    | Required proof                                                                                                                                                     | Falsifier                                                                                              |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| The first project page is Nostr-native | Browser evidence shows `/dh` opening `wss://relay.openagents.com`, completing a bounded `REQ`/`EOSE` snapshot, and receiving a live event with no project-data API | The page renders from baked data, a private database, an HTTP projection, or a server-side relay proxy |
| The surface is Omega's own code        | The web surface and the workbench share the `ui` component set, the Aiur theme, and `immortal-client`                                                              | The web surface reimplements components or decoding in another language                                |
| Anyone can join                        | An outside human plus their agent write and are read, using only public docs                                                                                       | Joining needs an operator to run SQL                                                                   |
| Coverage is honest                     | Every result references a Materialized Source Set; incomplete scans are visibly incomplete                                                                         | A result renders confidently over a partially-read program                                             |
| Divergence is captured                 | Two runs over one target produce a Divergence Note automatically                                                                                                   | Disagreement is only visible to whoever ran both                                                       |
| Findings are not raced                 | Commitments precede reveals; duplicate-work priority follows the admitted commitment anchor, not publication time                                                  | Priority accrues to whoever posts first                                                                |
| Disclosure is responsible              | No embargoed content on a public relay; maintainer path attempted before publication                                                                               | An embargo expiry alone publishes something                                                            |
| Verification is independent            | Verifier key ≠ producer key, enforced at admission                                                                                                                 | A scanner confirms its own finding                                                                     |

NIP-BT keeps its own payment and settlement falsifiers, but they are not an
acceptance gate for this first-pass roadmap while BT is deferred.

## 10. What this program will not build

- Not another general-purpose source scanner. Loupe stays a candidate
  generator and an upstream we contribute to; the missing system is the
  coordination layer around it.
- Not a relay-as-authority. Relay acceptance never admits a command, confirms
  a finding, completes a disclosure, or settles a payment.
- Not a public vulnerability feed. Aggregate coverage is public; specific gaps
  and embargoed findings are not.
- Not a bounty market by default, for the reasons in §5.5.
- Not a claim of ecosystem-scale coverage. Today the honest statement is that
  we have one measured experiment, one upstream PR, zero confirmed
  vulnerabilities in anyone else's code, and a coordination design nobody has
  joined yet. The roadmap's job is to change the last clause first.
