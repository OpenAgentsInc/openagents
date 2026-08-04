# The Bitcoin OSS hardening program as a Nostr-native public project

- Date: 2026-08-04
- Class: architecture spec and initial roadmap
- Status: proposal. Authorizes nothing. Names owner-gated decisions explicitly.
- Owner: OpenAgents
- Companion reading:
  [`../loupe/2026-08-01-coordination-not-scanners.md`](../loupe/2026-08-01-coordination-not-scanners.md)
  (the argument), [`../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md`](../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md)
  (what to hunt), [`../nips/PROPOSED.md`](../nips/PROPOSED.md) (the All Work
  NIP program), transcripts [263](../transcripts/263.md),
  [264](../transcripts/264.md), [265](../transcripts/265.md).

## 1. What this document is for

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

This document specifies how to build that coordination layer as a **public
project living in our own Nostr relay**, organized with the OpenAgents NIP
program, projected read-only onto the web, and worked through Omega.

The Episode 266 target, stated as a demo rather than a claim:

> A public page on openagents.com shows the hardening program — its targets,
> what has been scanned, with which profile, at what completeness, and what
> remains untouched — rendered entirely from signed events on
> `relay.openagents.com` through a TypeScript SDK anyone can install. An
> outside contributor can point their own agent at the same relay, claim a
> target, run a profile, and publish a coverage attestation that appears on
> that page.

## 2. What already exists

Facts, with paths, so the roadmap starts from the real substrate.

| Piece | State today | Where |
| --- | --- | --- |
| The relay | **Live.** `relay.openagents.com` serves Immortal 0.0.1 as of 2026-08-04, NIP-11 reporting NIPs 1, 9, 11, 17, 29, 40, 42, 45, 50, 65, 70, 94 plus eleven Block extensions | `~/work/immortal`, `PROVENANCE.md` |
| Relay write policy | `restricted_writes: true` — closed membership via `relay_member_pubkey`; agents can ride their owner's membership through Block NIP-AA | `immortal/src/store/mod.rs:457`, `src/gateway/server.rs:718` |
| Relay admin path | NIP-86 management API **not configured in production** (86 absent from live `supported_nips`); no media endpoint configured either | live NIP-11 |
| The NIP program | 25 All Work NIPs drafted across five layers | [`../nips/PROPOSED.md`](../nips/PROPOSED.md) |
| Signed workroom projection | **Implemented**, kinds 32150–32163 pinned, prepare/commit signing lane, persist-before-publish outbox | `packages/all-work-contract/src/signed-workroom-*.ts`, [`../nips/WA.md`](../nips/WA.md) |
| Contract → SDK generator | **Implemented**: one pinned JSON definition emits Effect Schema, a TypeScript client, Rust types, JSON Schema, fixtures, and a digest-bound compatibility manifest with drift checking | `packages/all-work-contract/scripts/generate.mjs` |
| Nostr client code | `packages/public-nostr-chat` (relay client, subscribe/snapshot, remote signer); `nostr-effect` sibling repo is the shared Effect implementation | `packages/public-nostr-chat/src/client.ts`, `~/work/nostr-effect` |
| Forensic workbench | Implemented in the **Omega** repo (Rust/GPUI) with an Effect Schema boundary and Loupe adapter in this monorepo | `~/work/omega/crates/omega_forensics/`, `packages/forensic-contract/`, `packages/forensic-loupe-adapter/` |
| Coldcard evidence | Pre-registered experiment, results, generator reproduction, evidence graph, historical fingerprint scan | `docs/loupe/`, `docs/coldcard/`, `fixtures/forensics/coldcard/` |
| Web app | Cloud Run Node monolith; retained public product routes are `/`, `/forum`, `/promises`; `apps/start` serves retained documents; `/api/public/*` routes are exact entries in one registry | `apps/openagents.com/workers/api/src/cloudrun/server.ts`, `src/index.ts` |
| Sats payout | **No live rail.** MDK/Nexus money authority retired under VP-1; payout/L402/credit routes stripped from the served registry; LDK exists as typed readiness projections only | root `INVARIANTS.md`, `workers/api/src/index.ts:13341`, `pylon-ldk-readiness-projections.ts` |

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
   scoring rubric was written, digested, and pushed *before* the run. On a
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
   for *coverage and verification work* rather than running a bounty market.

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
outcomes. It does not cover five things that this specific program lives or
dies on. Proposed kind block: **32450–32499**, extending the OpenAgents
addressable neighborhood declared in `PROPOSED.md`. Every number is tentative
pending a collision review, and each NIP is a draft-to-be-written, not a draft.

