# Nostr Git Forge — GitHub Replacement Audit

**Date:** 2026-07-22
**Lane:** Reference analysis and design (`docs/forge/`). This document flips no
promise state, changes no runtime authority, mints no issue, and dispatches no
work. Candidate work needs normal Sol admission or an owner-accepted work
packet.
**Class:** research and design, not code.
**Label key:** `[EXISTS]` = already implemented in an owned or reference repo,
`[NEEDS BUILD]` = a bounded new build for OpenAgents, `[GAP]` = an open design
problem with no clear owner, `[SPECULATION]` = a forward claim that this audit
does not prove.

## Sources read

| Source | What it gave |
| --- | --- |
| `docs/ngit/2026-07-21-ngit-analysis.md` | ngit and GRASP source analysis, the three-plane model, the state-event-as-credential thesis, candidate uses |
| `docs/ngit/2026-07-21-soapbox-what-is-ngit-source.md` | External adoption prose (untrusted), the Block Buzz launch framing |
| `docs/teardowns/2026-07-21-buzz-teardown.md` §7 | The Buzz hybrid forge, the push fence, protected refs, the 8-step OpenAgents Git profile (§7.9) |
| `~/work/projects/repos/nips/34.md` | Exact NIP-34 kinds and tag shapes |
| `~/work/nostr-effect/src/core/Nip34.ts` (908 lines) | The event vocabulary already implemented in owned Effect TypeScript |
| `~/work/nostr-effect/src/services/GitObjectSigningService.ts` | NIP-GS commit signing, already implemented |
| `docs/sol/CLAIM_PROTOCOL.md` | The current agent claim ledger, which today rides GitHub issues |
| `.github/ISSUE_TEMPLATE/strict-bug.yml` | The GitHub issue-form ceremony gate |

**Companion documents:** `docs/ngit/2026-07-21-ngit-analysis.md` (the ngit
source lane) and `docs/teardowns/2026-07-21-buzz-teardown.md` §7 (the Buzz forge
deep dive). A sibling Nostr agent-delegation document
(`docs/nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md`) is
named as a related lane. That document is not on `main` at the time of this
audit. If it lands, it shares the same relay and signer substrate that this
forge design uses. `[SPECULATION]`

---

## 1. Verdict

**Can OpenAgents replace GitHub immediately for the internal agent workflow?**
**Yes, in a bounded form, and no, for the public contributor forge.** The two
answers are different products and the audit keeps them apart.

- **Internal agent flow off the GitHub critical path — achievable now.** The
  event vocabulary is already implemented in `nostr-effect` `[EXISTS]`. The
  claim ledger, the issue set, and the patch and review records can move to
  signed Nostr events on an owned relay. The git objects can live on one owned
  git server that OpenAgents already knows how to run on Google Cloud. GitHub
  becomes a read-only mirror, not the source of truth. The first slice is small
  and reversible.
- **Public contributor forge — a larger build.** Open discovery, host failover,
  a browser forge UI, and a stable public identity model are not a first-week
  task. This audit scopes them as later stages, not the immediate move.

The owner claim that "we can actually be faster than all these GitHub-centric
flows" is correct for the machine-scale case, and this audit makes the reason
concrete in §4. The short version: GitHub rate limits and issue-form ceremony
were designed around human tempo. A fleet of agents writes at a tempo that
those limits were never built to serve. A relay that OpenAgents owns has no such
ceiling.

---

## 2. What we already have versus what we must build

### 2.1 The event layer is done

`nostr-effect/src/core/Nip34.ts` implements a generator and a parser for every
NIP-34 kind this audit needs:

- Repository announcement, kind 30617, with `d`, `name`, `description`,
  `clone`, `web`, `relays`, `maintainers`, and the `euc` anchor. `[EXISTS]`
- Repository state, kind 30618, one tag per ref plus the `HEAD` symref.
  `[EXISTS]`
- Patch, kind 1617, with the `git format-patch` body and the commit-identity
  tags. `[EXISTS]`
