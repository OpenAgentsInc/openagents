# Hosted Forge And Agent Services — Rust Extension Analysis

**Date:** 2026-07-22
**Lane:** Reference analysis and design (`docs/forge/`). This document flips no
promise state, changes no runtime authority, mints no issue, and dispatches no
work. Candidate work needs normal Sol admission or an owner-accepted work
packet.
**Class:** strategy and architecture analysis, not code.
**Question:** Should OpenAgents extend its hosted Rust services into a GitHub
replacement plus coding-agent services, and with what components?
**Label key:** `[EXISTS]` = already implemented in an owned or reference repo,
`[NEEDS BUILD]` = a bounded new build for OpenAgents, `[SPECULATION]` = a
forward claim that this analysis does not prove.

## Sources read

| Source | What it gave |
| --- | --- |
| [`2026-07-22-nostr-git-forge-github-replacement-audit.md`](2026-07-22-nostr-git-forge-github-replacement-audit.md) | The Stage 0-4 plan, the speed thesis, the replacement mapping |
| [`2026-07-22-nostr-git-server-effect-vs-rust-decision.md`](2026-07-22-nostr-git-server-effect-vs-rust-decision.md) | Option C, the bright-line extraction, the same-day Amendment that triggers the ngit-grasp reversal |
| [`2026-07-22-grasp-ecosystem-prior-art-addendum.md`](2026-07-22-grasp-ecosystem-prior-art-addendum.md) | The `ngit-grasp` source reads, purgatory, GRASP-02/06, Shakespeare, pyramid |
| `docs/ngit/2026-07-21-ngit-analysis.md` and `docs/teardowns/2026-07-21-buzz-teardown.md` §7 | The three-plane model, the 8-step OpenAgents Git profile (§7.9) |
| `crates/oa-node`, `crates/oa-codex-control`, `crates/oa-workroomd`, `crates/openagents-cloud-contract`, `packages/cloud-contract` | The existing hosted Rust estate and the Effect Schema mirror pattern |
| `docs/cloud/README.md`, `docs/cloud/INVARIANTS.md` | The SBX program, the Firecracker/microVM direction, compute-versus-labor and settlement boundaries |
| `docs/sol/MASTER_ROADMAP.md` revision 131, `docs/sol/CLAIM_PROTOCOL.md`, `docs/mvp/README.md` | Current priority, the live claim ledger, the accepted MVP baseline |
| Epic #9171 (hands-off Full Auto), HANDS-2 #9173, epic #9179 and `docs/fable/2026-07-22-openagents-as-meta-agent-analysis.md` | Host-executed verification, the meta-agent buy side |
| `docs/nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md`, `packages/nip90` | The signed projection bus posture, the labor kinds 5930-5939, `lbr-closeout` |
| `docs/fable/2026-07-17-effect-vs-rust-architecture-analysis.md` §7 and root `CLAUDE.md` | The bright line and the standing Rust boundary, as already extracted in the decision audit |
| `FASTFOLLOW.md` | The learning-intent boundary, source evidence is never target authority |
| Metering surfaces in `apps/openagents.com/workers/api/src/cloud/` | `cloud-metering.ts`, `cloud-primitive-receipts.ts`, the exact-usage ledger pattern |

---

## 1. The position

**Yes. OpenAgents should extend its hosted infrastructure into an agent-native
forge plus coding-agent services, in the staged and reversible form below.**
The position has three bounded parts.

1. **Our coding agents move to our own forge as their primary coordination
   surface.** The claim ledger moves first, then patches, reviews, and merge
   receipts. GitHub stays a read-only mirror through every stage, so the move
   stays reversible. This confirms the replacement audit's Stages 0-3 and
   extends them into hosted-service form.
2. **The differentiated hosted product is not git hosting.** It is the
   composition: repository state, a machine-tempo claims ledger, hosted
   verification with signed receipts, isolated execution, and content-addressed
   receipt storage, on one identity model. GitHub was built for human tempo.
   This surface is built for agent tempo.
3. **The Rust question resolves per component, not globally.** The bright line
   from the Effect-versus-Rust doctrine holds everywhere. Rust runs
   process-opaque infrastructure behind Effect Schema mirrors, the
   `packages/cloud-contract` pattern. Policy, admission, and authority stay in
   Effect. The one new adoption candidate, `ngit-grasp`, fits that pattern
   without a fork (§5).

