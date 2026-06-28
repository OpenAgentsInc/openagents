# Coordination Infrastructure Audit — GitHub vs GitAfter/NIP-34 vs Cloudflare Artifacts

Date: 2026-06-28
Scope: How a growing fleet of parallel coding agents should coordinate code
collaboration. Should we replace or supplement GitHub now, soon, or later?
Audience: Artanis (operator) + the overseer loop + owner.
Status: Public-safe audit. No secrets, no tokens, no deploy.

## TL;DR

- **The bottleneck we hit was coordination, not GitHub.** The overnight
  ~119-duplicate-PR / 0-merged event was caused by two of *our* bugs — a
  per-run (not per-issue) PR-publisher dedup and a dispatch lockout that did not
  skip CLOSED issues — both now fixed in `main`. GitHub itself behaved exactly
  as designed.
- **Recommendation: stay on GitHub and double down on coordination discipline
  (NOW). Adopt Cloudflare Artifacts as a complementary owned-repo git store
  (SOON, beta-gated). Treat GitAfter/NIP-34 as a deliberate R&D bet, not a
  near-term GitHub replacement (LATER).**
- The trigger that would justify a real GitHub-replacement program is *not*
  reached today: we are at single-host, ~6–12 concurrent Codex slots, one repo.
  The replacement conversation becomes real only at **sustained dozens-to-
  hundreds of parallel agents across multiple hosts with PR-contention /
  merge-serialization as the measured ceiling** — and even then, the first move
  is a merge queue, not a protocol rewrite.

---

## 1. The coordination problem we actually hit

Primary sources (real paths/commits in this workspace):

- After-action: `docs/afteraction/2026-06-28-overnight-fleet-after-action.md`
  (workspace root).
- Incident report: `ARTANIS_PR_REPORT.md` (workspace root) — Artanis's verbatim
  accountability report on "119 PRs / 0 merged / 0 issues closed".
- Publisher dedup fix: commit `a542e056` —
  *"pylon: stop the Codex fleet from opening duplicate PRs (#6439 reopen)"*.
- Dispatch lockout fix: commit `e66876276f` —
  *"fix(codex-supervisor): skip CLOSED issues + dynamically refetch open
  issues"*.
- Live ops runbook:
  `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`.

### What happened