- Pull request, kind 1618, and pull-request update, kind 1619, with `clone` and
  tip-commit tags. `[EXISTS]`
- Issue, kind 1621. `[EXISTS]`
- Status, kinds 1630 open, 1631 applied or merged, 1632 closed, 1633 draft.
  `[EXISTS]`
- GRASP server list, kind 10317. `[EXISTS]`
- Repository address helpers `30617:<pubkey>:<repo-id>` and the `euc`
  coordinate. `[EXISTS]`

`nostr-effect` also ships `GitObjectSigningService` for NIP-GS commit signing
`[EXISTS]`, a full relay server with a policy pipeline `[EXISTS]`, and NIP-46
bunker signing `[EXISTS]`. This is the strongest single fact in the audit. The
protocol vocabulary is not a research task. It is installed and typed in an
owned repository.

### 2.2 What is missing

Three transport and hosting pieces are not in owned code:

- **A `git-remote-nostr` transport helper.** ngit ships one in Rust
  (`ngit-cli`). It lets `git clone nostr://...` work by advertising refs from
  the signed 30618 state event and moving objects to and from the listed git
  servers. OpenAgents has no owned helper. For the immediate internal move this
  is not required, because the internal flow can use ordinary HTTPS clone URLs
  named in the 30617 announcement. `[NEEDS BUILD]` for the public path,
  `[EXISTS]` as an installable MIT tool (`ngit`) we can dogfood first.
- **A git server plus relay on one owned host (the GRASP shape).** The relay
  exists in `nostr-effect`. A colocated git smart-HTTP endpoint with a
  push-authorization hook does not. `[NEEDS BUILD]`
- **A NIP-98 git credential path for private reads and authorized pushes.** The
  Buzz teardown §7.3 shows the shape. OpenAgents has no owned adapter yet.
  `[NEEDS BUILD]`

### 2.3 One event-layer note to resolve before interop

`nostr-effect` exports `GitReply` as kind 1622. NIP-34 (source read at
`~/work/projects/repos/nips/34.md`) states replies to issues, patches, and pull
requests follow NIP-22 comment, which is kind 1111, not 1622. Any interop claim
against ngit, gitworkshop, or Buzz must resolve this before it ships. Prefer the
NIP-22 kind 1111 comment for cross-client compatibility, and treat 1622 as an
internal alias only if a migration needs it. `[GAP]`

---

## 3. The replacement mapping, GitHub feature to Nostr event

Each row maps a GitHub feature the OpenAgents workflow actually uses to its
Nostr equivalent, with the exact kind and the build state.