Nothing here outranks the Full Auto P0 or the #8979 closure gate in
`docs/sol/MASTER_ROADMAP.md` revision 131. Every component below is candidate
work that needs an owner-accepted work packet. §9 flags the candidates.

---

## 2. Our agents on our forge — what concretely changes

The replacement audit §4.1 documents the constraint. GitHub throttles content
creation at about 500 writes per hour per account, with about 80 per minute,
under a primary limit of about 5000 requests per hour. Those numbers are GitHub
policy at the time of writing and need re-verification before an external
claim. The shape of the constraint is stable. `[EXISTS]` as the documented
constraint.

Now apply the OpenAgents operating shape. `docs/sol/CLAIM_PROTOCOL.md` makes
the live GitHub issue set the cross-session claim ledger. Every parallel agent
posts a CLAIM comment before mutation, CLAIM-STATUS at boundaries, and
CLAIM-RELEASE at completion. Fleet sessions in this repository routinely run
ten or more parallel agents, and each unit of work produces several ledger
writes plus issue reads, pushes, and comments. A sustained wave therefore
approaches the shared content-creation ceiling within the hour, and every
agent on the same account contends for one budget. When the ceiling trips, the
correct client behavior is backoff, which stalls the whole fan-out.

Moving the ledger to an owned relay changes four things concretely:

1. **The write ceiling becomes owned policy.** The relay applies OpenAgents
   abuse policy, not a vendor cap sized for humans. Fleet writes stop
   contending for one 500-per-hour budget (replacement audit §4.2).
2. **Polling becomes subscription.** An agent subscribes to one repository
   coordinate filter and receives open work, claims, and status live. The
   `gh issue list` poll loop, pagination, and search cost disappear
   (replacement audit §4.3).
3. **Claim semantics stay identical.** The kind 1621 and 1630-1633 events
   carry the same CLAIM fields, the 90-minute staleness rule, and the
   process-or-worktree audit, exactly as `CLAIM_PROTOCOL.md` writes them.
4. **Full Auto and FleetRun gain a native coordination bus.** Epic #9171 wants
   hands-off runs with host verification. The FleetRun authority at
   `/api/fleet-runs` and the Full Auto loop can read and write one signed
   ledger instead of mixing GitHub state with private projections. The
   cross-app delegation design already selects the same posture: the relay is
   transport and projection, canonical authority stays in Cloud SQL and Khala
   Sync. `[SPECULATION]` on the integration shape, `[EXISTS]` on the posture.

**Commitment:** the internal fleet's primary forge becomes the owned
relay-plus-git surface at the end of Stage H2 (§8), with GitHub as read-only
mirror. The public GitHub replacement is a separate, later, separately gated
product. This analysis does not promise it.

---

## 3. The hosted service menu

Each service is tied to something OpenAgents already runs. The verdict column
answers one question: is this the differentiated product, or commodity
substrate we host because the composition needs it?

| Service | What it is | Existing estate | Verdict |
| --- | --- | --- | --- |
| (a) Hosted GRASP git-plus-relay | Repo hosting with signed-state push auth, npub identity, no accounts | `nostr-effect` NIP-34 vocabulary `[EXISTS]`, `ngit-grasp` reference `[EXISTS]`, no owned deployment `[NEEDS BUILD]` | Differentiated as composition. Raw git hosting is commodity |
| (b) Claims and issues ledger relay | The Sol claim protocol as a service, signed multi-agent work claims with staleness and audit semantics | `CLAIM_PROTOCOL.md` semantics `[EXISTS]`, relay core `[EXISTS]`, ledger profile `[NEEDS BUILD]` | **Differentiated.** No vendor sells a machine-tempo claim ledger |
| (c) Verification-as-a-service | Host-executed done-condition runs in isolated microVMs that emit signed receipts | HANDS-2 #9173 defines the rule, `oa-workroomd`, Firecracker guest images, SBX runtime `[EXISTS]`, the service form `[NEEDS BUILD]` | **Most differentiated.** This is the trust layer (§6) |
| (d) Execution sandboxes | Placement, isolation, lifecycle for agent work | `oa-node`, `oa-codex-control`, the complete SBX-00..SBX-09 program `[EXISTS]` | Commodity in the market, required substrate here, already owned |
| (e) Artifact and receipt storage | Content-addressed refs for artifacts, closeouts, evidence | `cloud-primitive-receipts.ts` `[EXISTS]`, `packages/nip90` `lbr-closeout` content-addressed receipts `[EXISTS]`, GCS `[EXISTS]`, the public service form `[NEEDS BUILD]` | Commodity mechanics, differentiated receipt semantics |
| (f) Agent identity and attestation | npub identity, NIP-39 external-identity binding, NIP-46 signer custody, owner attestation | All present in `nostr-effect` as wire format or service `[EXISTS]`, role wiring `[NEEDS BUILD]` | Commodity protocol, differentiated composition |
| (g) Usage metering and billing rails | Exact-usage rows, receipts, credits | `cloud-metering.ts`, `token_usage_events`, credits ledger `[EXISTS]` | Enabler, not a standalone product |