| NIP | Name | Tentative kinds | Why it cannot be folded into an existing NIP |
| --- | --- | --- | --- |
| NIP-SP | Scan Profiles and Pre-Registration | 32450–32459 | A profile is a versioned, shareable, digest-bound *configuration*, and the rubric must be committed before the run |
| NIP-SC | Source Completeness and Coverage | 32460–32469 | The Coldcard lesson: what was actually on disk is a claim that must be signed, checkable, and comparable across runs |
| NIP-FD | Findings, Verdicts, and Disclosure | 32470–32479 | Findings need severity, CWE, embargo state, hash commitments, and coordinated-disclosure lifecycle that generic evidence records do not carry |
| NIP-SI | Security Invariants and Regression Watch | 32480–32489 | The decisive Coldcard control was a build-time assertion about the shipped artifact — a durable property, not a one-time finding |
| NIP-BT | Bounties and Contribution Credit | 32490–32499 | Funding pools, credit standing, and (later, if ever) payouts need their own records with hard boundaries against the disclosure process |

### 5.1 NIP-SP — Scan Profiles and Pre-Registration

**Records.** A *Scan Profile* (`32450`): named, versioned, digest-bound
configuration — source materialization rules (submodules, vendored trees,
lockfile-pinned deps), file selection, attack-surface ranking policy, hunt
classes, model and harness roles, budget bounds, and evidence requirements. A
*Pre-Registration* (`32451`): published **before** a run, binding the target,
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
the difference is that a profile is a *measurement instrument* whose exact
bytes must be citable by a result. It references guidance and skills rather
than replacing them.

### 5.2 NIP-SC — Source Completeness and Coverage

**Records.** A *Materialized Source Set* (`32460`): for one scan, the exact
tree that was actually readable — repository refs, pinned commit, submodule
paths declared versus populated, vendored trees, dependency lockfile digests,
file count and byte count analyzed versus skipped and why. A *Coverage
Attestation* (`32461`): the durable public statement that this target, at this
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
finding sets is a lead, published as a *Divergence Note* (`32462`) that names
both runs and what differed.

### 5.3 NIP-FD — Findings, Verdicts, and Disclosure

**Records.** A *Finding Commitment* (`32470`): a digest-bound commitment
published at discovery, revealing nothing. A *Candidate Finding* (`32471`):
target, mechanism, severity proposal, CWE, evidence boundary, assumptions —
encrypted to the disclosure audience while embargoed. A *Finding Verdict*
(`32472`): an independent judgment (`confirmed`, `refuted`, `inconclusive`)
from a signer other than the producer. A *Disclosure State* (`32473`):
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

**Records.** A *Security Invariant* (`32480`): a named property that must hold
across source, configuration, build, artifact, and runtime, with its failure
consequence, required witnesses, and falsifiers. An *Artifact Provenance
Witness* (`32481`): evidence binding a built artifact to the exact source,
symbols, or configuration an invariant requires. A *Regression Watch*
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

### 5.5 NIP-BT — Bounties and Contribution Credit

**Records.** A *Funding Pool* (`32490`): a sponsor's committed budget for a
named campaign, scope, and period. A *Contribution Credit* (`32491`): the
receipt-backed record that a contributor produced a coverage attestation, an
independent verdict, a reproduction, or a confirmed finding — the standing
that the coordination analysis identified as sufficient incentive for a large
class of contributors. A *Payout Reference* (`32492`): if and only if a
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
  *refutes* a candidate, a negative control, a reproduction — these are the
  scarce goods, they are cheap to verify, and paying for them creates no
  incentive to publish prematurely or to inflate severity.
- **Credit before cash.** Contribution Credits work with no rail at all and
  are what §8 Phase 1 ships.
- **Payouts stay gated.** As §2 records, this repository has no live payout
  rail: MDK/Nexus money authority is retired under VP-1, payout routes are
  stripped from the served registry, and LDK is a readiness projection. Any
  sats flow is therefore an **owner decision plus a rail decision**, and this
  NIP must be written so the program is complete and useful without it.