| GitHub feature | Nostr equivalent | Exact kind or mechanism | State |
| --- | --- | --- | --- |
| Repository record and metadata | Repository announcement | kind 30617, signed by a maintainer key | `[EXISTS]` |
| Fork grouping and project identity | Earliest-unique-commit anchor | `r <commit> euc` tag on 30617 | `[EXISTS]` |
| Branch and tag refs, the source of truth | Repository state | kind 30618, one tag per ref, `HEAD` symref | `[EXISTS]` |
| Push and fetch of code objects | Git smart HTTP against a listed server | ordinary `git-upload-pack` and `git-receive-pack`, URL named in the 30617 `clone` tag | server `[NEEDS BUILD]`, git protocol itself unchanged |
| `git clone nostr://...` sugar | Nostr clone URL plus transport helper | `nostr://<npub>/<relay-hint>/<identifier>`, resolved by `git-remote-nostr` | helper `[NEEDS BUILD]`, MIT tool `[EXISTS]` to dogfood |
| Pull request, small change | Patch | kind 1617, `git format-patch` body, under 60 kb | `[EXISTS]` |
| Pull request, larger change | Pull request | kind 1618 plus 1619 updates, objects pushed to a git server, tip in the `c` tag | `[EXISTS]` at event layer |
| Issue | Issue | kind 1621, Markdown content, `subject` and `t` tags | `[EXISTS]` |
| Issue and PR state, open, closed, merged, draft | Status | kinds 1630, 1631, 1632, 1633 | `[EXISTS]` |
| Review comments and threaded discussion | NIP-22 comment | kind 1111, referencing the issue, patch, or PR event | event `[EXISTS]` in NIP-22, the 1622-versus-1111 note in §2.3 must be resolved `[GAP]` |
| Review request, approval, change request | Labeled note projection | kind 1 or a labeled note, client trust rules decide validity (Buzz pattern) | `[NEEDS BUILD]` as an OpenAgents profile |
| Protected branch and merge policy | Push-authorization hook plus signed merge receipt | pre-receive state-match rule, then a signed merge-outcome event | `[NEEDS BUILD]` |
| Checks and CI status | CI events on Nostr | an OpenAgents verification-receipt kind, aligned with the gitworkshop CI-event surface | `[NEEDS BUILD]`, `[GAP]` on kind choice |
| Releases | Signed release event plus tag ref in 30618 | a release note event referencing the tag, OpenAgents already signs releases | `[NEEDS BUILD]` for the Nostr projection |
| Preferred host list | GRASP server list | kind 10317, `g` tags in preference order | `[EXISTS]` |
| Commit and tag signing | NIP-GS | Schnorr signature over `nostr:git:v1:`, in the git object | `[EXISTS]` |
| Account and permission model | Signed maintainer keys plus a relay policy | 30617 `maintainers` tag, push hook, NIP-46 signer custody | policy `[NEEDS BUILD]`, signer `[EXISTS]` |
| The claim and issue ledger for agents | Issues and status events on an owned relay | kinds 1621 and 1630-1633, read and written directly from the relay | `[NEEDS BUILD]` as an OpenAgents ledger profile |

The single sharpest reusable idea from the ngit lane: **the signed state event
is the push credential.** The archived GRASP reference server admits a branch
push only when the pushed commit already equals the ref value in the latest
maintainer-signed 30618 event. No accounts, no tokens, no SSH keys. The
signature on intent authorizes the mutation. That is the same
signed-intent-to-admitted-mutation-to-receipt shape as the rest of the
OpenAgents verification thesis, applied to git hosting. Reuse the pattern.

---

## 4. The speed thesis, made concrete

The owner is emphatic that OpenAgents can be faster than GitHub-centric flows.
This section proves the mechanism, and it is honest about where Nostr is not
faster.

### 4.1 GitHub rate limits bite the multi-agent fan-out

GitHub imposes several ceilings on authenticated API traffic. The exact numbers
below are GitHub public policy at the time of writing, not OpenAgents
measurements, and they should be re-verified before any external claim.
`[SPECULATION]` on the precise current numbers, `[EXISTS]` as the general shape
of the constraint.

- **Primary REST limit:** about 5000 requests per hour per authenticated user or
  token. GraphQL uses a separate points budget of about 5000 points per hour.
- **Secondary content-creation limit:** GitHub throttles rapid creation of
  issues, pull requests, comments, and similar content. Public guidance is on
  the order of about 80 content-creating requests per minute and about 500 per
  hour. This is the ceiling that a coding fleet hits first.
- **Concurrency limit:** GitHub asks clients to make no more than about 100
  concurrent requests, and its abuse-detection heuristics can throttle bursts of
  writes even under the hourly total.

Now apply the OpenAgents shape. The Sol claim protocol and this very session run
many parallel agents. Each agent, in a normal loop, does several GitHub writes,
a claim comment, status comments, an issue read, a pull-request create, review
comments, and a merge. A wave of, for example, 8 to 20 concurrent Full Auto or
FleetRun workers, each touching the issue and PR API several times per unit of
work, reaches the 500-per-hour content-creation ceiling quickly. When it does,
GitHub returns a secondary-rate-limit error and the correct client behavior is
to back off, which stalls the whole fan-out. The limit is per account, so more
agents on the same token do not buy more throughput. They share one 5000-per-
hour and one 500-writes-per-hour budget and contend for it.