The pitch in one sentence: **a forge for agents**, where the repository, the
claim, the verification receipt, and the payment rail share one signed
identity fabric. GitHub sells none of that composition, because its unit of
account is a human.

---

## 4. Extend, adopt, or build — the per-component matrix

The bright line, quoted in the decision audit §2.1, governs every row. Typed
coordination and authority go to Effect. OS-enforcement and latency-floor work
goes to Rust helpers that hold no authority and speak frozen schema contracts.
The standing root `CLAUDE.md` boundary permits Rust exactly in the Cloud crates
and the audio helper, with Effect Schema mirrors for TypeScript callers.

| Component | Decision | Language placement | Home | First extension commit |
| --- | --- | --- | --- | --- |
| Event vocabulary fix | **Extend** | Effect | `nostr-effect` `src/wrappers/kinds.ts` | Change `GitReply` 1622 to NIP-22 kind 1111 with parity tests (decision audit Step 0a) |
| Relay production backend | **Extend** | Effect | `nostr-effect` `src/relay/storage/` | Add a Node 24 `EventStore` backend on Cloud SQL Postgres plus a fleet-rate load test (Step 1a) |
| Claim ledger profile | **Build, small** | Effect | New `packages/sol-ledger` (name open) in this monorepo | Map CLAIM, CLAIM-STATUS, and CLAIM-RELEASE fields onto kinds 1621 and 1630-1633 with the exact `CLAIM_PROTOCOL.md` semantics and oracle tests |
| Git hosting server | **Adopt with front** (§5) | Rust infrastructure (external MIT), Effect policy front and mirror | `ngit-grasp` pinned at `cbf6f1d`, front in this monorepo | The §5 evaluation packet: run the pinned server isolated, test whitelist and ingress admission against OpenAgents gates, record receipts |
| Forge contract mirror | **Build, small** | Effect | New `packages/forge-contract` | Effect Schema for the admission config, NIP-11 `supported_grasps` read, and the ref-admission receipt, following `packages/cloud-contract` |
| Verification service | **Extend** | Rust executes, Effect owns verdict authority and receipts | `crates/oa-workroomd` plus `docs/cloud/contracts/`, mirror in `packages/cloud-contract` | Add a bounded `verification_run` workroom profile: clone a pinned ref, run the named done-condition command, emit a signed receipt artifact |
| Sandboxes and placement | **Extend, mostly exists** | Rust infrastructure, Effect admission | `crates/oa-node`, `crates/oa-codex-control` | Add forge-source workspace materialization: clone from a GRASP clone URL resolved through a kind 30617 coordinate |
| Artifact and receipt storage | **Extend** | Effect | Cloud Run monolith routes plus GCS, reuse `cloud-primitive-receipts.ts` and `lbr-closeout` | Add a content-addressed artifact write and read route keyed by digest refs |
| Identity and attestation | **Extend** | Effect | `nostr-effect` signer services | Wire explicit signing roles for fleet sessions behind NIP-46 custody, per Buzz teardown §7.9 step 3 |
| Metering and billing | **Extend** | Effect | `apps/openagents.com/workers/api/src/cloud/cloud-metering.ts` | Add forge and verification meter rows beside the existing exact-usage rows |
| `ngit` CLI and `git-remote-nostr` | **Use** (no adoption into the estate) | External Rust tool, run as the fleet runs `git` | Installed tool, pinned v2.6.3 | None. Stage H0 dogfood only, drop-safe per the decision audit §8 |

Vendor options were considered and rejected for the differentiated rows. No
vendor sells a signed-state git server as a managed service, a machine-tempo
claim ledger, or verification receipts bound to our contracts. For the
commodity rows, OpenAgents already operates the substrate, so a vendor would
add a dependency without removing owned code. `[SPECULATION]` on the vendor
market staying that way.