**Settlement evidence, when it exists.** NIP-57 zap receipts (`kind:9735`) and
NIP-61 nutzaps are the obvious Nostr-native evidence carriers, and both are
ordinary events Immortal already stores. A zap receipt is evidence that a
payment happened; it is not authority to pay, and NIP-OC keeps the accounting
boundary.

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

### 6.2 The TypeScript SDK for Immortal

Nothing today lets an outside developer talk to our relay in typed TypeScript
with the OpenAgents kinds decoded. The SDK is the thing that makes the
program joinable, and it should be **generated, not hand-written**, reusing
the pattern already proven in `packages/all-work-contract`: one pinned
definition emits Effect Schema, a TypeScript client, Rust types, JSON Schema,
fixtures, and a digest-bound compatibility manifest, with clean-regeneration
drift checking in CI.

Proposed package: `@openagentsinc/immortal-sdk`.

| Layer | Contents |
| --- | --- |
| Transport | Connect, REQ/EVENT/CLOSE/AUTH, reconnect, NIP-42 auth, subscription lifecycle, EOSE and live handoff with explicit gap reporting |
| Relay features | NIP-11 capability read, NIP-45 COUNT, NIP-50 search, NIP-29 groups, NIP-17 private messages, NIP-70 protected events, Blossom when configured |
| Kinds | Typed encoders/decoders for the All Work kinds and the hardening kinds, with unknown-kind preservation |
| Identity | Local signer, remote signer (NIP-46), Block NIP-OA attestation helpers, NIP-AA auth flow |
| Discipline | Every decoder fails closed on malformed input; every projection carries freshness and completeness; the client never asserts authority a record does not carry |

Two boundaries to write into the package from day one: it is a **client**, not
an authority — it decodes and publishes, it does not admit anything — and it
must extend `nostr-effect` rather than rebuilding Nostr primitives, per
workspace policy.

### 6.3 The public projection on openagents.com

A read-only page rendering the program from the relay: targets and their
coverage state, recent scans with their completeness, the divergence feed,
open invariants, the disclosure funnel in aggregate (counts and states, never
embargoed content), and the contributor roster with credits.

**Honest constraint.** The retained public product routes are `/`, `/forum`,
and `/promises`; adding a new public product page is a **product-shape
exception requiring owner authority**. Three viable shapes, in the order I
would propose them:

1. **`/hardening` as an owner-admitted new public route** — cleanest for the
   demo and for outside contributors, and it needs an explicit owner decision
   plus a product-promise entry before it can make any public claim.
2. **Inside `/forum`** as a program space, since Forum is retained and already
   the intake-first surface — no new route, weaker as a dashboard.
3. **API-first**: ship `/api/public/hardening/*` read-only projections now
   (the `relay-health-routes.ts` pattern: read-only, Effect, no-store, declared
   staleness, "grants no authority"), and let Omega and third-party clients
   render them while the page decision is pending.

Option 3 is the one that requires no exception and can land immediately;
option 1 is what Episode 266 wants to show. They compose: build the API and the
projection logic first, mount the page when the owner admits it.

The page must state what it is: a projection with freshness, not a claim.
Coverage counts inherit the completeness of their inputs, and a target with no
attestation renders as *never examined* — which is the honest and uncomfortable
default for most of the ecosystem.

### 6.4 Omega as the working surface

Omega already has the forensic workbench (`crates/omega_forensics`), the
entropy campaign dashboard, and a NIP-29 chat. What this spec adds is that the
workbench's outputs become **signed events on the relay** rather than local
state: a scan produces a Materialized Source Set and a Coverage Attestation;
findings enter the commitment-then-disclose lifecycle; claims prevent
collisions across contributors; and the campaign room is the Workroom the
program's NIP-OT binding names.

That is the division of labor for the demo: **Omega runs the work, the relay
holds the record, the web page shows the record, the SDK lets anyone else
join.**

### 6.5 Other possibilities worth naming

- **Attested absence as a public good.** The coverage map's most valuable
  number is how many high-value targets have *never* been examined. That
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
*before* the ledger exists rather than after.

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
participants together, in the open, before Phase 2 ships.

## 8. Initial roadmap

Phases are ordered by what unlocks the most downstream work, with the demo cut
line explicit. Nothing here is admitted work: each phase becomes issues under
ordinary authority.

### Phase 0 — Make the relay joinable (blocking everything)

