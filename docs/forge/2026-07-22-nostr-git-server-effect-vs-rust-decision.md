# Nostr Git Server — Effect versus Rust Decision Audit

**Date:** 2026-07-22
**Lane:** Architecture decision audit (`docs/forge/`). This document flips no
promise state, changes no runtime authority, mints no issue, and dispatches no
work. Candidate work needs normal Sol admission or an owner-accepted work
packet.
**Class:** decision audit, not code.
**Question:** For the Nostr git-forge plan in
[`2026-07-22-nostr-git-forge-github-replacement-audit.md`](2026-07-22-nostr-git-forge-github-replacement-audit.md),
should OpenAgents (a) leverage the existing Rust reference implementations for
the git server, relay, and tooling layer, or (b) reimplement that layer in
Effect TypeScript inside `nostr-effect` in one large sprint?
**Label key:** `[EXISTS]` = already implemented in an owned or reference repo,
`[NEEDS BUILD]` = a bounded new build for OpenAgents, `[SPECULATION]` = a
forward claim that this audit does not prove.

## Amendment (2026-07-22, same day): the adopted-server reversal condition is TRIGGERED

The GRASP acquisition pass that followed this audit obtained every target,
including `ngit-grasp`. The full evidence is in
[`2026-07-22-grasp-ecosystem-prior-art-addendum.md`](2026-07-22-grasp-ecosystem-prior-art-addendum.md)
and the pinned clones are in the workspace lane `projects/grasp/repos/`.
Three claims below are corrected by that evidence:

1. **"`ngit-grasp` is not obtainable" is now false.** A GRASP server exposes
   plain git smart-HTTP at `/<npub>/<identifier>.git` beside its relay, so
   kind-30617 discovery plus an ordinary `git clone` from `relay.ngit.dev`
   obtained it. Pinned at `cbf6f1d`, version v1.2.0, MIT license.
2. **"The Rust option's server half is hollow" is now false.** `ngit-grasp`
   is a maintained, MIT, production pure-Rust GRASP server. It runs both
   public instances, embeds a rust-nostr relay, implements GRASP-01/02/06,
   parses pushed refs from the pack protocol in-process, and validates them
   against signed 30618 state before git applies refs, with real tests.
   `[EXISTS]`
3. **The §8 "owned front → adopted server" reversal condition is met.** All
   four written conditions hold: clonable, licensed, auditable, maintained.
   Per this audit's own rule, Step 1b (the owned Effect front plus
   `git http-backend`) re-opens as a bounded evaluation packet: owned Effect
   front versus adopting `ngit-grasp` as infrastructure behind an Effect
   Schema mirror, the `packages/cloud-contract` pattern. The evaluation must
   answer one question honestly: can the OpenAgents admission policy ride
   `ngit-grasp` without forking its authorization core. If not, the owned
   Effect front stands.

**What does not change.** The recommendation remains Option C. Steps 0a
(`GitReply` 1622→1111 fix), 0b (`ngit` dogfood), 1a (compliant Node/Cloud SQL
`EventStore` backend plus fleet-rate load test), and 2 (claim ledger through
typed `nostr-effect` clients) are unaffected. The bright line also stands:
if `ngit-grasp` is adopted, it runs as process-opaque infrastructure with the
ref-admission policy contract mirrored in Effect Schema, and any policy the
server cannot express stays in an Effect front, never in a fork we silently
maintain. The original text below is retained unchanged as the decision
record. Read it with this amendment first.

## Sources read

| Source | What it gave |
| --- | --- |
| `docs/forge/2026-07-22-nostr-git-forge-github-replacement-audit.md` | The Stage 0-4 rollout under re-evaluation |
| `docs/fable/2026-07-17-effect-vs-rust-architecture-analysis.md` | The bright-line doctrine, the workload table, the reversal tests |
| `docs/voice/2026-07-12-effect-vs-rust-audio-architecture-decision.md` | The one accepted product-side Rust exception and its shape |
| Root `CLAUDE.md` Rust boundary clauses | The standing repository policy on where Rust is permitted |
| `docs/ngit/2026-07-21-ngit-analysis.md` | The pinned-commit inventory of ngit-cli, ngit-relay, gitworkshop, Shakespeare, nostrify |
| `docs/teardowns/2026-07-21-buzz-teardown.md` §7.9 and §8 | The 8-step OpenAgents git profile, the substrate rejections |
| `docs/nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md` | The relay-substrate posture for the sibling delegation lane |
| `~/work/nostr-effect` at commit `c160378` | The relay server source, storage backends, and test files, read directly |
| `~/work/projects/repos/ngit-cli` at `6d806d5` (v2.6.3) | The Rust CLI and remote helper, read directly |
| `~/work/projects/repos/ngit-relay` at `632be04` | The archived GRASP reference server, read directly |