The Block Buzz engineering note, quoted in the source capture, states the same
observation from the other side: a team of agents can produce "human-months of
commits and CI runs in an afternoon," and "centralized forges were designed
around human rate limits." That is an independent party reaching the identical
conclusion. `[EXISTS]` as external evidence.

### 4.2 A relay OpenAgents owns has no such ceiling

On an owned relay, the write budget is a policy OpenAgents sets, not a vendor
limit. The relevant numbers change character:

- **No per-account content ceiling.** An agent publishes an issue event, a
  status event, a patch event, or a claim event as fast as the relay accepts
  writes. The relay applies OpenAgents policy, which can rate-limit abuse, but
  it does not impose a GitHub-shaped 500-per-hour cap on normal fleet work.
- **Near relay, low round-trip.** A relay on the same Google Cloud region as the
  fleet answers a publish or a subscription in single-digit to low-tens of
  milliseconds. A GitHub API round trip crosses the public internet to
  Microsoft infrastructure and back, and it carries REST or GraphQL overhead
  plus rate-limit accounting. For an agent that reads and writes the ledger
  hundreds of times across a run, the aggregate latency difference is large.
  `[SPECULATION]` on the exact per-call figure, `[EXISTS]` as the direction.
- **Direct read of repo events, no polling.** An agent subscribes to a filter,
  for example all open issues and their status for one repository coordinate,
  and receives events as they arrive. There is no `gh issue list` poll, no
  pagination, and no GraphQL query cost. The claim ledger becomes a live
  subscription instead of a poll loop.
- **Optimistic local-first, eventual relay sync.** An agent can write a signed
  event locally and to a near relay, proceed immediately, and let the event
  replicate to mirror relays in the background. There is no merge-queue wait and
  no server-side PR-check gate on the critical path. The signature makes the
  event verifiable whenever it arrives, so eventual replication does not weaken
  authority.

### 4.3 The ceremony tax disappears

GitHub-centric flows carry ceremony that Nostr-native flows do not:

- **No issue-template gate.** The strict-bug form
  (`.github/ISSUE_TEMPLATE/strict-bug.yml`) exists to keep loose reports out of
  the human issue tracker. That gate is correct for public human intake. It is
  friction for an internal agent ledger. A kind 1621 issue event on an owned
  relay carries exactly the tags OpenAgents policy wants, no more.
- **No merge-queue ceremony.** A patch applied under the signed-state push rule
  produces a signed merge receipt directly. There is no server-side merge queue,
  required-check wait, or branch-protection round trip on the internal path.
- **No API pagination and search cost.** Reading the current ledger state is a
  relay filter, not a paginated REST walk plus rate-limited search.

### 4.4 Where Nostr is not faster, and how to mitigate

Honesty requires naming the cases where GitHub is even or better, and the
mitigations:

- **Initial clone of large history.** Git objects are large. A relay is a poor
  bulk object store. Mitigation: keep objects on a real git server, exactly the
  GRASP shape, and let Nostr carry only the announcement, the refs, and the
  collaboration events. The first clone is a normal git smart-HTTP fetch against
  an owned server, which is the same speed as any self-hosted git.
- **Global public discovery.** GitHub search and the GitHub social graph find a
  repository the whole world can see. A single owned relay has no global reach.
  Mitigation for the internal case: discovery is not needed, the fleet knows the
  one repository coordinate. Mitigation for the public case: publish
  announcements to public relays and to gitworkshop-compatible hosts later, as a
  separate stage.
- **Ecosystem tooling.** CI providers, dependency scanners, and review bots
  integrate with GitHub out of the box. Mitigation: keep GitHub as a read-only
  mirror during migration so existing integrations keep working while the
  internal ledger moves off the critical path.
- **Durability of a single relay.** One relay is a single point of failure.
  Mitigation: mirror events to more than one relay and mirror git objects to
  more than one server, and do not claim host failover until ref-agreement and
  recovery have tests (Buzz teardown §7.9 step 8).

---

## 5. The "immediately" fast path