1. Decide and implement the admission path (§6.1): NIP-86 management on
   production, or the gated public join endpoint, or NIP-43 in Immortal.
2. Admit the program's event kinds in relay policy, with fixtures per
   `AGENTS.md` rule 8.
3. Publish the program's root records: Organization, Team, Initiative, and the
   NIP-29 Workroom binding, signed by the program authority key.
4. Verify a second, non-owner identity can authenticate, publish, and be read.

**Exit:** an outside human joins, attests an agent with NIP-OA, and that agent
writes one event that a third party reads back.

### Phase 1 — Coverage ledger and the SDK (the Episode 266 demo)

5. Draft **NIP-SC** and **NIP-SP** as spec files in `docs/nips/` — coverage and
   profiles are the two records the demo needs, and they are the two the
   evidence most directly supports.
6. Generate `@openagentsinc/immortal-sdk` from a pinned contract definition:
   transport, NIP-42 auth, typed decoders for the program kinds, drift check.
7. Seed the ledger from work already done: publish Coverage Attestations and
   Materialized Source Sets for the Coldcard experiment's two arms, and for the
   Omega self-scan. The divergence between arm A and arm B becomes the first
   published Divergence Note — the program's founding data point is a result we
   already have.
8. Ship `/api/public/hardening/*` read-only projections and the projection UI;
   mount `/hardening` if and when the owner admits the route.
9. Omega publishes attestations from real workbench runs rather than local
   state.

**Exit (the demo):** the page renders live from the relay; an outside
contributor using only the published SDK adds a coverage attestation that
appears on it.

### Phase 2 — Findings and disclosure

10. Draft **NIP-FD**; implement commitments, encrypted candidate findings, the
    independent-verdict path (producer ≠ verifier, enforced), and the
    disclosure state machine.
11. Settle §7 in the open with the first outside participants; encode the
    answer in the audience rules rather than in prose.
12. Build the maintainer contact path: encrypted, per-project, with
    acknowledgement tracking. Slow and correct beats fast and voluminous — the
    first five disclosures set the program's reputation permanently.

**Exit:** one real finding travels commitment → verdict → maintainer →
acknowledgement → publication without any private material reaching a public
relay.

### Phase 3 — Invariants and regression watch

13. Draft **NIP-SI**; encode the first invariant families from the hunt-class
    list, starting with entropy provenance and build-versus-source divergence,
    because those are where the free-oracle property concentrates.
14. Implement artifact provenance witnesses — the `nm`-on-objects class of
    check — and bind them to invariants.
15. Regression watches over target revisions, with freshness and honest
    stopping rules.

**Exit:** a target regression is detected by a watch rather than by a human
noticing.

### Phase 4 — Funding and credit (owner-gated)

16. Draft **NIP-BT**; ship Contribution Credits with no rail, because credit
    and standing work today and are what the analysis says most contributors
    actually want.
17. Funding Pools for sponsored campaigns, scoped and public.
18. Payout references **only** after an admitted settlement-rail decision. This
    repository has no live payout authority today, and the spec deliberately
    does not assume one.

**Exit:** a sponsor funds a campaign, contributors accrue credits against
published coverage, and the accounting is auditable end to end — with or
without a payment rail attached.

## 9. Acceptance and falsification

| Claim the program will want to make | Required proof | Falsifier |
| --- | --- | --- |
| The program is Nostr-native | Every record readable from `relay.openagents.com` with the published SDK and no OpenAgents API | The page needs a private database to render |
| Anyone can join | An outside human plus their agent write and are read, using only public docs | Joining needs an operator to run SQL |
| Coverage is honest | Every result references a Materialized Source Set; incomplete scans are visibly incomplete | A result renders confidently over a partially-read program |
| Divergence is captured | Two runs over one target produce a Divergence Note automatically | Disagreement is only visible to whoever ran both |
| Findings are not raced | Commitments precede reveals; credit is commitment-ordered, not publication-ordered | Credit accrues to whoever posts first |
| Disclosure is responsible | No embargoed content on a public relay; maintainer path attempted before publication | An embargo expiry alone publishes something |
| Verification is independent | Verifier key ≠ producer key, enforced at admission | A scanner confirms its own finding |
| Money never distorts disclosure | Credits pay for coverage and verification; payouts, if any, are gated behind disclosure completion | A payout is claimable by publishing a finding |

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
