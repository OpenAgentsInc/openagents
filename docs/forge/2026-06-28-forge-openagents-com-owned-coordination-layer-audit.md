# forge.openagents.com — Owned Coordination Layer + GitHub as Mirror (committed build plan)

Date: 2026-06-28 (rev 2 — decision committed)
Scope: How an arbitrarily large fleet of parallel coding agents coordinates
code collaboration. This revision supersedes the prior "stay on GitHub, maybe
later" audit: the owner has **decided**.
Audience: Artanis (operator) + the overseer loop + owner.
Status: Public-safe build plan. No secrets, no tokens, no deploy.

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

---

## 7. Honest risks & unknowns (the hard parts we are owning)

We are choosing to own these. They are real.

1. **We now own merge convergence and "which patch is current."** GitHub's
   battle-tested merge/branch-protection semantics are replaced by our virtual
   merge queue + D1 truth. Mitigation: the queue is a pure, deterministic,
   tested planner (path-conflict detection, projected HEAD) and D1 is the single
   writer — convergence is resolved by the authority, not by hoping relay events
   agree. **Unknown:** behavior under high path-overlap churn at large N; needs
   load testing against realistic conflict rates.
2. **We now own CI/verification authority.** GitHub Actions / required checks no
   longer gate merges. The `merge-deploy-gate` signature (`check:deploy` + smoke)
   must be as trustworthy as GitHub's required-checks. **Unknown:** how we run
   verification at arbitrary N without it becoming the new serialization point
   (the queue serializes promotion, but verification can parallelize per
   candidate — needs capacity planning).
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

The owner will present this to you directly. No `artanis.sh` invocation is
needed from the build side.

---

## Related issues & docs

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