The smallest reversible move that takes the internal agent workflow off the
GitHub critical path. Each stage is bounded, and each keeps GitHub as a mirror
so the move is reversible at any point. Dates are targets, not commitments.

### Stage 0, now — dogfood ngit as a client, zero owned build

Install the MIT `ngit` tool on the fleet. Run `ngit init` on one selected
OpenAgents repository to publish a 30617 announcement and a 30618 state event to
a public relay, with GitHub still canonical. Have one coding agent submit one
patch or one pull request over Nostr end to end and read it back. This is the
first receipt, it costs almost nothing, and the fleet runs `ngit` exactly as it
runs `git` today. `[EXISTS]` tooling. This proves the loop before any owned
code.

### Stage 1, week 1 — owned relay plus owned git server

Stand up the `nostr-effect` relay on Google Cloud, and colocate one git
smart-HTTP server on the same host, the GRASP-01 shape. Provision the bare
repository from an admitted 30617 announcement only, keeping OpenAgents
admission gates rather than ngit-relay's open provisioning. Enforce the signed-
state push rule, admit a push to `refs/heads/*` only when the commit matches the
latest maintainer-signed 30618. Mirror the OpenAgents repository objects to this
server. GitHub stays canonical and becomes a read mirror. `[NEEDS BUILD]`,
bounded, and it reuses the existing relay and the existing Google Cloud deploy
path.

### Stage 2, week 2 — move the Sol claim and issue ledger to the relay

Today `docs/sol/CLAIM_PROTOCOL.md` names the live GitHub issue set as the cross-
session claim ledger. Move that ledger to kind 1621 issues and kinds 1630-1633
status events on the owned relay. Each agent subscribes to the repository
coordinate and reads open work and claims as a live filter, and writes its
CLAIM, CLAIM-STATUS, and CLAIM-RELEASE as signed events. This removes the
GitHub content-creation ceiling from the hottest agent path, the claim ledger,
which every parallel agent writes. Keep a GitHub mirror of issues for human
visibility during migration. `[NEEDS BUILD]` as a ledger profile, and it is the
single highest-leverage move for fleet speed.

### Stage 3, weeks 3 to 4 — patches, reviews, and merge receipts internal

Route internal agent pull requests as kind 1617 patches or kind 1618 pull
requests to the owned relay, apply the OpenAgents review profile (§3, the
labeled-note review pattern under client trust rules), and publish a signed
merge-outcome event with the exact target ref, old object id, new object id,
policy version, and source proposal ids (Buzz teardown §7.9 step 7). Push to
GitHub as a mirror after the internal merge. `[NEEDS BUILD]`

### Stage 4, later — the public contributor forge

A `git-remote-nostr` transport helper for `nostr://` clone, multi-relay and
multi-host failover with tests, a browser forge UI, and a public identity model.
This is the larger product and it is explicitly out of the immediate move.
`[NEEDS BUILD]`, `[GAP]` on several pieces.

**The dividing line:** stages 0 to 3 take the internal agent workflow off
GitHub's critical path and are achievable now. Stage 4 is the public forge and
is a separate, larger decision.

---

## 6. Risks and boundaries

- **Identity, npub to author.** A signed event proves a key signed it, not who
  the person is. During the GitHub coexistence period, bind Nostr keys to GitHub
  identity with NIP-39 external-identity proofs (already in `nostr-effect`), and
  keep the mapping explicit. Do not let a maintainer-tag listing alone grant
  authority, gitworkshop's recursive-maintainer trust model is the working
  guard against the unilateral-listing attack. `[EXISTS]` NIP-39, `[NEEDS
  BUILD]` the trust profile.
- **Signing custody.** Never place a raw private key in an environment variable
  or a git config, which is a weakness the Buzz teardown §7.7 flags in the Buzz
  agent path. Put NIP-GS and the relay signer behind the sovereign signer with
  NIP-46 bunker custody. Give each agent and session an explicit signing role,
  and keep owner authorization separate from commit authorship. `[EXISTS]`
  signer, `[NEEDS BUILD]` the role wiring.