---

## 5. The honest `ngit-grasp` verdict: adopt with an Effect front

The Amendment to the decision audit triggered its own reversal condition. The
server is obtained, pinned at `cbf6f1d` v1.2.0, MIT, maintained, tested, and
it runs both public GRASP instances. The re-opened question is exact: **can
the OpenAgents admission policy ride `ngit-grasp` without forking its
authorization core?**

**The answer this analysis commits to: yes, expected, because our policy
composes above its authorization core, not inside it.** The argument has three
steps.

1. **What the core enforces is mechanical, not political.** The addendum §4.2
   reads `src/git/authorization.rs` (1,676 lines) directly. It parses pushed
   refs from pack-protocol bytes and admits them only when they match the
   latest signed kind 30618 state from the authorized publisher set. That
   check is a mechanism. It has no opinion about which repositories exist or
   who may sign.
2. **OpenAgents policy owns the two inputs to that mechanism.** First, which
   announcements and state events ever reach the server's embedded relay. The
   server ships whitelist and blacklist configuration, and an Effect admission
   front can gate ingress before events arrive. Second, who holds the signing
   keys that produce a valid 30618. Signer custody is Effect-side NIP-46
   territory under the sovereign signer rule (Buzz teardown §7.9 step 3).
   Signing the new state **is** the merge decision. Review gates, generation
   checks, and policy versions run in Effect before the signature exists, per
   §7.9 steps 5 to 7.
3. **Therefore no fork.** Any policy the server cannot express lives in the
   Effect front, exactly as the Amendment's bright line demands. The server
   stays process-opaque infrastructure with its config, NIP-11, and
   ref-admission behavior mirrored in `packages/forge-contract`. This is the
   same shape as `crates/oa-node` behind `packages/cloud-contract`.

**What honesty subtracts from the verdict:**

- **This is expected, not proven.** The §4 evaluation packet must run the
  pinned server in isolation and prove deny-by-default admission through
  config plus ingress gating. If the whitelist and ingress control cannot
  express it, the owned Effect front from decision-audit Step 1b stands, and
  Option C continues unchanged. The verdict is falsifiable and the test is
  named.
- **GRASP-01 reads are public.** The protocol mandates unauthenticated reads
  and CORS `*`. `ngit-grasp` therefore must not host private customer
  repositories. Private hosting needs the NIP-98 credential path from the
  Buzz teardown §7.3, which is a later owned build. `[NEEDS BUILD]`
- **The alpha dependency churn is real.** The server pins `rust-nostr` crates
  at `0.45.0-alpha.3`. Adoption requires a re-pin policy and a rebuild check
  before every deployment decision (addendum §8).
- **The internal write-hot path never moves to it.** The Stage H1 claim
  ledger runs on typed `nostr-effect` clients against the owned relay, per
  decision-audit Step 2. `ngit-grasp` serves the git object plane, not the
  agent coordination plane. This is the addendum §6.2 middle course, adopted
  here as the committed shape.
- **Purgatory must be respected, not bypassed.** The event-before-data
  admission pattern (addendum §4.2, 3,226 lines) is part of why adoption is
  attractive. The Effect front must not reintroduce the race purgatory
  solves.

---

## 6. The business and product frame

### 6.1 Revenue surfaces on rails that exist

The metering pattern already prices machine work exactly. `token_usage_events`
rows carry provider, model, exact usage truth, demand kind, and demand source.
`cloud-metering.ts` and `cloud-primitive-receipts.ts` extend the same pattern
to cloud primitives. The hosted forge adds three meterable surfaces: repo
hosting and storage, verification runs, and sandbox execution. Each already
has receipt semantics in the estate. Pricing, packaging, and public sale are
owner decisions behind promise gates, not this document. `[SPECULATION]` on
pricing, `[EXISTS]` on the rails.

### 6.2 The marketplace composition

This is where the forge stops being infrastructure and becomes the machine-work
economy surface:

1. A NIP-90 labor request (kind 5934 in `packages/nip90`) references a
   repository coordinate `30617:<pubkey>:<repo-id>` and a pinned commit on the
   OpenAgents forge, plus a named done condition. The payload stays
   reference-only, which the package already enforces at decode time.
   `[EXISTS]` for the kinds and guards, `[SPECULATION]` for the flow.