---

## 1. Verdict in one paragraph

**Neither pure option survives contact with the facts.** The "leverage Rust
infrastructure" option assumes a mature adoptable Rust git-plus-relay server.
That server does not exist in our reference set. The GRASP reference server is
archived Go, and its successor is not obtainable through our sync lane (§3).
The "one big Effect sprint" option assumes the git server needs a TypeScript
packfile implementation. It does not. Stock `git http-backend` does the
packfile work, and the owned build is a thin admission front plus a hook, which
is policy code (§5). The honest decision is a refined hybrid: dogfood the Rust
`ngit` CLI as an external tool exactly as the fleet runs `git`, build the small
owned server surface in Effect where the doctrine already puts policy code, and
put the event layer on `nostr-effect`, which already owns it. This confirms the
forge audit's Stage 0-4 order and amends its Stage 1 and Stage 4 content (§7).

---

## 2. The doctrine, extracted from our own documents

The decision criteria below are not invented for this audit. They are quoted or
condensed from three standing sources.

### 2.1 The bright line (`docs/fable/2026-07-17-effect-vs-rust-architecture-analysis.md` §7)

> If the workload's correctness is defined by typed coordination — who may do
> what, in what order, surviving which failures — it belongs in the Effect
> kernel. If its correctness is defined by what the operating system enforces
> or by sub-frame/sub-buffer latency, it belongs in a Rust helper process that
> holds no authority, speaks a frozen schema'd contract, and whose absence
> fails closed.

Its corollaries: helpers are opaque and authority-free, the kernel is the only
policy owner, and nothing native links into the kernel process. Its workload
table assigns sync/relay/E2EE work (class W9) to **Effect** for protocol and
state, with Rust "only if a crypto or throughput seam demands it," and it makes
that reversal "a load-test verdict, not a vibe." Its sequencing rule for the
Rust references is explicit: "Port the designs into small owned helpers, do not
vendor the megacores."

### 2.2 The standing repository boundary (root `CLAUDE.md`)

Rust is permitted in this monorepo in exactly two places. First, the Cloud
crates (`crates/oa-node`, `crates/oa-codex-control`, `crates/oa-workroomd`,
`crates/openagents-cloud-contract`), justified as "systems infrastructure
(Firecracker/vsock microVMs, GCE capacity, managed-node lifecycle), not UI or
Worker logic." Second, the narrow persistent-audio helper
`crates/oa-desktop-audio`. The clause that governs everything else: "Product,
UI, Worker, and Pylon logic stays on Effect/TypeScript." TypeScript callers
never link the crates directly. They use the Effect Schema mirrors in
`packages/cloud-contract` and documented HTTP contracts.

### 2.3 The one product-side exception and its shape (`docs/voice/2026-07-12-effect-vs-rust-audio-architecture-decision.md`)

The audio decision admitted Rust because the workload was realtime device I/O:
microphone capture, resampling, bounded buffers, barge-in flush. The decision
divided the system "by authority and realtime constraints." Effect kept the
contract, the policy, the supervision, and the receipts. The Rust helper got no
command, credential, Sync, or retention authority, and the measured AUDIO-2
result kept even the cloud gateway in Effect because the packaged Node client
passed its threshold. The exception proves the rule: Rust enters only where a
measured realtime or OS constraint demands it, and the boundary is audited so
it cannot widen.

### 2.4 The criteria this audit applies

From the three sources above, five named criteria:

1. **Doctrine fit.** Is the workload typed coordination and policy (Effect) or
   OS-enforcement and latency-floor work (Rust helper or systems crate)?
2. **Time-to-first-receipt.** How fast does each option produce the Stage 0
   and Stage 1 receipts the forge audit defines?
3. **Maintenance surface.** How much owned code, in how many languages, must
   OpenAgents keep healthy afterward?
4. **Agent ergonomics.** The fleet writes most future code. Which option keeps
   the write-heavy paths (the Stage 2 claim ledger above all) in the stack
   agents author and test fastest?
5. **Reversibility.** Can the choice be swapped later behind a stable seam, and
   is the reversal test measurable?

---

## 3. The Rust inventory, verified