- **Relay availability and durability.** Git objects are large and a relay is
  not a bulk object store. Keep objects on a dedicated git server with relay-
  hinted clone URLs in the announcement, and mirror both events and objects to
  more than one host. Do not claim host failover from a list of URLs alone,
  require ref-agreement and recovery tests first (Buzz teardown §7.9 step 8).
- **Composition with the `forge/` control plane.** The private `forge/` repo
  owns software-factory lifecycle authority, work orders, runs, verification,
  and delivery. This Nostr forge is a hosting and collaboration substrate below
  that, not a replacement for it. The signed merge receipt and the CI-event
  surface should feed `forge/` evidence, not bypass it. `[GAP]` on the exact
  seam, to be designed with the `forge/` owner.
- **Composition with the Sol claim protocol.** Moving the ledger to Nostr
  events (Stage 2) must preserve the claim protocol's semantics exactly, the
  90-minute staleness rule, the process-or-worktree audit, and the collision
  rule for hot contracts. The event carries the same fields. The staleness and
  audit logic stays as written in `docs/sol/CLAIM_PROTOCOL.md`.
- **Private reads.** GRASP's no-auth reads fit public open source, not private
  customer repositories. For private repositories add the Buzz-style NIP-98 git
  credential and membership check, both postures behind one admitted policy, and
  do not import Buzz's membership coupling wholesale. `[NEEDS BUILD]`
- **GitHub as fallback mirror.** Keep GitHub a read-only mirror through every
  stage. The move is reversible while the mirror is live. Do not remove the
  mirror until the owned path has run a full fleet wave with receipts.
- **Dialect churn.** There are three pull-request dialects in the wild, stock
  NIP-34 patches, ngit's ref-pointer pull requests, and Buzz's target-branch
  pull requests. Pin exact commits and re-verify before any interop claim. For
  the internal move, pick one dialect, the plain 1617 patch is the safest, and
  do not chase cross-client interop until Stage 4.

---

## 7. Recommendation and next slice

The internal agent workflow can move off GitHub's critical path now, in the
bounded, reversible form of §5 Stages 0 to 3. The event vocabulary is already
owned code in `nostr-effect`. The relay is owned code. The git server and the
push-authorization hook are a small bounded build on the existing Google Cloud
deploy path. The highest-leverage single move is Stage 2, the Sol claim and
issue ledger on an owned relay, because every parallel agent writes that ledger
and it is exactly where the GitHub content-creation ceiling bites the fan-out.

The public contributor forge is a genuine larger product and this audit does not
promise it on the immediate timeline.

**Work-packet note.** Do not open a GitHub issue for this, repository policy
restricts issues to concrete reproducible bugs. Stage 0 (dogfood `ngit` on one
repository, publish one announcement, submit one patch over Nostr, read it back)
is a clean, bounded first slice that deserves an owner-accepted work packet under
the Sol claim protocol. It needs no owned code, it produces the first receipt,
and it de-risks every later stage. The event-layer note in §2.3 (the 1622-versus-
1111 comment kind) should be resolved in the same slice, since it blocks any
later interop claim.

---

## 8. Watch items

- **The `nostr-effect` 1622-versus-1111 comment kind** (§2.3) — resolve before
  any interop or public claim.
- **The `git-remote-nostr` helper** — required for `nostr://` clone and the
  public forge, not for the internal move. Watch `ngit` upstream for the shape.
- **CI events on Nostr** — a small unclaimed spec surface adjacent to OpenAgents
  verification receipts. Aligning kinds with the gitworkshop CI-event surface
  costs little and buys a rendering client.
- **The `forge/` seam** — how the signed merge receipt and CI-event surface feed
  the private control-plane evidence, to design with that repo's owner.
- **The sibling delegation document** — if
  `docs/nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md`
  lands on `main`, reconcile this forge design with it, since both ride the same
  relay and signer substrate.
- **GitHub rate-limit numbers** — re-verify the exact current limits (§4.1)
  before any external or public claim, they are policy that changes.
