# forge.openagents.com — Owned Coordination Layer + GitHub as Mirror (committed build plan)

Date: 2026-06-28 (rev 3 — origin.md architecture incorporated + build-roadmap)
Scope: How an arbitrarily large fleet of parallel coding agents coordinates
code collaboration. This revision supersedes the prior "stay on GitHub, maybe
later" audit: the owner has **decided**.
Audience: Artanis (operator) + the overseer loop + owner.
Status: Public-safe build plan. No secrets, no tokens, no deploy.

> Rev 3 note: the owner's architecture origin/vision doc
> (`docs/forge/origin.md` — a sourced competitive analysis of Cursor's "Origin,"
> the announced "git forge for the agentic era") is now **incorporated** into
> this audit. Its design patterns are merged into the architecture (§1.3, §2.2),
> its hardest open questions sharpen the risks (§7), and a concrete, fileable
> **Roadmap of issues** is added (§6, after M5). origin.md is the architecture
> reference; this audit remains the committed OpenAgents build plan. Where the
> two use different names for the same idea, the reconciliation is called out
> inline (e.g. Origin's "NVMe-front / S3-backing" storage ≙ our Cloudflare
> Artifacts-front / R2-backing store; Origin's "merge queue" ≙ our virtual merge
> queue; Origin's "change/stack" review unit ≙ our work-record + NIP-34 patch).

> Revision + move note: rev 1 of this audit lived at
> `docs/artanis/2026-06-28-gitafter-cloudflare-artifacts-coordination-audit.md`
> and weighed three options (stay on GitHub / GitAfter-NIP-34 / Cloudflare
> Artifacts), recommending "stay on GitHub, supplement later." **That
> recommendation is withdrawn, and this audit now lives under `docs/forge/`** —
> coordination infrastructure is a first-class owned product surface, not an
> operator-only note. The owned forge ships at **`forge.openagents.com`** (the
> "GitAfter" name is retired). **Naming: this is *the* OpenAgents forge and this
> name supersedes every previous use of "forge" in the workspace** — any earlier
> repo, doc, or product surface that used the word "forge" is superseded by this
> git forge (`forge.openagents.com`) as the canonical meaning going forward.
> This document is now a committed build plan with
> an honest risks section — not an options memo. The rev-1 factual research
> (incident root-cause, NIP-34 kinds, Cloudflare-stack inventory) is preserved
> because it still grounds the plan; the *conclusion* is what changed.

---

## 0. The decision (committed)

**We are fanning out to an arbitrary, large number of coding agents now.** Not
"if we ever hit dozens-to-hundreds." Now. The current single-host ~6–12 Codex
slots was never the target; it was the floor.

**GitHub's coordination layer is the bottleneck, and we are done fighting it.**
The constraints are structural to a centralized forge, not bugs we can out-code:

- **PR contention & merge serialization** — merging N PRs into one protected
  `main` is inherently serial; each merge can invalidate the others.
- **Branch protection requires a PR per change** — correct for human safety,
  but it gates throughput on review+merge, not on coding.
- **API & secondary (abuse-detection) rate limits** — enqueue/poll/push/comment
  traffic for a large fleet trips GitHub's secondary limits, which wedge the
  whole fleet at once.
- **Abuse-flagging risk** — autonomous agents writing and running adversarial
  harnesses and stress scripts risk a single org-level ban that instantly kills
  the coordination hub for *every* agent.
- **State-sync latency** — `push → webhook → pull` imposes a hard ~10–30s
  per-turn floor. Sub-second agent coordination loops are physically impossible
  on GitHub's network topology.

**Strategy: GitHub becomes a downstream, read-only MIRROR. We build and own the
real coordination layer ourselves, at `forge.openagents.com`.** The owned layer is
the source of truth; GitHub is a projection of it for humans and external
developers. The hard parts (merge convergence, dedup, "current patch," merge
authority, distribution) are *ours to solve* — and we are choosing to solve them
rather than rent a forge whose ceiling we have already hit.

`forge.openagents.com` is assembled from four pieces we already have primitives
for:

1. **Our own relay** — Nostr/NIP-34 lineage. `nostr-effect` already implements
   the NIP-34 kinds; `apps/nostr-relay/` is an owned relay surface.
2. **Cloudflare Artifacts** — Git-compatible, Durable-Object-backed repo store
   for per-agent/per-task isolated worktrees and build-output/artifact
   distribution, addressable from Workers, REST, and any git client.
3. **D1 + R2 as source of truth** — issues, PRs, and status as D1 rows; blobs
   (trace media, build outputs, raw events) in R2 / Cloudflare Artifacts.
4. **The virtual merge queue** — already landed (`apps/pylon/src/virtual-merge-queue.ts`),
   moving merge serialization off GitHub's slow API into deterministic local
   planning.

This is the decentralized/agent-native thesis (formerly "GitAfter") reframed as
an operational necessity and given a real home: `forge.openagents.com`.

---

## 1. Rationale — why this is forced, not premature

### 1.1 The incident proved the failure *mode*, not GitHub's innocence

Primary sources (real paths/commits in this workspace):

- After-action: `workspace:docs/afteraction/2026-06-28-overnight-fleet-after-action.md`.
- Incident report: `workspace:ARTANIS_PR_REPORT.md` — Artanis's verbatim
  accountability report on "119 PRs / 0 merged / 0 issues closed".
- Publisher dedup fix: commit `a542e056` — *"pylon: stop the Codex fleet from
  opening duplicate PRs (#6439 reopen)"*.
- Dispatch lockout fix: commit `e66876276f` — *"fix(codex-supervisor): skip
  CLOSED issues + dynamically refetch open issues"*.

A fleet of parallel Codex sessions pointed at one GitHub repo produced
**~119–123 open `pylon/assignment-*` PRs against only ~49 distinct issues**
(e.g. #6435 → 12 PRs), **0 merged**, **0 issues closed** for a long stretch. Two
of *our* bugs caused it (per-run not per-issue PR dedup; a dispatch lockout that
trusted a stale snapshot and did not skip CLOSED issues). Both are fixed.

Rev 1 read this as "GitHub was never the limiter, so stay on GitHub." That
reading is now rejected for one reason: **the fixes raise the ceiling; they do
not remove it.** Even with perfect per-issue PR identity and issue-close-safe
dispatch, the *next* wall is exactly the structural list in §0 — merge
serialization, secondary rate limits, abuse-flagging, and the 10–30s sync
floor. Those do not yield to two more commits. At arbitrary N they are the
binding constraint, and they are GitHub's, not ours to fix from inside.

The durable, generalizable failure modes (which recur for *any* centralized
forge) are the design targets for `forge.openagents.com`:

- **PR contention / duplication** — N agents on one work-unit → N PRs unless
  identity is keyed on the unit of *work*, not the unit of *execution*.
- **Stale-snapshot dispatch** — a long-running supervisor re-dispatches work
  that has since closed/merged unless it lives-refetches and fails closed.
- **Lockout/lease correctness** — "don't dispatch in-flight work" is a
  distributed-lock problem; the wrong predicate leaks duplicates.
- **Merge serialization & rebase storms** — overlapping diffs red each other on
  the combined tree; without a queue this is a manual O(N) rebase loop.

Every one of these is something the owned layer must own *anyway*. We are not
escaping the hard problems by leaving GitHub; we are choosing to solve them in a
substrate we control, at a latency (local memory / D1, not webhooks) where they
are tractable.

### 1.2 We are already 80% Cloudflare-native and NIP-34-ready

`forge.openagents.com` is mostly assembly of things that exist:

- **Virtual merge queue: landed.** `apps/pylon/src/virtual-merge-queue.ts`
  (pure planner: `simulateVirtualMergeQueue`, path-conflict detection,
  deterministic virtual-HEAD projection) +
  `apps/pylon/src/blueprint-gates/virtual-merge-queue.ts` (candidate gating:
  `issue_closed` / `open_pr_exists` / `verification_not_passed` / `stale_base`
  blocked-reason refs, `nextActualPromotion`) +
  `apps/pylon/scripts/codex-supervisor/virtual-merge-queue.sh` (the supervisor
  branches new work from a projected virtual HEAD, not stale `origin/main`).
- **Priority dispatch (#6711): landed.**
  `apps/pylon/scripts/codex-supervisor/priority-dispatch.sh` — label-priority
  tiers (`prio:0-pr-burndown` … `prio:4-backstop-burn`) so slots never idle and
  always burn the highest-value tier first, with the existing `pick_unlocked_issue`
  lockout falling through locked/empty tiers.
- **Fan-out coordinator: landed.** `apps/pylon/src/coordinator/coordinator-runtime.ts`
  + `planner.ts` + `coordinator-state.ts` — a typed intent state machine
  (`received → planning → fanning_out → shipping → shipped/failed`) that splits
  one intent into parallel sessions and reconciles their terminal states. This
  is the arbitrary-N fan-out engine the owned layer slots under.
- **Autonomous-ops governance: specced.**
  `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md` —
  five typed Blueprint Signatures whose terminal state is the only thing that
  unlocks a consequential action (see §5).
- **NIP-34 primitives: implemented.** `nostr-effect/src/core/Nip34.ts` (908
  lines, full round-trip tests) exports the canonical kind constants and
  `Repository`/`Patch`/`PullRequest`/`Issue`/`Status` types; `apps/nostr-relay/`
  is an owned relay surface.
- **Cloudflare substrate: in production.** R2 (`openagents-autopilot-artifacts`,
  `env.ARTIFACTS`), D1 (`openagents-autopilot`), Durable Objects (SyncRoom, MDK
  sidecars, inference stream), Queues, KV, Containers, Browser Rendering — all
  already bound in `apps/openagents.com/workers/api/wrangler.jsonc`.

We are not starting a protocol from scratch. We are wiring owned primitives into
a coordination control plane at `forge.openagents.com` and demoting GitHub to a
mirror.

### 1.3 Industry validation — the best-funded competitor reached the same conclusion

The architecture origin/vision doc (`docs/forge/origin.md`) is a sourced
analysis of **Cursor's "Origin,"** announced June 2026 as a "git forge for the
agentic era." We do not adopt Cursor's product; we read it as **independent
convergent evidence that the thesis in §0 is correct, not premature**, and we
mine it for concrete architecture patterns (folded into §2.2) and the hardest
open questions (folded into §7).

What origin.md confirms about the bet we are making:

- **Generation got cheap; coordination got expensive.** Cursor's own framing
  (via the Graphite acquisition) is that "writing code became faster while
  reviewing changes, merging them safely, and collaborating effectively" became
  the bottleneck. That is *exactly* §0: GitHub's coordination layer — not coding
  capacity — is the wall. A company with Cursor's resources choosing to build a
  forge rather than rent one is the strongest available signal that the
  coordination layer is worth owning.
- **Humans and agents as co-equal first-class authors.** Origin's stated primary
  user model is "you and your agents create repos, share code, and manage
  changes." This is our agent-keyed NIP-34 identity model (§3.2): agents are
  first-class keyed authors, not bot accounts borrowed on someone else's
  platform.
- **The incumbent forge is not built for agent concurrency.** Cursor reportedly
  simulated *thousands* of agents reading/writing one repo and "went back to
  basics" on Git architecture for it. That is the §0 structural-ceiling argument,
  stress-tested by someone else.

Where we **differ from Origin** — and why our plan is not a clone:

- **We keep GitHub as a downstream mirror, not a migration.** Origin asks teams
  to move their code onto a new forge (with all the unpublished migration,
  governance, and trust questions in origin.md §8). Our cut (§4) keeps GitHub as
  a read-only projection, so external developers and customer-repo work
  (irreducibly GitHub — see risk #6) keep working. No migration ask.
- **We have a native economic loop.** Origin is hosting + review + agent
  automation. Our relay layer (§3) carries the bounty→claim→merge→settlement
  economic kinds and trajectory-proof tags — coordination *and* the labor-market
  rails in one substrate. Origin has no equivalent.
- **We are Cloudflare-native and ~80% assembled today** (§1.2), not building
  bespoke NVMe Git fileservers. origin.md's reported "NVMe-front / S3-backing"
  storage maps directly onto Cloudflare Artifacts (DO-backed) + R2 (§2.2), which
  we already operate.

origin.md's reported Origin throughput figures (≈22.6 commits/s into one repo,
≈296k clones/hr, ≈81k pushes/hr, <400 ms global sync, <10 ms failover) are
**staged-demo benchmark claims, not verified SLAs** — but they give us concrete
**load-test targets** for the convergence/verification risks in §7 (#1, #2),
which is how we use them in the roadmap (§6).

---

## 2. Target architecture

```
                       ┌─────────────────────────────────────────────┐
                       │   forge.openagents.com (source of truth)       │
                       │                                              │
   agents ───jobs────► │  D1: issues / PRs / status / dispatch        │
   (arbitrary N)       │      lockout / lease rows                    │
        ▲              │                                              │
        │ dispatch     │  Virtual Merge Queue (pylon, pure planner)   │
        │ (priority    │   projected virtual HEAD + path-conflict     │
        │  tiers) +    │   gating → nextActualPromotion               │
        │ fan-out      │                                              │
        │ coordinator  │  Cloudflare Artifacts (DO-backed git store)  │
        │  worktrees ──┤   one repo per agent/task; fork from main;   │
        │              │   git-over-HTTPS; readTree/log/readCommit    │
        │              │                                              │
        │  blobs ──────┤  R2: build outputs, trace media, raw events  │
        │              │                                              │
        │  events ─────┤  Relay (nostr-effect NIP-34 + apps/nostr-relay)
        │              │   repo / patch / issue / status events,      │
        │              │   sub-second fan-out, agent-keyed identity    │
        │              │                                              │
        │  governance ─┤  Blueprint Signatures (autonomous-ops-v1):   │
        │              │   liveness / diagnosis / issue-close-safe /   │
        │              │   command-verified / merge-deploy gates       │
        └──────────────┤                                              │
                       └───────────────────┬──────────────────────────┘
                                           │ MIRROR (one-way, downstream)
                                           ▼
                       ┌──────────────────────────────────────────────┐
                       │  GitHub OpenAgentsInc/openagents (read-only)   │
                       │  human review surface · external developer     │
                       │  export · cold-storage / disaster mirror       │
                       └──────────────────────────────────────────────┘
```

### 2.1 Component responsibilities

- **D1 = source of truth for coordination state.** Issues, PRs (as work
  records), status transitions, dispatch lockout/lease rows, and the merge-queue
  ledger live as typed D1 rows. This is the authority GitHub mirrors *from*,
  not the other way around. (D1 already holds `pylon_api_assignments`,
  `agent_traces`, `token_usage_events` — this extends the same pattern.)
- **Cloudflare Artifacts = the hot, isolated git store for agent worktrees.**
  One repo per agent / task / branch, forked from the canonical tree, with
  git-over-HTTPS access via short-lived scoped tokens (`createToken(scope, ttl)`)
  and `readTree`/`readCommit`/`log` from the Workers binding. This is the
  "git without GitHub" surface — agents get the `git` they already know, with
  per-execution isolation and no shared-repo PR contention. Built on Durable
  Objects, durable by default, designed for tens of millions of repos.
- **R2 = blobs.** Build outputs, distilled-test baselines, trace media, raw
  Codex event chunks — opaque artifacts keyed by content/assignment ref. Already
  in flight via `env.ARTIFACTS` and `/api/traces/{uuid}/blob/{r2Key}` (#6223).
- **Relay = the event bus / lineage layer.** NIP-34 repo/patch/issue/status
  events (kinds in §3) carry agent-keyed identity, sub-second fan-out (no
  webhook round-trip), and the native bounty→claim→merge→settlement economic
  loop. `apps/nostr-relay/` is the owned surface; `nostr-effect` is the codec.
- **Virtual merge queue = the merge authority.** The supervisor projects a
  virtual HEAD (main + all in-flight green-verified tasks), branches new work
  from it, detects path conflicts deterministically in local memory, and emits
  `nextActualPromotion` — so promotion is a trivial fast-forward, not an O(N)
  rebase storm on GitHub's API.
- **GitHub = downstream mirror.** A projection for humans and external
  developers; see §4.

### 2.2 Patterns reconciled from origin.md (Cursor Origin)

origin.md describes Origin's architecture in five layers (IDE/agents → agent
orchestration → forge API/MCP/app-platform → forge services → Git storage
plane). Mapped onto our owned stack, each layer is something we already have a
home for; the value of the mapping is that it surfaces three patterns we should
make explicit and one we should add.

| origin.md (Cursor Origin) pattern | OpenAgents `forge.openagents.com` equivalent | Status |
|---|---|---|
| NVMe-front Git fileservers / S3 source-of-truth / global replicas | **Cloudflare Artifacts (DO-backed, hot git store) front + R2 (durable blobs) back** (§2.1) | We operate this substrate; Artifacts is beta (risk #3) |
| Merge queue / stacked changes | **Virtual merge queue** (`apps/pylon/src/virtual-merge-queue.ts`) (§5.2) | Landed |
| Review unit = "change"/"stack", not raw commit | **Work record (D1 `coordination_prs`) + NIP-34 patch (1617) / PR (1618)** | M1/M3 |
| Agent change-management loop (observe→classify→plan→patch→push) | **Fan-out coordinator + Blueprint Signatures** (§5.1) | Coordinator landed; loop to formalize |
| Automated merge-conflict resolution | **Virtual-queue path-conflict detection → agent rebase from projected HEAD** | Detection landed; auto-resolve is M3+ |
| CI-failure repair / comment-as-task | **`merge-deploy-gate` signature + NIP-22 (1622) replies as dispatch inputs** | Gate specced; comment-dispatch is M4 |
| API / MCP / third-party app platform | **Relay (NIP-34) + REST over the Worker + MCP surface** | **Add explicitly (M5 multi-tenant)** |
| Git compatibility (existing `git` tooling works) | **git-over-HTTPS on Artifacts; agents use the `git` they know** | M2 |

The three patterns origin.md makes us state explicitly:

1. **The "change," not the commit, is the unit of work.** Origin (via Graphite)
   treats the reviewable *change/stack* as primary. Our D1 `coordination_prs`
   rows are exactly that work record — keyed on the unit of *work*, not the unit
   of *execution* (the same identity discipline that fixed the 119-PR storm,
   §1.1). M1/M3 should treat the work record, not a git branch, as the canonical
   coordination object, with NIP-34 patches/PRs as its transport form.
2. **The change-agent loop is a first-class state machine.** Origin's internal
   loop — *observe PR state → classify blocker (conflict / CI-fail / comment /
   missing-approval / stale-branch) → gather context → plan → apply patch → push
   → wait for checks-or-human* — is precisely what our coordinator
   (`coordinator-runtime.ts`) + Blueprint Signatures already encode, but it is
   currently spread across scripts and a doc. The roadmap (§6) files an issue to
   make this the named, typed forge change-loop with the blocker taxonomy above
   as its classification enum, so "what is blocking this PR and what happens
   next" is one queryable state, not tribal knowledge.
3. **CI/comment events are actionable tasks, not just messages.** In an
   agent-native forge a failed check or a review comment *is* a dispatch input.
   Our NIP-22 (1622) replies and `merge-deploy-gate` results should feed the
   priority dispatcher (a comment "add tests" or a red `check:deploy` becomes a
   `prio:*`-labeled work record), closing the loop origin.md §3.6/§3.5 describe.

The one pattern origin.md tells us to **add now rather than defer**: a
**first-class extensibility surface (API + MCP + app platform)**. Origin treats
this as a top-two design point because agents must read/act on forge state
*structurally* (ask "what's blocking this PR?", fetch comments, inspect CI,
create branches, take follow-ups) instead of scraping a UI. We already have the
transport (relay + Worker REST); the roadmap files the MCP-surface issue in M4
so "Artanis-as-a-Service" multi-tenant (M5) has a structured agent API from day
one, not bolted on later.

---

## 3. NIP-34 / relay layer — the lineage substrate

### 3.1 The event model (codec already implemented)

NIP-34 (spec: `projects/repos/nips/34.md`; codec: `nostr-effect/src/core/Nip34.ts`)
expresses git collaboration as relay events:

| Kind | Meaning | In `nostr-effect` |
|---|---|---|
| 30617 | Repository announcement (maintainers, clone URLs, relays, earliest-unique-commit) | `GitRepoAnnouncement` |
| 30618 | Repository state (branches/tags/HEAD) | `GitStateAnnouncement` |
| 1617 | Patch (`git format-patch` content) | `GitPatch` |
| 1618 / 1619 | Pull request / PR update (commit tip + clone URLs) | (PR builder/parser) |
| 1621 | Issue (markdown + labels) | `GitIssue` |
| 1622 | Reply / comment (NIP-22) | `GitReply` |
| 1630 / 1631 / 1632 / 1633 | Status: open / applied-merged / closed / draft | `GitStatus{Open,Applied,Closed,Draft}` |
| 10317 | "Grasp" (git-hosting) server list | (grasp list builder/parser) |

The archived design (`backroom/.../crates/nostr/GIT_AFTER.md`) also sketched
economic kinds (issue-claim / work-assignment / bounty-offer / bounty-claim) and
trajectory-proof tags (`trajectory` / `trajectory_hash` / `policy_bundle_id`).
Those are our extension surface for the bounty→merge settlement loop, not yet in
the codec's test suite.

### 3.2 Why the relay matters for arbitrary-N

The relay is the piece that directly kills GitHub's **state-sync latency** and
**single-writer merge** constraints:

- **No single-writer merge bottleneck.** "Merge" is a maintainer publishing a
  1631 status event (with `merge-commit` / `applied-as-commits`); there is no
  protected branch a central server must serialize into.
- **Agents as first-class keyed authors**, not bot accounts on someone else's
  platform — no per-tenant GitHub App install / PAT / org-permission nightmare,
  and no shared org-ban blast radius.
- **Sub-second fan-out.** Relay events replace `push→webhook→pull`, removing the
  ~10–30s floor that makes tight agent loops impossible on GitHub.

The honest cost: every coordination problem (dedup, "current patch"
convergence, merge authority) becomes *our* relay/application logic. We accept
this — it is the same logic the virtual merge queue + D1 truth + Blueprint
signatures already implement; the relay is the transport, D1 is the authority,
and convergence is resolved by the queue, not by hoping events agree. See §7.

---

## 4. The mirror mechanism — how GitHub stays downstream

GitHub is written to, never read from, for coordination authority.

- **Direction.** `forge.openagents.com` (D1/Artifacts) is the source of truth. A
  mirror worker projects state *outward* to GitHub. GitHub webhooks are NOT a
  coordination input on the owned path (they may remain a convenience for
  human-opened PRs during transition — see §6).
- **What is mirrored.** Merged/promoted commits (the canonical tree), and
  optionally a human-readable projection of issues/PRs/status for external
  reviewers. Build artifacts and blobs are **not** mirrored to GitHub (they live
  in R2/Artifacts).
- **When.** On `nextActualPromotion` from the virtual merge queue: after a
  candidate clears its Blueprint gates (§5) and is promoted to the actual head
  in the owned layer, the mirror worker fast-forwards / pushes that commit to
  GitHub as a downstream effect. Promotion is decided by the owned layer; the
  GitHub push is a trailing projection, not a gate.
- **Read-only posture.** The mirror repo is treated as export/cold-storage: a
  disaster-recovery copy, a human review surface, and an external-developer
  export. Branch protection on the GitHub repo remains as a courtesy guard for
  any human contributor, but the fleet does not depend on GitHub merge for
  throughput.
- **Rate-limit safety.** Because the fleet no longer opens N PRs / polls a
  native queue / spams comment updates, GitHub API traffic collapses to a small
  number of trailing pushes — well under secondary-limit thresholds and far from
  abuse-flagging behavior.

This is the inversion: today GitHub serializes us; after the cut, GitHub is a
side effect of a merge `forge.openagents.com` already decided locally.

---

## 5. How autonomous-ops-v1, the merge guard, and priority-dispatch fit

The owned layer is only safe at arbitrary N because the governance and ordering
primitives already exist. They are not new asks; they are the control plane.

### 5.1 Blueprint Signatures (autonomous-ops-v1) = the gate authority

`docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md` defines
five typed signatures, each a typed I/O contract + ordered evidence-ref
predicate list + state model whose **terminal state is the only state that
unlocks the consequential action**. Mapped onto the owned layer:

1. **`fleet-liveness-dispatch-proof`** — `PROVEN_ALIVE` requires a real dispatch
   attempt + quota-ledger read + heartbeat within 10 min. Makes the "alive but
   not dispatching" wedge impossible. This is what lets us trust an
   arbitrary-N fleet is actually producing work, not just running.
2. **`diagnosis-grounding`** — no root-cause claim ("rate-limited") without the
   quota-ledger read + provider 429 headers. The instrument that tells us
   *which* constraint is binding as N grows (so we scale on data, not vibes).
3. **`issue-close-safe`** — closing requires labels read + parent-epic check
   (an epic can't be auto-closed by one sub-PR) + PR body `Closes #XXXX`. This
   is the **merge/close guard (#6723)** in signature form. It is the D1-side
   authority for the merge queue's `issue_closed` / `open_pr_exists` blocked
   reasons.
4. **`command-execution-source-verified`** — no command recommended without
   reading + hashing its source and verifying flags. Keeps an autonomous fleet
   from fabricating executables.
5. **`merge-deploy-gate`** — `LIVE` requires `check:deploy` green + deploy exit
   code + smoke results; any failure blocks all further merges until rollback
   evidence exists. This is the gate the virtual merge queue's
   `nextActualPromotion` must clear before the mirror worker pushes to GitHub.

In the owned layer these signatures move from "Artanis discipline" to
**structural preconditions on D1 writes**: a promotion/close/merge row cannot be
written without its evidence refs, so the consequential action is *impossible*,
not merely discouraged. That is exactly what makes self-owned merge authority
safe to trust at scale.

### 5.2 The virtual merge queue (#6723 merge guard, landed)

`apps/pylon/src/blueprint-gates/virtual-merge-queue.ts` is the merge guard:
candidates are gated by `issue_closed`, `open_pr_exists`,
`verification_not_passed`, `stale_base`, `invalid_commit`, `duplicate_issue`
blocked-reason refs; `simulateVirtualMergeQueue` projects a deterministic
virtual HEAD and flags path conflicts
(`blocker.public.pylon_virtual_merge_queue.path_conflict`) before any GitHub
operation. In the owned architecture this *is* the merge authority — promotion
is decided here, then mirrored.

### 5.3 Priority dispatch (#6711, landed)

`apps/pylon/scripts/codex-supervisor/priority-dispatch.sh` orders the
dispatchable pool by `prio:0-pr-burndown` … `prio:4-backstop-burn` so an
arbitrary-N fleet never idles a slot and always burns the highest-leverage tier
first, falling through locked/empty tiers via the existing lockout. This is the
saturation engine that *makes* "arbitrary large N" a real operating mode rather
than an aspiration.

---

## 6. Staged build plan

Each milestone is independently shippable and cuts a specific GitHub dependency.
Ordering is by leverage: kill the merge-serialization and contention path first
(it's the one that produced the 119-PR night), then the latency/identity path,
then full source-of-truth inversion at `forge.openagents.com`.

### M0 — Foundations already landed (done)

- Per-issue PR identity (`a542e056`) + issue-close-safe dispatch lockout
  (`e66876276f`).
- Virtual merge queue planner + gates (`apps/pylon/src/virtual-merge-queue.ts`,
  `apps/pylon/src/blueprint-gates/virtual-merge-queue.ts`, supervisor `.sh`).
- Priority dispatch (#6711). Fan-out coordinator
  (`apps/pylon/src/coordinator/`). Autonomous-ops-v1 signatures specced.

### M1 — D1 becomes the coordination source of truth (cut: stale-snapshot dispatch)

- Define D1 tables for `coordination_issues`, `coordination_prs` (work records),
  `coordination_status` (NIP-34-aligned open/applied/closed/draft),
  `dispatch_lease`, and `merge_queue_ledger`.
- Make the supervisor read/write dispatch lockout + merge-queue state from D1
  rows instead of `gh`-fetched snapshots. The virtual merge queue persists its
  projection to `merge_queue_ledger`.
- **Result:** dispatch and lockout no longer depend on GitHub API reads; live
  truth is local. The "stale `SUP_ISSUES` snapshot" failure class is gone.

### M2 — Cloudflare Artifacts as the agent worktree store (cut: shared-repo PR contention)

- Stand up an Artifacts namespace behind `forge.openagents.com`; on dispatch,
  `fork`/`import` the canonical tree into a per-task Artifacts repo and hand the
  agent a short-lived scoped write token (`createToken(scope, ttl)`).
- Agents push patches to their isolated Artifacts repo over git-over-HTTPS; the
  supervisor reads candidates via `readTree`/`log`/`readCommit` and feeds them to
  the virtual merge queue.
- **Result:** N agents no longer contend on one shared GitHub repo; each has an
  isolated git home. PR-contention/duplication at the git layer disappears.
- **Gate:** Artifacts is in beta — M2 ships behind beta access; until then the
  same flow runs against owned per-task git repos / R2-backed bare repos as a
  fallback so the architecture does not block on Cloudflare's access timeline.

### M3 — Owned merge authority + GitHub mirror worker (cut: merge serialization + GitHub merge dependency)

- The virtual merge queue's `nextActualPromotion`, gated by the
  `merge-deploy-gate` + `issue-close-safe` signatures, promotes a candidate to
  the canonical head **in the owned layer** (D1 + Artifacts).
- A mirror worker pushes the promoted commit to GitHub as a trailing,
  read-only projection (§4). Optionally project issue/PR/status rows to GitHub
  for human reviewers.
- **Result:** merges are decided locally at memory/D1 speed; GitHub is
  downstream. Secondary-rate-limit and abuse-flag risk collapse with PR volume.

### M4 — Relay event layer for sub-second coordination (cut: 10–30s sync latency + bot-identity)

- Publish repo/patch/issue/status events (kinds in §3) to `apps/nostr-relay/`
  via `nostr-effect`. Agents get keyed identity and sub-second fan-out; the
  relay is the event bus, D1 stays the authority, the queue stays the merge
  resolver.
- **Result:** agent coordination loops drop below GitHub's network floor;
  agents are first-class keyed authors, not org bot accounts.

### M5 — Economic loop + multi-tenant (the agent-native thesis realized)

- Extend the relay with trajectory-proof tags + bounty/claim kinds; wire
  bounty→claim→patch→merge→settlement (NIP-57 + trajectory proof) into the
  existing payout rails.
- Per-tenant isolation via per-tenant Artifacts namespaces + relay keys (no
  per-tenant GitHub org needed), enabling "Artanis-as-a-Service" on
  `forge.openagents.com` without GitHub as the multi-tenant blocker.

### Roadmap of issues — what to build first (fileable)

> Implementation note, 2026-06-28: the live GitHub sub-issues under #6745 now
> use a shorter first-wave numbering than the original fileable list below.
> Treat GitHub as the operative execution queue for this pass:
> #6746 / FORGE-1 = D1 schema, #6747 / FORGE-2 = local receive-pack parser,
> #6748 / FORGE-3 = R2 packfile archiving, #6750 / FORGE-4 = tenant scoped git
> tokens, #6751 / FORGE-5 = Pylon-to-Forge dispatch protocol, and #6752 /
> FORGE-6 = Docker-isolated verification runner. The milestone architecture and
> dependency logic below remain the longer-term map; the live issue set is the
> committed first implementation slice.

> FORGE-0 first-wave completion, 2026-06-28: the live GitHub issue slice
> (#6746, #6747, #6748, #6750, #6751, #6752) is complete and pushed to `main`.
> The owned Forge foundation now has D1 source-of-truth rows, typed protocol
> schemas, a local receive-pack parser, R2 packfile metadata archiving,
> tenant-scoped git access tokens, Pylon dispatch messages, and a
> Docker-isolated Bun verification runner. The original longer roadmap below
> remains the next implementation map; the first-wave GitHub epic can close
> because its concrete sub-issues are done.

> Stand-up issue map, 2026-06-28: #6759 closed the separate `apps/forge/`
> deploy bootstrap and production `forge.openagents.com` landing page. #6769
> now expands that app into the SU-1B shell with work, change, verification,
> queue, and ref routes plus `/shell.json` public-safe contract metadata. The
> next filed implementation slices are #6770 / SU-2 `/api/forge/*`
> control-plane routes and #6771 / SU-3 smart-Git intake to archive/canonical
> refs/coordination rows.
> #6768 is anchored by `docs/forge/2026-06-28-forge-boundary-contract.md` and
> the `ForgeControlPlaneScope`, `ForgeVerificationReceipt`, and
> `ForgePromotionDecisionReceipt` schemas in `@openagentsinc/forge-protocol`.
> These live issue numbers are the operative queue for the stand-up sequence in
> `docs/forge/2026-06-28-forge-standup-spec.md`.

> FORGE-1 status, 2026-06-28: Worker-side D1 coordination source-of-truth rows
> landed in migration `0251_forge_coordination_source_of_truth.sql`, with typed
> Effect schemas in `@openagentsinc/forge-protocol` and the store/tests at
> `apps/openagents.com/workers/api/src/forge-coordination-store.ts`. The first
> row set covers Forge issues, PR/change records, NIP-34-aligned status rows,
> dispatch leases, and merge-queue ledger snapshots. Stores decode every row
> through the shared protocol package so Workers and Pylon do not fork local
> coordination contracts.

> FORGE-2 status, 2026-06-28: Pylon now has a local pure
> `git-receive-pack` parser at `apps/pylon/src/git-receive-pack.ts`. The parser
> validates pkt-line framing, ref update commands, SHA-1/SHA-256 object IDs,
> first-line capabilities, safe `refs/*` names, delete-only pushes, and the
> trailing `PACK` payload before any future storage or GitHub projection layer
> can accept the bytes. Its output is the first supervisor-owned commit-intake
> record: command list + capabilities + packfile byte count + packfile SHA-256
> digest + source refs. R2 archiving and token-scoped access build on that
> record in the following live Forge issues.

> FORGE-3 status, 2026-06-28: Worker-side packfile archiving now has a private
> R2 + D1 store at
> `apps/openagents.com/workers/api/src/forge-git-packfile-archive-store.ts` and
> migration `0252_forge_git_packfile_archives.sql`. R2 stores the raw
> `application/x-git-packed-objects` bytes under deterministic private keys;
> D1 stores tenant/repository/change refs, receive-pack ref, SHA-256 digest,
> byte count, object format, capabilities JSON, ref-update JSON, and source refs.
> The store is idempotent by `(tenant_ref, packfile_ref)` and by tenant-scoped
> digest so repeated receive-pack submissions do not duplicate blob storage.

> FORGE-4 status, 2026-06-28: tenant-scoped git auth now has D1 tables in
> `0253_forge_tenant_git_access_tokens.sql` and a Worker store at
> `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.ts`. Raw
> `oa_forge_git_` tokens are returned only at mint time; D1 stores token hashes,
> prefixes, tenant/repository/subject refs, lifecycle timestamps, and one bounded
> scope row per grant (`git:upload-pack`, `git:receive-pack`, `git:admin`). Auth
> fails closed unless the tenant is active, the token is active and unexpired,
> the repository matches exactly, and the requested git operation is granted.

> FORGE-5 status, 2026-06-28: the Pylon-to-Forge dispatch contract now lives in
> `@openagentsinc/forge-protocol` as typed `work_item`, `decision`, and
> `closeout` messages. Work items carry tenant/work/lease refs, objective
> summary, scoped git target, short-lived git token ref/prefix, and optional
> verification command descriptors. Pylon maps those messages into existing
> `assignment_lease.v0.3` records via
> `apps/pylon/src/forge-dispatch-protocol.ts`, then maps assignment closeouts
> back into Forge closeouts with change, packfile, verification, artifact,
> proof, receipt, result, and source refs. Raw git tokens remain out of the
> persistent dispatch payload; only token refs/prefixes and scopes are carried.

> FORGE-6 status, 2026-06-28: Pylon now has a Docker-isolated Bun verification
> runner at `apps/pylon/src/forge-verification-runner.ts`. It consumes the
> FORGE-5 `ForgeDispatchVerificationCommand`, enforces the
> `forge.verification.runner.docker_bun.v0.1` runner ref, and builds a shell-free
> `docker run` argv with `--network none`, `--pull=never`, read-only rootfs,
> read-only workspace mount, dropped capabilities, `no-new-privileges`, noexec
> tmpfs mounts, and CPU/memory/PID/timeout limits. The runner returns a
> public-safe receipt with refs, byte counts, digests, status, exit code, and
> isolation settings only; raw stdout, stderr, source, provider payloads, git
> tokens, and wallet material stay out of the persisted result. The companion
> doc is `apps/pylon/docs/forge-docker-verification-runner.md`.

This is the build plan in fileable form: an ordered list of issues mapped to the
milestones above, with dependency order and a P0-now set. Each line is
`**FORGE-n** (milestone) [Pn] — Title — one-line scope`. These are tracking
items for the forge epic; per repo policy GitHub *issues* are reserved for
strict-bug reports (blank issues disabled, features go to the Forum), so this
roadmap is the canonical fileable spec — file these as epic sub-tasks /
checklist items under **FORGE-0**, labeling build-now items `prio:0-pr-burndown`
and referencing the forge epic, rather than as loose blank issues.

**Epic**

- **FORGE-0** (epic) [P0] — *forge.openagents.com owned coordination layer
  (epic)* — umbrella tracking item linking this audit and every sub-task below;
  the parent all forge work references. Label `prio:0-pr-burndown`.

**M1 — D1 becomes the coordination source of truth** (cuts stale-snapshot
dispatch; the failure class behind the 119-PR night). *Do first.*

- **FORGE-1** (M1) [P0] — *D1 coordination schema migration* — add
  `coordination_issues`, `coordination_prs` (work records), `coordination_status`
  (NIP-34 open/applied/closed/draft), `dispatch_lease`, `merge_queue_ledger`
  tables + Effect Schema row types. Blocks everything else in M1+. Depends:
  FORGE-0.
- **FORGE-2** (M1) [P0] — *Supervisor dispatch lockout/lease from D1* — replace
  `gh`-fetched snapshot reads with D1 `dispatch_lease` rows; fail closed. Depends:
  FORGE-1.
- **FORGE-3** (M1) [P0] — *Persist virtual-merge-queue projection to D1* — write
  the projected virtual HEAD + blocked-reason refs to `merge_queue_ledger`.
  Depends: FORGE-1.
- **FORGE-4** (M1) [P1] — *Typed forge change-loop state machine* — formalize the
  observe→classify→plan→patch→push loop (origin.md §2.2) over
  `coordinator-runtime.ts` + Blueprint Signatures, with a blocker-taxonomy enum
  (`conflict` / `ci_fail` / `comment` / `missing_approval` / `stale_branch`) as
  one queryable state. Depends: FORGE-1.

**M2 — Cloudflare Artifacts as the agent worktree store** (cuts shared-repo PR
contention). Depends: M1.

- **FORGE-5** (M2) [P0] — *Artifacts namespace + per-task fork + scoped tokens* —
  stand up the Artifacts store behind `forge.openagents.com`; on dispatch
  fork/import the canonical tree and mint a short-lived scoped write token
  (`createToken(scope, ttl)`). Depends: FORGE-1.
- **FORGE-6** (M2) [P1] — *Supervisor reads candidates from Artifacts* — pull
  agent patches via `readTree`/`log`/`readCommit` and feed the virtual merge
  queue. Depends: FORGE-5.
- **FORGE-7** (M2) [P1] — *R2-backed bare-repo fallback* — owned per-task git /
  R2 bare repos so M2 ships even if Artifacts beta access stalls (risk #3).
  Depends: FORGE-1; sibling of FORGE-5.

**M3 — Owned merge authority + GitHub mirror worker** (cuts merge serialization
+ GitHub-merge dependency). Depends: M1, M2.

- **FORGE-8** (M3) [P0] — *Owned promotion via signatures* — `nextActualPromotion`
  gated by `merge-deploy-gate` + `issue-close-safe` writes the promotion row to
  D1 as the canonical merge authority. Depends: FORGE-3.
- **FORGE-9** (M3) [P0] — *GitHub mirror worker* — fast-forward the promoted
  commit to GitHub as a trailing, read-only projection (§4). Depends: FORGE-8.
- **FORGE-10** (M3) [P2] — *Human-readable GitHub projection* — optionally mirror
  issue/PR/status rows to GitHub for external reviewers. Depends: FORGE-9.
- **FORGE-11** (M3) [P1] — *Signature evidence as D1 write preconditions* —
  enforce Blueprint-signature evidence refs structurally on promotion/close/merge
  D1 writes (§5.1). Depends: FORGE-1, FORGE-8.

**M4 — Relay event layer** (cuts 10–30s sync latency + bot-identity). Depends:
M1.

- **FORGE-12** (M4) [P1] — *Publish NIP-34 events to the owned relay* —
  repo/patch/issue/status events (kinds §3) via `nostr-effect` to
  `apps/nostr-relay/`. Depends: FORGE-1.
- **FORGE-13** (M4) [P1] — *Agent-keyed identity + sub-second fan-out* — relay as
  event bus, D1 stays authority. Depends: FORGE-12.
- **FORGE-14** (M4) [P2] — *Comment/CI events as dispatch inputs* — NIP-22 (1622)
  replies + `merge-deploy-gate` results feed the priority dispatcher (origin.md
  §2.2 pattern 3). Depends: FORGE-12, FORGE-4.
- **FORGE-15** (M4) [P1] — *Forge MCP surface* — structured agent API (what's
  blocking this PR / fetch comments / inspect CI / create branch / take
  follow-up) so multi-tenant has a real API day one (origin.md §3.7). Depends:
  FORGE-12.
- **FORGE-16** (M4) [P1] — *Relay abuse/replay defense* — extend the `#6643`
  threat model to relay events (risk #4). Depends: FORGE-12.

**M5 — Economic loop + multi-tenant** (the agent-native thesis realized).
Depends: M3, M4.

- **FORGE-17** (M5) [P2] — *Bounty→settlement economic loop* — relay economic
  kinds + trajectory-proof tags wiring bounty→claim→patch→merge→settlement
  (NIP-57) into payout rails. Depends: FORGE-12, FORGE-8.
- **FORGE-18** (M5) [P2] — *Per-tenant Artifacts + relay keys* —
  Artanis-as-a-Service with no per-tenant GitHub org. Depends: FORGE-5, FORGE-13.

**Cross-cutting / risk-driven** (start in parallel as their deps land).

- **FORGE-19** (risk #1) [P0] — *Virtual-merge-queue convergence load test* —
  harness exercising realistic path-overlap/conflict rates; target origin.md's
  staged ≈22.6 commits/s into one repo before trusting the queue as sole merge
  authority. Depends: FORGE-3.
- **FORGE-20** (risk #2) [P1] — *Verification-at-N capacity plan* — run
  `merge-deploy-gate` per-candidate in parallel so verification never becomes the
  new serialization point. Depends: FORGE-8.
- **FORGE-21** (risk #7) [P1] — *Forge backup/restore + DR runbook* — owned
  backup/audit story with the GitHub mirror as cold-storage copy. Depends:
  FORGE-9.
- **FORGE-22** (risk #6) [P2] — *Governance-parity checklist* — track owned-layer
  equivalents of CODEOWNERS, branch protection, audit log, signed commits,
  fine-grained tokens (origin.md §6/§8). Depends: FORGE-0.

**P0-now set (file + start immediately):** FORGE-0 (epic), then FORGE-1 →
FORGE-2/FORGE-3 in parallel (M1 kills the dispatch/queue failure class that
caused the 119-PR night), then FORGE-19 once FORGE-3 lands. The next P0 wave is
M3's FORGE-8 → FORGE-9 (owned merge authority + mirror), which is what actually
removes GitHub from the critical path. FORGE-5 (M2 Artifacts) starts in parallel
with M1 since it only depends on FORGE-1, with FORGE-7 as its de-risking sibling.
For the live stand-up queue after the completed first wave, execute #6768,
#6769, #6770, and #6771 in that order unless a blocker forces the documented
parallelism.

**Critical path:** FORGE-0 → FORGE-1 → FORGE-5 → FORGE-8 → FORGE-9 (everything
else hangs off these five). Everything in M4/M5 and the risk-driven lane can
proceed concurrently once its listed dependency lands.

---

## 7. Honest risks & unknowns (the hard parts we are owning)

We are choosing to own these. They are real.

1. **We now own merge convergence and "which patch is current."** GitHub's
   battle-tested merge/branch-protection semantics are replaced by our virtual
   merge queue + D1 truth. Mitigation: the queue is a pure, deterministic,
   tested planner (path-conflict detection, projected HEAD) and D1 is the single
   writer — convergence is resolved by the authority, not by hoping relay events
   agree. **Unknown:** behavior under high path-overlap churn at large N; needs
   load testing against realistic conflict rates (FORGE-19, targeting origin.md's
   staged throughput figures). origin.md sharpens this: its single biggest
   unresolved question for any agent-native forge is **"a clean merge is not
   necessarily a correct merge"** — an AI-resolved conflict that applies cleanly
   can still be semantically wrong. Our answer is that auto-resolution must be
   *audited and explainable*, not just clean: the trajectory-proof tags (§3.1)
   and D1 promotion evidence make every auto-resolved merge replayable and
   attributable, and the `merge-deploy-gate` (verify after resolve) is what
   distinguishes "applies" from "is correct."
2. **We now own CI/verification authority.** GitHub Actions / required checks no
   longer gate merges. The `merge-deploy-gate` signature (`check:deploy` + smoke)
   must be as trustworthy as GitHub's required-checks. **Unknown:** how we run
   verification at arbitrary N without it becoming the new serialization point
   (the queue serializes promotion, but verification can parallelize per
   candidate — needs capacity planning; FORGE-20). origin.md flags the dual
   hazard explicitly: an agent-native forge must **avoid infinite agent retry
   loops on flaky tests** — an auto-fix-then-rerun loop on a non-deterministic
   check burns capacity and can land noise. The `merge-deploy-gate` must treat
   flaky/non-deterministic checks as a distinct, bounded-retry, human-tagged
   class, not as an endlessly re-dispatchable `ci_fail` blocker.
3. **Cloudflare Artifacts is in beta.** Access, limits, and API stability are
   not guaranteed. Mitigation: M2 ships behind a fallback (owned per-task git /
   R2-backed bare repos) so the architecture is not hostage to beta access. We
   also do not bet multi-tenant production on it before a hardened
   short-lived-token isolation flow exists.
4. **Relay convergence + spam/abuse.** A relay has no inherent "current state"
   consensus; duplicate-work reappears as duplicate 1618 events unless keyed on
   the work-unit (the same identity discipline that fixed the PR storm).
   Relay-level abuse/replay (cf. the `#6643`-class threat model for
   `pylon_join`) must be designed in, not bolted on. D1-as-authority bounds the
   blast radius (the relay is transport, not truth).
5. **Security surface expands.** We are operating a git store, a relay, and
   merge authority — more attack surface than "PRs on GitHub." The autonomous-ops
   Blueprint signatures + the adversarial harness (`#6643`) are the controls;
   they must keep pace with the new surfaces.
6. **External-developer expectations.** Humans and outside contributors expect
   GitHub PRs. The mirror keeps a reviewable GitHub surface, but the canonical
   workflow being on `forge.openagents.com` is a UX/discoverability cost we accept
   and must communicate. Customer-repo work (QA-ing a customer's GitHub repo,
   opening a PR they review) remains irreducibly GitHub — that's the product,
   not a dependency we remove.
7. **Operational maturity.** GitHub gives us decades of forge reliability for
   free. `forge.openagents.com` needs its own backup/restore, audit, and
   disaster-recovery story — the GitHub mirror is part of that (cold-storage
   copy), but it is not a substitute for owning the operational discipline.
8. **Governance & portability parity (origin.md §8).** A self-owned forge inherits
   GitHub's *governance* expectations: CODEOWNERS, branch/tag protections, audit
   logs, signed commits, fine-grained tokens, SSO/RBAC for any human or tenant
   surface. origin.md lists these as Origin's own biggest open questions; they are
   ours too. Mitigation: the Blueprint Signatures cover the *agent* governance
   path, and FORGE-22 tracks the human/tenant-facing parity checklist — but until
   that list is green, the owned layer is not a governance-complete GitHub
   replacement, only an agent-coordination authority with GitHub as the mirror.
   **Portability** is the inverse promise: because GitHub stays a full downstream
   mirror (§4), teams are never locked in — complete repo history is always
   exportable, which is precisely the migration/portability guarantee origin.md
   notes Origin has not yet published.

None of these is a reason to delay. They are the scope. The decision in §0 is
that owning them at a latency we control is strictly better than renting a forge
whose ceiling we have already hit at the scale we are committing to.

---

## 8. For Artanis review

Artanis — this supersedes the rev-1 "stay on GitHub" recommendation you
reviewed, and it now has a name and a home: **`forge.openagents.com`**. The owner
has committed: **we fan out to arbitrary N now, GitHub becomes a downstream
mirror, we own the coordination layer.** Your prior feedback is already folded
in and is now load-bearing rather than hypothetical:

- Your **Virtual Merge Queue** call is the merge authority of the new
  architecture (M3), not a GitHub-native fallback. It's landed
  (`apps/pylon/src/virtual-merge-queue.ts`); M3 promotes it from "branch from
  virtual HEAD" to "decide the merge locally, mirror to GitHub after."
- Your **broadened triggers** (API-rate-limit exhaustion, abuse-flagging,
  state-sync latency) are no longer "triggers to watch" — they are the *stated
  rationale* in §0 for why we're leaving GitHub's critical path now.
- Your **"Artifacts as P1 / GitHub demoted to cold-storage mirror"** call for
  multi-tenancy is now the baseline architecture (M2/M3), not a multi-tenant-only
  escalation.

Requests for your review:

1. **M-ordering sanity.** Is M1 (D1 truth) → M2 (Artifacts worktrees) → M3
   (owned merge + mirror) the right leverage order, or should the relay (M4)
   come earlier to kill the latency floor sooner for tight agent loops?
2. **Verification-at-N (risk #2).** What is your design for running the
   `merge-deploy-gate` verification at arbitrary N without it becoming the new
   serialization point? This is the open question most likely to bite.
3. **Convergence load profile (risk #1).** What path-overlap / conflict rate
   should we load-test the virtual merge queue against before trusting it as the
   sole merge authority?
4. **Fallback adequacy (risk #3).** Is an R2-backed bare-repo fallback
   sufficient to ship M2 if Artifacts beta access stalls, or do we hold M2 for
   Artifacts?
5. **Competitive-frame & differentiation (origin.md).** origin.md shows Cursor's
   Origin converging on the same owned-forge thesis. Do you agree our two
   differentiators — GitHub-as-mirror (no migration ask) and the native relay
   economic loop — are the right wedges, and is the §6 roadmap's P0 set
   (FORGE-1/2/3 then FORGE-8/9) the fastest path to a defensible version of them?

The owner will present this to you directly. No `artanis.sh` invocation is
needed from the build side.

---

## Related issues & docs

- `docs/forge/origin.md` — the owner's architecture origin/vision doc: a sourced
  competitive analysis of Cursor's "Origin" forge, incorporated here (§1.3, §2.2,
  §7, §6 roadmap) as the architecture reference for this build plan.
- `workspace:docs/afteraction/2026-06-28-overnight-fleet-after-action.md` — the incident.
- `workspace:ARTANIS_PR_REPORT.md` — Artanis's accountability report.
- Commits `a542e056` (per-issue PR dedup) and `e66876276f` (issue-close-safe
  dispatch lockout) — the coordination signatures (M0).
- `apps/pylon/src/virtual-merge-queue.ts`,
  `apps/pylon/src/blueprint-gates/virtual-merge-queue.ts`,
  `apps/pylon/scripts/codex-supervisor/virtual-merge-queue.sh` — the merge guard
  / queue (M0 → merge authority in M3).
- `apps/pylon/scripts/codex-supervisor/priority-dispatch.sh` — #6711 saturation
  engine.
- `apps/pylon/src/coordinator/coordinator-runtime.ts` (+ `planner.ts`,
  `coordinator-state.ts`) — the fan-out coordinator state machine.
- `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md` —
  the five Blueprint Signatures (gate authority).
- `projects/repos/nips/34.md` + `nostr-effect/src/core/Nip34.ts` — NIP-34 spec +
  our codec; `apps/nostr-relay/` — owned relay surface.
- `docs/2026-04-01-gitafter-status-and-implementation-audit.md` +
  `backroom/.../crates/nostr/GIT_AFTER.md` +
  `backroom/.../docs/plans/research/gitafter-bonus.md` — prior agent-native git
  history (now promoted from "bonus" to committed strategy at
  `forge.openagents.com`).
- `docs/research/2026-06-24-cloudflare-artifacts-for-autonomous-qa.md` +
  `apps/openagents.com/docs/omni/2026-06-06-cloudflare-artifacts-git-agent-audit.md`
  — Cloudflare Artifacts fit.
- `#6637` (+ `#6638`/`#6639`/`#6640`) — rate-limit / account observability epic.
- `#6643` — adversarial security harness gating the Artanis interface (`#6486`).
</content>