What actually exists on disk, at the pinned commits in
`docs/ngit/2026-07-21-ngit-analysis.md`, re-verified for this audit.

- **`ngit-cli` (Rust, MIT, v2.6.3, commit `6d806d5`, active).** `[EXISTS]`
  Two binaries: the `ngit` porcelain and the `git-remote-nostr` transport
  helper. About 69k lines with tests, on `rust-nostr` and libgit2. NIP-46
  bunker signing and NIP-49 encrypted local keys, so no plaintext key on disk.
  This is real, installable, and licensed for use. It is the strongest Rust
  asset in the set.
- **`ngit-relay` (Go, not Rust, commit `632be04`, ARCHIVED).** `[EXISTS]` as
  reference only. The GRASP reference server is khatru (Go) plus nginx plus
  stock `git-http-backend` plus hook binaries under supervisord. Upstream
  archived it after khatru breakage. It is a design document in code form, not
  an adoptable service.
- **`ngit-grasp` (the successor).** *(Superseded same day — see the
  Amendment above. Obtained at `cbf6f1d`, MIT, maintained.)* Original text:
  Not obtainable. Its development is hosted
  over ngit itself, and our sync lane cannot pull `nostr://` remotes yet. We
  cannot audit, pin, or deploy what we cannot clone. `[SPECULATION]` on its
  quality.
- **`gitworkshop` (no license file).** Reference-only forever. Never a code
  source.
- **Mature standalone Rust relays** (for example `nostr-rs-relay`) exist
  upstream but are not in our reference set, are not audited by any owned
  teardown, and none of them serves the git half of GRASP. `[SPECULATION]`
  until cloned and audited.
- **Our own Rust estate.** `crates/oa-node`, `crates/oa-codex-control`,
  `crates/oa-workroomd`, `crates/oa-desktop-audio`, plus the sibling `tap-ldk`
  and `ldk-node` repos. `[EXISTS]` OpenAgents demonstrably ships and operates
  Rust daemons on GCE and Cloud Run. Operational capability is not the
  constraint.

**The inventory verdict:** the Rust option's client half is strong (`ngit` is
real and MIT). The Rust option's server half is hollow. There is no maintained,
licensed, adoptable Rust git-plus-relay server in our reference set. Choosing
"Rust infrastructure" for Stage 1 would mean porting an archived Go design into
a new owned Rust service, which is a build, not an adoption, and it is a build
in the language our doctrine reserves for OS-adversary and latency-floor work
that this service does not contain.

---

## 4. The `nostr-effect` relay, honestly

The forge audit says `nostr-effect` ships "a full relay server." Verified
directly at commit `c160378`, here is what that means and does not mean.

**What exists.** `[EXISTS]`

- A platform-agnostic relay core: `MessageHandler` (591 lines),
  `SubscriptionManager`, `RelayInfo`, a policy pipeline, and a `NipRegistry`
  with more than twenty NIP modules (01, 09, 11, 15, 16, 20, 28, 29, 40, 42,
  45, 50, 57, 62, 67, 70, 77, 86, and the OpenAgents draft modules).
- A storage seam: the `EventStore` interface in `src/relay/storage/` is
  platform-agnostic, with two implementations — `BunSqliteStore` (Bun runtime,
  SQLite) and `DoSqliteStore` (Cloudflare Durable Objects).
- Fifteen relay test files at the top level plus per-module tests, covering
  deletion, ephemeral events, command results, expiration, vanish, EOSE,
  protected events, Negentropy sync, management, and rate limiting.
- The complete NIP-34 vocabulary in `src/core/Nip34.ts` (908 lines) and NIP-GS
  signing in `src/services/GitObjectSigningService.ts` (950 lines).

**What does not exist.** `[NEEDS BUILD]`

- **No production deployment.** No owned relay serves traffic anywhere. The
  relay has never carried a fleet-scale write load. Its throughput at target
  fleet size is unmeasured.
- **No compliant production backend.** One backend targets Cloudflare Durable
  Objects, which root `CLAUDE.md` retires as a deploy target — it must not be
  used. The other backend targets the Bun runtime, and the monorepo's
  supported-host contract is Node 24 (a zero-supported-bun guard runs in
  `check:fast`). A Google Cloud deployment therefore needs either a Node
  backend behind the existing `EventStore` seam with Cloud SQL or durable-disk
  SQLite storage, or an explicit owner decision to run Bun on Cloud Run in the
  sibling repo. Either path is bounded because the core is platform-agnostic,
  but neither exists today.