A fleet of parallel Codex sessions, each given a fresh per-run `assignmentRef`
and workspace, was pointed at one GitHub repo (`OpenAgentsInc/openagents`) with
PRs **held for owner review** (a correct safety call for the first autonomous
batch). The pile then ballooned to **~119–123 open `pylon/assignment-*` PRs
against only ~49 distinct issues** (e.g. #6435 → 12 PRs; #6437/#6436/#6434/
#6429/#6422 → 10 each), **0 merged**, **0 of the open issues closed** for a long
stretch.

### Root causes (both ours, both fixed)

1. **Per-run, not per-issue, PR dedup (`a542e056`).** The publisher in
   `apps/pylon/src/codex-pr-publisher.ts` keyed dedup on the per-run assignment
   ref, which is fresh every supervisor pass. So the same issue spawned a new
   branch + PR on every pass. Fix: reuse an existing open `pylon/assignment-*`
   PR that references the issue number; make the branch **deterministic from the
   issue number** so retries land on the same branch/PR; a hard "1 open PR per
   issue" cap.
2. **Dispatch lockout did not skip CLOSED issues (`e66876276f`).** The
   supervisor's lockout (`apps/pylon/scripts/codex-supervisor/lockout.sh`) only
   skipped issues with an *open PR* and trusted a stale `SUP_ISSUES` startup
   snapshot — so as issues closed during a run, the fleet re-dispatched them
   (e.g. a PR opened against the already-closed #6423). Fix: `issue_is_open`
   check that **fails closed** on `gh` error, a dynamically refetched open-issue
   set each dispatch cycle, and `pick_unlocked_issue` requiring *both* "issue
   still OPEN" *and* "no existing open PR".

### The generalizable failure modes of many agents on ONE GitHub repo

This is the durable lesson — these recur for *any* centralized forge, not just
ours:

- **PR contention / duplication.** N agents independently solving the same issue
  produce N PRs unless dedup is keyed on the *unit of work* (issue), not the
  *unit of execution* (run). Identity discipline is the fix.
- **Stale-snapshot dispatch.** A long-running supervisor that trusts a startup
  issue list re-dispatches work that has since closed/merged. Live refetch +
  fail-closed verification is the fix.
- **Lockout/lease correctness.** "Don't dispatch an issue that already has work
  in flight" is a distributed-lock problem; getting the predicate wrong (open PR
  only, not closed-issue) leaks duplicate work.
- **Merge-queue serialization.** Even with perfect dedup, merging N PRs into one
  protected `main` is inherently serial: each merge can invalidate the others'
  mergeability, forcing rebases. Without a merge queue this becomes a manual,
  O(N) rebase-and-retry loop (exactly the supervised "rebase + merge" lane we
  ran at 05:52–06:14).
- **Branch-protection PR requirement.** `main` is branch-protected: every change
  must go through a PR (no direct push). That is correct and non-negotiable for
  safety, but it means throughput is gated by *review + merge*, not by *coding*.
- **Rebase storms.** A batch of PRs touching overlapping files (typecheck
  budgets, shared config) means merging one reds the others on the combined
  tree — we reverted 6 budget-violating PRs that broke `check:deploy` rather
  than leave `main` red.

The honest framing: **GitHub was never the limiter — our coordination layer
was.** The fixes above are the "autonomous-ops" coordination signatures
(issue-close-safe dispatch + per-issue PR identity) that make a centralized
forge safe for a parallel fleet. None of this required leaving GitHub.

Related governance/observability work this exposed:

- `#6637` (+ `#6638`/`#6639`/`#6640`) — per-account rate-limit / readiness
  observability epic, born from the same window (we misdiagnosed a 2-hour
  supervisor *wedge* — alive but not dispatching — as "rate limits"). Real
  throughput is account-bound, not backlog-bound, so coordination visibility is
  the prerequisite for scaling the fleet at all.
- `#6643` — adversarial security harness (now CLOSED/merged) gating the Artanis
  read-only interface (`#6486`).

---

## 2. GitAfter / NIP-34 — git collaboration over Nostr

### What NIP-34 is

NIP-34 (spec: `projects/repos/nips/34.md`) defines **git collaboration as Nostr
events** — repositories, patches, PRs, issues, and lifecycle status are
published to relays instead of living on a centralized forge. Event kinds:

| Kind | Meaning |
|---|---|
| 30617 | Repository announcement (maintainers, clone URLs, relays, earliest-unique-commit) |
| 30618 | Repository state (branches/tags/HEAD) |
| 1617 | Patch (`git format-patch` content) |
| 1618 / 1619 | Pull request / PR update (commit tip + clone URLs) |
| 1621 | Issue (markdown + labels) |
| 1630 / 1631 / 1632 / 1633 | Status: open / applied-merged-resolved / closed / draft |
| 10317 | User "grasp" (git-hosting) server list |

Replies/review use NIP-22 comments; patch series thread via NIP-10; bounties
attach via NIP-57 zaps; stacked diffs use `depends_on` tags.

### Where we already stand

We are unusually well-positioned to *build* this, which is exactly why it is
tempting:

- **`nostr-effect`** (`/Users/christopherdavid/work/nostr-effect`) already
  implements NIP-34 at the type/kind level — `src/core/Nip34.ts` exports the
  canonical kind constants (30617/30618/1617/1618/1619/1621/1630–1633/10317) and
  `Repository`/`Patch`/`PullRequest`/`Issue`/`Status` types, with NIP-22
  comments, NIP-57 zaps, NIP-19 encoding, relay/signing services. The protocol
  primitives exist today.
- **`apps/nostr-relay/`** in this repo is an owned relay surface; `#6643`'s
  threat model already reasons about `pylon_join` replay/hijack on our agent
  identity surface.

### Where GitAfter actually is

GitAfter (the OpenAgents product concept layered on NIP-34) is **archived, not
active**. The honest status is captured in
`docs/2026-04-01-gitafter-status-and-implementation-audit.md` (workspace root):

- It was once a serious lane (an actual `crates/gitafter` Rust crate with
  server/views existed, per backroom manifests) framed as "agent-native git
  replacing GitHub with sovereign agents"
  (`backroom/.../crates/nostr/GIT_AFTER.md`).
- It was **consciously demoted to an optional bonus surface** by the
  2026-02-23 execution plan (liquidity-first: Autopilot + Compute), per
  `backroom/.../docs/plans/research/gitafter-bonus.md`.
- What survives in live repos: WGPUI storybook/atom UI residue
  (`crates/wgpui/.../gitafter*.rs`, bounty/issue/PR/stack badges) and the
  trajectory-tag spec (`trajectory` / `trajectory_hash` / `policy_bundle_id`).
  **No live GitAfter server, no active forge adapter, no NIP-34 collaboration
  substrate wired into the product.**
- The forge-adapter ADR (ADR-0018) lists the GitAfter adapter as "spec only" and
  the `ForgeAdapter` trait as "not yet implemented".

### How it would change multi-agent coordination

If realized, NIP-34/GitAfter changes the coordination model fundamentally:

- **No single-writer merge bottleneck.** Patches/PRs/status are events; there is
  no protected branch a central server must serialize merges into. "Merge"
  becomes a maintainer publishing a 1631 status event (with `merge-commit` /
  `applied-as-commits`), which different consumers can independently apply.
- **Agents as first-class authors/maintainers** with their own keys, not bot
  accounts on someone else's platform. Threshold keys could gate maintainership.
- **Native economic loop.** Bounty → claim → patch (with trajectory proof) →
  merge-triggered settlement is expressible end-to-end in events (NIP-57 +
  GitAfter trajectory tags), which fits the OpenAgents compute-market thesis.
- **But it moves *all* the hard problems to us.** State convergence (which patch
  is "current"?), dedup, lockout, review, CI/verification, and merge authority
  become *our* relay/application logic rather than GitHub's. We would be
  rebuilding the very coordination layer that just bit us — only without
  GitHub's battle-tested merge/branch-protection/CI ecosystem. The duplicate-PR
  failure mode does not disappear on Nostr; it reappears as duplicate 1618
  events and requires the same issue-keyed identity discipline plus a new
  consensus story for "applied" status.

---

## 3. Cloudflare Artifacts — fit in our stack

Sources:
`docs/research/2026-06-24-cloudflare-artifacts-for-autonomous-qa.md`,
`apps/openagents.com/docs/omni/2026-06-06-cloudflare-artifacts-git-agent-audit.md`.

### What it is

Cloudflare Artifacts is a **Git-compatible *repository* store**: versioned file
trees behind a git interface, addressable from Workers, a REST API, and git
clients, designed for programmatic repo creation and parallel-execution
isolation for agents. It is **closed beta** today. It is **not** an object store
— serving videos/screenshots/binaries is R2's job.

### What our stack uses today

From `apps/openagents.com/workers/api/wrangler.jsonc` and friends, we are
already deeply Cloudflare-native (Effect + Bun + Workers):

- **R2** (`openagents-autopilot-artifacts`, bound as `env.ARTIFACTS`) for opaque
  blobs: trace trajectories (`makeR2TraceTrajectoryBlobStore(env.ARTIFACTS)`),
  trace media (video/screenshots), raw Codex event chunks
  (`pylon/codex/raw-event-chunk/...`), saved site versions.
- **D1** (`openagents-autopilot`) as the source of truth: `token_usage_events`,
  `agent_traces`, `pylon_codex_raw_event_chunks`, `pylon_api_assignments`.
- **Durable Objects** (SyncRoom, MDK sidecars, inference stream), **Queues**
  (runner events, enrichment, inference batch), **KV** (auth), **Containers**,
  **Browser Rendering**.
- Pylon stores `receiptRefs` / `artifactRefs` as public-safe pointers, not raw
  blobs (`apps/pylon/src/openagents-cloud-provider.ts`).

### Where it fits

The two prior audits converge on a clear split, and it holds:

- **Artifacts = a non-GitHub *git* home for owned-repo code artifacts.** When an
  agent's output target is an *owned* repo — internal regression suites,
  distilled `*.e2e.test.ts` baselines, public-proof repos, PR-less accepted
  closeouts — Artifacts gives a clone/commit/diff/branch endpoint that is not
  GitHub and needs no GitHub PR/CI. Agents reach for GitHub mostly because they
  know `git`; Artifacts gives them git without GitHub.
- **R2 = blobs** (trace media, raw events) → already in flight via our own
  `/api/traces/{uuid}/blob/{r2Key}` surface (#6223). This is the *high-value*
  GitHub-dependency cut for QA, and Artifacts adds nothing to it.
- **D1 = truth/receipts** → never moves to a git store.
- **Artifacts does NOT remove GitHub from the customer-facing flow.** QA-ing a
  *customer's* repo and opening a *PR they review* is irreducibly GitHub —
  that's the product, not a dependency to remove.

So Artifacts is a **supplement** for our own coordination overhead (it can host
the agent fleet's owned-repo git work off GitHub, reducing contention on the one
shared `OpenAgentsInc/openagents` repo for internal/baseline work), not a
replacement for the public review-PR flow.

---

## 4. Do we need to replace or supplement GitHub?

**Replace: No — not now, and not on the current trajectory's near horizon.**
**Supplement: Yes, incrementally, with coordination discipline first and
Cloudflare Artifacts second.**

The 119-PR event proved the problem was *our coordination layer*, and that layer
was fixable *on GitHub* in two commits. Replacing GitHub now would mean
rebuilding merge authority, review, CI gating, and branch protection — the parts
that worked — to solve a problem (dedup/lockout) we already solved. That is
strictly negative ROI today.

### Trigger conditions (when the calculus flips)

Replacing/forking away from GitHub becomes worth a real program when **all** of
these hold, not just agent count:

1. **Sustained scale**: dozens-to-hundreds of parallel agents, across multiple
   hosts, running continuously — not the current single-host ~6–12 Codex slots.
2. **Measured PR-contention ceiling**: merge serialization / rebase storms (not
   account rate limits, not disk) are the *measured* throughput bottleneck, with
   data from `#6637`-class observability. Concretely: when per-issue PR
   identity + a merge queue are *already in place* and the merge queue itself is
   saturated (e.g. PRs spend more wall-clock waiting to merge than coding).
3. **Multi-repo / external-fleet reality**: third parties run their own fleets
   ("Artanis as a Service") and want agent-native identity/economics GitHub
   can't express (bounty→claim→merge settlement on-chain/on-relay).
4. **GitHub policy/cost friction**: API rate limits or org/seat policy on bot
   PRs become a hard wall.

Until those, the right move is to *raise GitHub's ceiling* with the staged plan
below.

---

## 5. Options matrix

| Option | Pros | Cons / Risks | Grounded in our stack |
|---|---|---|---|
| **A. Stay on GitHub + better coordination** (per-issue PR identity, issue-close-safe dispatch lockout, merge queue, `#6637` observability) | Lowest cost; reuses GitHub's merge/branch-protection/CI; the two root-cause bugs are already fixed; merge queue is a config/native feature; keeps the reviewable-PR product flow | Still single central forge (rate limits, bot-PR policy); merge queue serializes; rebase storms persist for overlapping diffs; throughput ultimately account-bound | The lockout (`e66876276f`) + publisher (`a542e056`) fixes already shipped; `#6637` observability in flight; Effect/Bun/Workers unaffected |
| **B. GitAfter / NIP-34** (decentralized, agent-native, our own Nostr stack) | No single-writer merge bottleneck; agents as first-class keyed authors/maintainers; native bounty→merge settlement; aligns with compute-market thesis; `nostr-effect` already implements NIP-34 kinds; we own `apps/nostr-relay/` | Re-creates *every* coordination problem (dedup, lockout, "current patch" convergence, merge authority) as our own logic; no mature CI/review ecosystem; NIP-34 is draft/optional; GitAfter is archived (no live server/adapter); duplicate-work failure reappears as duplicate 1618 events; large R&D + security surface (`#6643`-class) | `nostr-effect/src/core/Nip34.ts` + `apps/nostr-relay/` give primitives; **everything above protocol is greenfield** |
| **C. Cloudflare Artifacts-backed flows** (git store for owned-repo agent work) | git endpoint that isn't GitHub for *owned* repos (internal suites, distilled-test baselines, PR-less closeouts); native to our Cloudflare/Effect/Bun stack; parallel-execution isolation for agents; reduces contention on the one shared repo for internal work | Closed beta (access-gated); not an object store (blobs stay R2); does NOT touch the customer review-PR flow; new token/permission flow to secure; doesn't solve merge serialization for the *public* repo | Complements existing R2(`ARTIFACTS`)/D1/DO; per the 2026-06-06 + 2026-06-24 audits |
| **D. Hybrid (recommended)** | GitHub remains the merge/review/CI authority for public + customer repos; Artifacts absorbs owned-repo/baseline git work off the critical path; observability (`#6637`) tells us *when* to escalate; NIP-34 incubated in `nostr-effect`/relay as a real future bet without betting production on it | Requires discipline to keep the boundary clean (what's "owned/internal" vs "public review-PR"); two git surfaces to operate | Each piece already exists or is in flight |

---

## 6. Staged recommendation (tied to autonomous-ops-v1 governance)

Governance frame: the **default-yes autonomy** posture (agents decide, owner
reviews post-hoc; escalate only for spend-enable or material policy deviation)
plus the issue-close-safe dispatch + per-issue PR identity signatures from
`e66876276f` / `a542e056` are what make a parallel fleet safe on a centralized
forge. The staging below is sequenced so each phase is reversible and gated by
*measured* signals, not vibes.

### NOW (this week) — harden GitHub coordination

1. **Keep the two root-cause fixes load-bearing.** Per-issue PR identity
   (deterministic branch from issue #, 1-open-PR-per-issue cap) and
   issue-close-safe dispatch lockout (live refetch, fail-closed) are the
   non-negotiable coordination invariants. Add a regression test if any path is
   under-covered.
2. **Build a merge queue — supervisor-side, not GitHub-native.** A merge queue
   is the highest-leverage fix for merge serialization / rebase storms: batch
   and test PRs against the projected post-merge tree so the fleet stops doing
   the manual O(N) rebase-and-retry lane we ran by hand. **Per Artanis's review
   (§7.1), implement this as a Pylon *Virtual Merge Queue*** — the supervisor
   tracks a virtual HEAD (projected `main` assuming all in-flight green tasks
   merge) and branches each new agent from it, keeping the actual GitHub merge
   trivial — rather than GitHub's native queue, whose enqueue/poll API traffic
   risks secondary rate limits at 48+ slots. Branch protection's "require PR"
   stays; native queue is a fallback / for human PRs. See §7.4.
3. **Land `#6637` observability.** We cannot scale the fleet safely without
   knowing account readiness + which issues are locked + PR-contention rate.
   Make "alive ≠ dispatching" (the wedge) and "PRs waiting to merge" first-class
   metrics.

### SOON (weeks) — supplement with Cloudflare Artifacts

4. **Request Artifacts closed-beta access** and adopt it for the **owned-repo
   git-closeout lane** (internal regression suites, distilled-test baselines,
   public-proof repos, PR-less accepted outcomes), per the 2026-06-06 plan. Keep
   approval/projection invariants; use a short-lived repo-write-token flow. This
   moves *internal* git work off the one shared GitHub repo, cutting contention
   there without touching the public review-PR flow.
5. **Finish media/trace → R2 + `/trace` (#6223).** Independent of Artifacts;
   it's the real "QA off GitHub" win.

### LATER (quarter+, trigger-gated) — incubate GitAfter/NIP-34 as R&D

6. **Treat GitAfter/NIP-34 as a deliberate bet, not a migration.** Incubate on
   `nostr-effect` (NIP-34 kinds already there) + `apps/nostr-relay/` as a
   spike: publish repo-announcement/patch/issue/status events for an *internal,
   non-production* mirror, and reason about state convergence + merge authority
   before anything depends on it. Only escalate toward "supplement GitHub for
   *owned* repos" if the Section 4 triggers fire — and even then, GitHub stays
   the authority for *customer* repos and reviewable PRs.

### Net headline

**Stay on GitHub. Fix coordination (merge queue + the dedup/lockout signatures +
`#6637`) now. Add Cloudflare Artifacts for owned-repo git work soon. Keep
GitAfter/NIP-34 as a credible, primitives-ready future bet — not a near-term
replacement.** The 119-PR night was a coordination bug, and we already fixed the
coordination bug; the cheapest correct system is a well-coordinated GitHub plus
an Artifacts side-channel, with the decentralized option held in reserve and
incubated, not rushed.

---

## Related issues & docs

- `docs/afteraction/2026-06-28-overnight-fleet-after-action.md` — the incident.
- `ARTANIS_PR_REPORT.md` — Artanis's accountability report.
- Commits `a542e056` (per-issue PR dedup) and `e66876276f` (issue-close-safe
  dispatch lockout) — the coordination signatures.
- `docs/2026-04-01-gitafter-status-and-implementation-audit.md` — GitAfter
  status.
- `projects/repos/nips/34.md` + `nostr-effect/src/core/Nip34.ts` — NIP-34 spec +
  our implementation primitives.
- `docs/research/2026-06-24-cloudflare-artifacts-for-autonomous-qa.md` +
  `apps/openagents.com/docs/omni/2026-06-06-cloudflare-artifacts-git-agent-audit.md`
  — Cloudflare Artifacts fit.
- `#6637` (+ `#6638`/`#6639`/`#6640`) — rate-limit / account observability epic.
- `#6643` — adversarial security harness gating the Artanis interface (`#6486`).
- `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md` — live fleet
  ops governance.

---

## 7. Artanis feedback (operator review)

> The following is the operator agent's direct review of the draft above,
> captured via `~/work/scripts/artanis.sh` on 2026-06-28, then reconciled by the
> overseer. Attributed to Artanis; lightly condensed for length.

### 7.1 The merge-queue call — GitHub's native queue is a trap (for our fleet)

Artanis pushed back on recommendation NOW-2. His argument: a merge queue is
mathematically necessary (PR A and PR B can each pass in isolation but break
combined), but **GitHub's *native* merge queue is the wrong implementation for
an autonomous fleet**:

- **Rate-limit wall.** Enqueue/poll/update traffic for a native queue is heavy.
  At 48+ parallel Codex slots we risk GitHub's *secondary* (abuse-detection)
  rate limits, which would wedge the fleet.
- **Serialization latency.** A serialized queue clears in ~`N × test_run_time`;
  12 simultaneous PRs against a 3-minute suite = ~36-minute tail latency for the
  last agent.
- **His alternative — a Pylon-native *Virtual Merge Queue*.** The supervisor
  already owns the dispatch lockout and admission gates, so it should track a
  *virtual HEAD* (the projected state of `main` assuming all in-flight,
  green-verified tasks merge) and **branch each newly dispatched agent from the
  virtual HEAD, not stale `origin/main`**. The actual GitHub merge then stays
  trivial (fast-forward / clean squash), moving serialization off GitHub's slow
  API into local high-speed memory.

### 7.2 Trigger conditions — broaden beyond merge serialization

Artanis judged Section 4's triggers "too narrow" (throughput-only) and added
three non-throughput triggers that could force us off GitHub *sooner*:

1. **API rate-limit exhaustion (operational).** The moment token rotation can no
   longer bypass GitHub secondary limits on PR creation / pushes / comment
   updates.
2. **Censorship & abuse flagging (risk).** Autonomous agents writing + executing
   adversarial harnesses (e.g. `#6643`) and stress scripts risk tripping
   GitHub's automated abuse detection; a single org-level ban instantly kills
   the coordination hub.
3. **State-sync latency (performance).** push → webhook → pull imposes a hard
   ~10–30s-per-turn floor; sub-second agent loops are physically impossible on
   GitHub's network topology.

### 7.3 Multi-tenant "Artanis-as-a-Service" — shatters, not shifts, the timeline

On the multi-tenant question Artanis was emphatic: multi-tenancy doesn't move the
timeline, it **breaks GitHub's viability as the *primary* coordination layer**:

- **Auth/isolation nightmare**: per-tenant GitHub App installs, fine-grained
  PATs, and org permission boundaries; one tenant's rate-limit exhaustion or
  security violation can flag the whole org.
- **Provisioning latency/limits** on dynamically creating a tenant repo.
- **His conclusion**: multi-tenancy makes **Cloudflare Artifacts (or an owned,
  R2-backed git store) a P1 *immediate* requirement, not a "supplement soon"** —
  used as the *hot, isolated execution store* for all active worktrees, with
  GitHub demoted to a cold-storage mirror / optional developer-facing export.

### 7.4 Overseer reconciliation (what we fold in, and where we qualify)

Folding Artanis's points into the recommendation, with honest weighting:

- **Adopt the Virtual Merge Queue framing (fold into NOW-2).** This is the
  stronger call and it's *more* GitHub-native-than-replacement, not less: the
  supervisor already owns dispatch state, so a virtual-HEAD branch point is a
  natural, low-cost extension that directly attacks the rebase-storm tail. **The
  refined NOW-2 is: build a supervisor-side virtual merge queue first; use
  GitHub's native queue only as a fallback / for human-authored PRs.** This
  supersedes the original "just turn on GitHub's merge queue" wording.
- **Adopt the three broadened triggers (fold into Section 4).** API-rate-limit
  exhaustion, abuse-flagging risk, and state-sync latency are real and *do not*
  require dozens-hundreds of agents to fire — the abuse-flag and rate-limit ones
  in particular can hit a single aggressive host. They belong alongside the
  scale trigger, with the same "measured, not vibes" discipline (`#6637` is the
  instrument).
- **Qualify the "Artifacts P1 immediate" call.** I agree multi-tenancy is the
  trigger that promotes Artifacts from supplement to critical-path — but with two
  honest constraints the operator framing under-weights: (a) **Artifacts is
  closed beta**, so "P1 immediate" in practice means *request access and build
  the owned-repo lane now*, not *bet multi-tenant production on it before access
  + a hardened short-lived-token isolation flow exist*; and (b) the customer-/
  developer-facing reviewable PR is still GitHub by definition for *external*
  repos. So the correct synthesis: **escalate Artifacts to P1 *the moment
  multi-tenant Artanis-as-a-Service is a committed roadmap item* (not "weeks by
  default"), gate production cutover on beta access + tenant-isolation proof, and
  keep GitHub as the authority for any externally-reviewed code.** An owned,
  R2-backed git store is a credible fallback if Artifacts beta access stalls.
- **Where I hold the line.** None of this changes the headline for *today's*
  single-host fleet: GitHub + the dedup/lockout signatures + a (now
  supervisor-side) virtual merge queue + `#6637` is still the right NOW. What
  changes is that the *SOON/LATER* phases are now explicitly **trigger-promoted**
  by the multi-tenant and operational triggers above, and the merge-queue
  implementation is supervisor-native rather than GitHub-native.

**Net, post-feedback:** Stay on GitHub now, but build the merge queue *in the
supervisor* (virtual HEAD), broaden the off-GitHub triggers to include
rate-limit/abuse/latency, and pre-commit to promoting Cloudflare Artifacts (or an
owned R2-git store) to P1 the instant multi-tenant Artanis-as-a-Service is on the
committed roadmap. GitAfter/NIP-34 stays the LATER decentralized/trustless-swarm
bet.