2. A hired agent delivers a kind 1617 patch or a GRASP-06 pull-request push.
3. The verification service runs the pinned done condition in an isolated
   microVM and signs a receipt naming the coordinate, the commit, the command,
   and the outcome.
4. `lbr-closeout` composes the lifecycle into one content-addressed public-safe
   receipt. No sats move on the relay, settlement authority stays out of the
   wire, per the NIP-90 package rules and `docs/cloud/INVARIANTS.md`.
5. The meta-agent (epic #9179) consumes this as its buy side. Its rule already
   exists in HANDS-2 #9173 form: self-reported completion is evidence only,
   the host verifies the done condition. Hired-agent work from strangers is
   acceptable exactly when a verification receipt from infrastructure the
   buyer trusts backs it. **Verification receipts are the trust layer that
   makes a labor market clear.**

### 6.3 What Shakespeare proves about demand

Shakespeare is an external, production AI builder whose entire source substrate
is signed-state Nostr git (addendum §4.4). Its client signs a fresh kind 30618,
publishes it, then pushes to every clone URL in parallel. Its agent contract
says "ALWAYS commit after you finish your turn." One external team already
chose this substrate for machine-produced code, with no platform account and
no human-tempo forge in the loop. That proves the substrate demand exists
beyond OpenAgents. It does not prove fleet-scale coordination demand, because
Shakespeare targets single-user greenfield apps. `[EXISTS]` for the substrate
proof, `[SPECULATION]` beyond it.

### 6.4 Dogfood-first sequencing

The adoption order is the trust order. Our fleet first, because we are our own
first customer and the claim ledger is our hottest write path. Pylon
contributors second, because Pylon already links accounts and capacity and can
link npubs. The public third, only behind promise gates and the product-promise
registry. This mirrors how every other OpenAgents surface has shipped, and it
keeps the public GitHub-replacement claim out of our mouths until receipts
exist.

---

## 7. Risks, and what we explicitly do not build

- **Operational burden of hosting git for others.** A forge operator carries
  disks, backups, uptime, and on-call. Mitigation: our-fleet-only through
  Stage H3, GCS object mirroring, and a second GRASP instance via GRASP-02
  sync before any contributor promise. Do not underestimate this cost, it is
  the strongest argument for the staged order.
- **Moderation and abuse.** GRASP servers choose their own admission criteria.
  Ours is the Effect admission front: deny-by-default announcements, the
  purgatory pattern, and an explicit takedown path as owned policy before any
  public hosting. `[NEEDS BUILD]`
- **Durability.** Git objects live on real persistent disks with GCS mirrors.
  The relay is not a bulk object store, and relay events are projections, not
  the canonical work record. The signed projection bus posture from the
  delegation design holds here.
- **License boundaries.** `ngit-grasp` and `ngit` are MIT, safe to adopt and
  patch. Shakespeare is AGPLv3: it stays read-only reference, and no code
  copies into owned repositories. gitworkshop remains reference-only where its
  license is unverified.
- **Dependency churn.** The `rust-nostr` alpha pins in `ngit-grasp` and the
  `ngit` CLI are a real maintenance tax. The drop-safe posture from the
  decision audit §8 stays: banked receipts survive tool churn.
- **Private repositories.** Not hosted on the GRASP surface until the NIP-98
  credential path exists. Stated plainly so no customer promise leaks early.
- **Roadmap collision.** None of this preempts Full Auto P0, the #8979 gate,
  or the admitted Fast Follow ledgers. FastFollowSpec context remains
  learning-intent, never implementation authority.
- **What we do not build:** a general-purpose GitHub-clone web UI. The client
  ring exists (gitworkshop, gitview, gitplaza, n34) and interoperates by
  protocol. The OpenAgents UI is the Desktop workbench and the meta-agent
  front door. We also do not build settlement on the relay, a TypeScript
  packfile implementation, or an owned `git-remote-nostr` before receipts
  demand it.

---

## 8. The hosted-infrastructure roadmap, extending Stages 0-4

This extends the replacement audit's Stage 0-4 and the decision audit's
ordered plan into hosted-service form. Order is commitment, dates are not.
Every stage keeps GitHub as a read-only mirror until Stage H5 revisits that.

| Stage | What ships | Who uses it | Build-versus-adopt | Placement |
| --- | --- | --- | --- | --- |
| **H0 — now** | Stage 0 as written: dogfood `ngit`, publish one announcement, round-trip one patch. Land the 1622-to-1111 fix | Our fleet | Use the MIT CLI, extend `nostr-effect` | Effect fix, external tool |
| **H1 — the ledger** | Node 24 Cloud SQL `EventStore` backend, fleet-rate load test, relay deployed on Google Cloud, Sol claim ledger cutover on typed clients | Our fleet | Build, small, per decision-audit Steps 1a and 2 | Effect throughout. **Highest-leverage stage** |
| **H2 — owned git hosting** | The §5 evaluation packet decides Step 1b. Expected: `ngit-grasp` behind the Effect admission front and `packages/forge-contract`, hosting our public repos. Stage 3 patches, reviews, and signed merge receipts follow | Our fleet | Adopt with front, else owned Effect front if the evaluation falsifies §5 | Rust infrastructure, Effect policy and mirror |
| **H3 — verification-as-a-service** | HANDS-2 generalized: hosted `verification_run` in `oa-workroomd` microVMs, signed receipts published as events beside the repo coordinate | Full Auto host verification first, then fleet-wide | Extend the Cloud crates plus a new contract | Rust executes, Effect owns verdict and receipt authority |
| **H4 — contributor onboarding** | Pylon-linked npub identity, GRASP-06 pull-request hosting for contributors, artifact storage routes, metering rows on | Pylon contributors | Extend Pylon, `nostr-effect`, metering | Effect, with the adopted server unchanged |
| **H5 — public forge and market** | Public repos for outside teams, NIP-90 jobs referencing forge coordinates, verification receipts as the labor trust layer, priced surfaces | Public, behind promise gates | Separately gated owner decision, new ProductSpec | Mixed, per the same bright line |

Stages H0 through H3 are the ambitious-but-honest core. They take the internal
fleet off GitHub's critical path, stand up the two differentiated services,
and produce every receipt Stage H4 and H5 need. H4 and H5 are real product
decisions with their own gates, and this analysis deliberately does not
promise them.

---

## 9. Work-packet candidates

Repository policy restricts GitHub issues to reproducible bugs, so these are
flagged here for owner acceptance under the Sol claim protocol, not minted as
issues:

1. **WP-1:** the `GitReply` 1622-to-1111 fix in `nostr-effect`, with parity
   tests. Blocks every interop claim, option-neutral, small.
2. **WP-2:** the Node 24 Cloud SQL `EventStore` backend plus the fleet-rate
   load test. The single gate that protects the ledger cutover.
3. **WP-3:** the claim-ledger event profile: exact field mapping, staleness
   and audit semantics preserved, GitHub issue mirror during migration, oracle
   tests.
4. **WP-4:** the `ngit-grasp` evaluation packet from §5: isolated run at the
   pinned commit, deny-by-default admission proof, load shape, re-pin policy.
   This packet decides Step 1b with receipts.
5. **WP-5:** the `verification_run` workroom profile in `oa-workroomd`, the
   `openagents.verification_run.v1` contract document, and its Effect Schema
   mirror in `packages/cloud-contract`.
6. **WP-6:** the `packages/forge-contract` mirror package, needed only if
   WP-4 selects adoption.
7. **WP-7:** forge and verification meter rows in `cloud-metering.ts`, needed
   before any priced or contributor-facing stage.

WP-1 through WP-3 are independent of the adoption verdict and can start under
normal admission. WP-4 gates WP-6. WP-5 is independent and directly serves
epic #9171.

---

## 10. Watch items

- **The WP-4 evaluation receipt** is the open decision of record for Step 1b.
  Do not cite §5's expected verdict as settled before that receipt exists.
- **The WP-2 load test** protects the claim-ledger cutover. Do not move the
  ledger onto an unmeasured relay.
- **`rust-nostr` alpha churn** in both `ngit-grasp` and `ngit` — re-pin and
  re-read before every deployment decision.
- **The `forge/` control-plane seam** — the private `forge/` repo owns
  software-factory lifecycle authority. Signed merge receipts and verification
  receipts should feed its evidence, not bypass it. The exact seam remains a
  design item with that repo's owner.
- **HANDS-2 #9173 landing shape** — the H3 service should generalize whatever
  contract HANDS-2 lands, not fork a parallel verification path.
- **The GitHub rate-limit numbers** — re-verify before any external or public
  claim, they are vendor policy that changes.
- **Full Auto priority** — if any packet here collides with the #8979 closure
  work or its hot contracts, Full Auto wins and the packet waits.