- **The 1622-versus-1111 defect.** `src/wrappers/kinds.ts` line 667 exports
  `GitReply = 1622` while NIP-34 specifies NIP-22 kind 1111 comments. The
  forge audit §2.3 flags this. It is a `nostr-effect` fix and it is needed
  under every option, which makes it option-neutral and first in the plan.

**The reality-check verdict:** the relay is real, typed, tested library code
with a clean storage seam, and it is not yet a production service. The distance
from here to a deployed relay is a bounded backend-plus-deploy slice, not a
rewrite. That distance is roughly equal to the distance of standing up any
unaudited external relay, because the external relay would still need our
policy pipeline, our admission rules, and our deploy path.

---

## 5. The load-bearing correction: a git server is mostly not code we write

Both framed options overstate the server build. The GRASP shape, as implemented
by the archived reference server and described in the Buzz teardown, is:

1. An HTTP front that routes `/<npub>/<identifier>.git` requests.
2. Stock **`git http-backend`** — the CGI program that ships with git itself —
   doing all `git-upload-pack` and `git-receive-pack` packfile work.
3. A **pre-receive hook** that admits a push to `refs/heads/*` only when the
   pushed commit equals the ref value in the latest maintainer-signed 30618
   event.
4. The relay beside it.

Item 2 is C code maintained by the git project. Nobody reimplements it, not
ngit-relay (which fronts it with nginx), not Buzz. So the "git smart-HTTP
server plus packfile handling in TS" cost that makes Option B look enormous is
a phantom: no packfile code needs writing in any language. And the "mature Rust
git server" that makes Option A look free is equally phantom (§3). What
OpenAgents actually owns and builds is items 1, 3, and 4: request routing,
push-admission **policy**, and the relay. Under the §2.1 bright line, items 1
and 3 are typed coordination and authority decisions — who may mutate which
ref, under which signed intent — which is exactly the workload class the
doctrine assigns to Effect. There is no OS-enforcement or latency-floor
workload anywhere in this service. Spawning `git http-backend` per request is
ordinary subprocess I/O, the same class of work Pylon does with `git` and
`codex` today. `[EXISTS]` for the mechanism, `[NEEDS BUILD]` for the owned
front and hook.

---

## 6. The three options, scored

### Option A — Rust-leveraged infrastructure

Adopt `ngit` CLI for client operations. Run a Rust relay and a GRASP-shaped
git server as infrastructure under the systems-infrastructure exception, with
Effect Schema mirrors and typed clients over HTTP/WS, the
`packages/cloud-contract` pattern.

- **Doctrine fit: weak for the server, strong for the CLI.** The CLI is an
  external tool like `git` itself, and the doctrine says nothing against
  shelling to external tools. But the owned server's core logic is push
  admission — policy and authority — which the bright line assigns to Effect.
  A Rust service here would hold ref-mutation authority, violating "helpers
  are opaque and authority-free." The Cloud-crates precedent does not carry:
  those crates manage microVMs, vsock, and GCE capacity, real OS-counterparty
  work. A git front is not that.
- **Time-to-first-receipt: fast for Stage 0, slow for Stage 1.** There is no
  adoptable server (§3), so Stage 1 under Option A means writing a new Rust
  service from an archived Go design, then writing its Effect Schema mirror,
  then deploying both.
- **Maintenance surface: worst.** A new owned Rust service, its contract
  mirror, and a two-language seam for a service whose logic changes with
  policy, which is the kind of change agents make weekly.
- **Agent ergonomics: split.** Fine for the CLI. Poor for the server, since
  policy iteration crosses a language boundary.
- **Reversibility: moderate.** The HTTP seam keeps it swappable.

### Option B — all-Effect sprint

Extend `nostr-effect` with the git server, a `git-remote-nostr` helper, NIP-98
credentials, and push authorization in one large sprint.

- **Doctrine fit: strong**, with one caveat: reimplementing packfile transport
  in TypeScript would be waste, and reimplementing the remote helper before
  any receipt exists violates "port the designs into small owned helpers"
  sequencing — build after need, not before.
- **Time-to-first-receipt: worst.** The sprint frontloads the remote helper
  and credential work that Stage 0-2 does not need. The forge audit already
  established that internal flows use ordinary HTTPS clone URLs, so
  `nostr://` transport is a Stage 4 concern. A "one big sprint" spends weeks
  before the first receipt that Stage 0 produces in a day with `ngit`.
- **Maintenance surface: good long-term**, one language, one library — but it
  buys that coherence earlier than any receipt justifies.
- **Agent ergonomics: best** once built. Typed Effect clients beat shelling
  out, for the write-heavy ledger path above all.
- **Reversibility: good**, the event layer keeps everything swappable.

### Option C — hybrid staged (refined by §5)

Stage 0 dogfoods `ngit` (external MIT tool, run exactly as the fleet runs
`git`). Stage 1 deploys the `nostr-effect` relay on Google Cloud with a
compliant backend, plus a thin Effect admission front spawning stock
`git http-backend`, plus the signed-state pre-receive hook. Stage 2 moves the
Sol claim ledger onto typed `nostr-effect` clients. The `git-remote-nostr`
helper and NIP-98 path stay deferred to Stage 4 and get built, or upstreamed
into ngit, only after receipts prove the flow.

- **Doctrine fit: strongest.** Policy and coordination in Effect. No owned
  Rust. Stock git binaries do transport. The external CLI carries the early
  receipts without entering the maintenance surface.
- **Time-to-first-receipt: fastest.** Stage 0 is a day of tool installation
  and one patch round-trip. Stage 1 is one bounded backend-and-deploy slice
  plus a hook.
- **Maintenance surface: smallest.** The owned delta is a relay backend, a
  thin HTTP front, a hook, and the 1111 fix, all in the library that already
  owns the vocabulary.
- **Agent ergonomics: best where it matters.** The Stage 2 claim ledger, the
  write-heavy path, is a typed Effect subscription and publisher, not a CLI
  shell-out. The CLI shell-out is confined to Stage 0 proof work.
- **Reversibility: best.** Every seam has a named swap (§8).

### Score summary

| Criterion | A Rust-leveraged | B all-Effect sprint | C hybrid staged |
| --- | --- | --- | --- |
| Doctrine fit | weak (server holds authority in Rust) | strong (with waste) | strongest |
| Time-to-first-receipt | Stage 0 fast, Stage 1 slow | worst | fastest |
| Maintenance surface | worst (new Rust service + mirror) | good, bought too early | smallest |
| Agent ergonomics | split | best, later | best where write-heavy |
| Reversibility | moderate | good | best |

---

## 7. The recommendation and the ordered plan

### The recommendation

**Adopt Option C.** Dogfood the Rust `ngit` CLI as an external tool for the
first receipts, build the small owned server surface — relay backend, admission
front, pre-receive hook — in Effect inside and around `nostr-effect`, and defer
all `nostr://` transport work to Stage 4. Do not write a new Rust service, and
do not run the unlicensed or archived reference servers.

The two strongest reasons:

1. **The Rust alternative does not exist as claimed.** There is no maintained,
   licensed Rust git-plus-relay server to adopt (§3). Option A is secretly a
   greenfield Rust build of a policy service, which our own bright line (§2.1)
   assigns to Effect, since push admission is authority logic and nothing in
   the service is OS-enforcement or latency-floor work.
2. **The costly part of Option B is a phantom, so its benefit comes cheap.**
   Stock `git http-backend` removes the packfile burden (§5), which shrinks
   the "all-Effect git server" to a thin front plus a hook. OpenAgents gets
   one-language coherence, typed clients for the write-heavy Stage 2 ledger,
   and reuse across Desktop, the Cloud Run monolith, and Pylon, without the
   big-sprint price the option assumed.

### The ordered plan, amending the forge audit's stages

Each step is a bounded packet under normal Sol admission. The forge audit's
stage ORDER stands. The amendments are to Stage 1 content, Stage 2 mechanism,
and Stage 4 scope.

1. **Step 0a — fix `GitReply` 1622 to NIP-22 1111 in `nostr-effect`.**
   Option-neutral, blocks every interop claim, one bounded change in
   `src/wrappers/kinds.ts` and any Nip34 reply builders, with parity tests.
   `[NEEDS BUILD]`, small.
2. **Step 0b — Stage 0 as written: dogfood `ngit`.** Install the MIT tool,
   announce one repository, submit and read back one patch over Nostr, GitHub
   stays canonical. Confirmed unchanged from the forge audit. `[EXISTS]`
   tooling.
3. **Step 1a — give the `nostr-effect` relay a compliant production backend.**
   *Amends Stage 1.* Implement the `EventStore` interface for a Node 24 host
   with Cloud SQL Postgres or durable-disk SQLite, reusing the
   platform-agnostic core. Do not deploy the Cloudflare backend, that target
   is retired. Add a load test at fleet write rates before Stage 2 cutover.
   `[NEEDS BUILD]`, bounded by the existing storage seam.
4. **Step 1b — deploy relay plus git front on Google Cloud.** *Amends Stage
   1.* One service: the relay, a thin Effect HTTP front routing
   `/<npub>/<identifier>.git` to spawned stock `git http-backend`, and the
   signed-state pre-receive hook (a small Node script that queries the relay
   for the latest maintainer-signed 30618). Provision bare repositories from
   admitted 30617 announcements only, keeping OpenAgents admission gates.
   Mirror the repository, GitHub stays canonical. `[NEEDS BUILD]`.
5. **Step 2 — move the Sol claim ledger onto typed Effect clients.** *Amends
   Stage 2 mechanism.* The ledger profile uses `nostr-effect` client
   subscriptions and publishers directly, not `ngit` shell-outs, because the
   write-heavy agent path deserves typed clients and the CLI adds no value
   for pure event traffic. Semantics stay exactly as
   `docs/sol/CLAIM_PROTOCOL.md` writes them. This is the highest-leverage
   step, unchanged in rank from the forge audit. `[NEEDS BUILD]`.
6. **Step 3 — patches, reviews, merge receipts, as the forge audit's Stage 3.**
   All event-layer work on `nostr-effect`, merge admission through the Step 1b
   hook. Unchanged. `[NEEDS BUILD]`.
7. **Step 4 — the public forge, rescoped.** *Amends Stage 4.* Before building
   an owned `git-remote-nostr`, evaluate contributing needed changes to ngit
   upstream, and adopt `ngit-grasp` for study once a `nostr://` pull path
   exists. An owned Effect remote helper is justified only if upstream cannot
   serve the public-forge requirements, and that evaluation is a receipt-gated
   decision, not a default. `[SPECULATION]` on the outcome, deliberately.

---

## 8. Reversal conditions

Each choice above is falsifiable. The evidence that flips it:

- **Relay Effect → Rust or C++ relay.** If the Step 1a load test, or production
  operation, shows the Node relay cannot sustain the fleet's measured write and
  subscription rates within the latency budget the claim ledger needs, adopt a
  mature external relay (audit and pin it first) behind the same event
  contracts, and keep the policy pipeline as an admission proxy in front. The
  event layer makes the relay a swappable component. This is the doctrine's own
  W9 reversal: "a load-test verdict, not a vibe."
- **Effect git front → native front.** If measured spawn overhead of
  `git http-backend` per request becomes the bottleneck at real traffic, front
  it with nginx exactly as ngit-relay did, keeping the hook and policy in
  Effect. This changes a deployment detail, not the architecture.
- **Owned front → adopted server.** If `ngit-grasp` becomes clonable,
  licensed, auditable, and maintained, and its admission model can enforce
  OpenAgents policy without forking, re-evaluate running it as infrastructure
  with an Effect Schema mirror, the `packages/cloud-contract` pattern. Two of
  those four conditions were unmet at first writing. *(Same-day Amendment:
  all four conditions now hold, this reversal is TRIGGERED, see the
  Amendment section at the top.)*
- **Deferred helper → early build.** If Stage 2-3 receipts show agents
  materially blocked by the absence of `nostr://` remotes, or an interop
  partner requires them, pull the Step 7 evaluation forward.
- **CLI dogfood → drop.** If `ngit` upstream stalls or breaks against pinned
  `rust-nostr` alphas (the ngit analysis §7 names this dependency risk), Stage
  0's receipts are already banked and nothing owned depends on the tool.
- **The whole split → more Rust.** Inherited from the doctrine document
  verbatim: the split reverses toward Rust only if agent competence in
  large-scale Rust observably surpasses TypeScript on this team's own
  review-and-oracle metrics.

---

## 9. Watch items

- **The Step 1a load test** is the single gate that protects the claim-ledger
  cutover. Do not move Stage 2 onto an unmeasured relay.
- **`ngit-grasp` availability** — re-run the §8 adopted-server evaluation when
  it becomes clonable.
- **The 1622-versus-1111 fix** (Step 0a) blocks every interop claim and is
  needed under every option. It should land first.
- **The sibling delegation lane**
  (`docs/nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md`)
  rides the same relay. Its posture — "authority stays in Cloud SQL and Khala
  Sync, the relay is transport" — is the same posture this audit keeps for git:
  refs and admission are authority, the relay carries signed projections and
  proposals. One deployed relay should serve both lanes.
