# Programmatic Multi-Account Pylon Audit: Codex Orchestration + Account Usage

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


> Status: audit, 2026-06-13. Authored by Fable (`claude-fable-5`,
> session-verified). Owner question: how far is Pylon from (a) an external
> orchestrator (e.g. a chat session spawning subagents) programmatically
> driving MULTIPLE parallel Codex sessions, each in its own worktree, each
> on a DIFFERENT account; and (b) a Pylon CLI command reporting usage for
> one or all connected accounts (Codex subscriptions, Claude, etc.)?
> Verdict up front, updated after #4868-#4870: **(a) now has both a local
> first-class script surface and a running-Pylon control-server surface for N
> bounded sessions over worktrees × accounts. (b) still does not exist and has
> an honest API-reality boundary.**

## 1. Version truth first

- Published: `@openagentsinc/pylon@0.3.0-rc2` (`rc` dist-tag; `latest`
  stays 0.2.5). **There is no rc3 yet.** Source main is ahead of the rc2
  artifact by the post-publish fixes: the self-reported-version constants
  (rc2 artifact says rc1 — fixed, `1b451ba37`), the NIP-90 provider-loop
  60s-timeout death (`db135e787`), the stranger-probe smoke (`4eda30abc`),
  the M10 overnight harness (`5b6b5af70`), and the backlog-faucet contract
  (`aa23e362b`). **Everything below describes source main (the would-be
  rc3) unless marked rc2.**

## 2. What is DONE toward the target (with receipts)

### Programmatic session control — substantially done
- **Headless single-session driver**: `apps/pylon/scripts/dev-proof-run.ts`
  (#4847/#4860) drives the exact composer execution path
  (`runCodexComposerStream` / `runClaudeComposerStream`, no TUI coupling)
  with `--adapter codex|claude_agent --objective ... -- <verify argv>`,
  retained redaction-clean proof artifacts. `runProof` is exported with a
  parameterized `cwd` (M10 change), so a caller points each session at any
  directory — including a worktree.
- **Multi-session orchestration exists in primitive form**: the M10
  harness `apps/pylon/scripts/overnight-proof-run.ts` (#4768) runs a
  4-cell grid (Codex+Claude × composer+work-order) unattended for hours
  against a scratch repo with per-task artifacts, heartbeats, redaction
  scans, and failure tolerance. It is sequential-per-cycle, single-account
  — but it proves the spawn/drive/verify/retain loop end to end. A live
  run is executing as this audit is written
  (`run.m10.overnight.fe89b989a4adfdde61631a79`).
- **Concurrent multi-session spawner**: `apps/pylon/scripts/multi-session-run.ts`
  (#4869) reads a JSON plan, resolves each `accountRef`/direct credential
  home, materializes or accepts one workspace per session, launches
  `dev-proof-run.ts` with bounded concurrency, and writes M10-style
  per-session proof/failure artifacts, heartbeats, and a path-safe
  `multi-session-summary.json`. The focused tests cover bounded parallelism,
  failure continuation, redaction-safe summaries, and parallel shared
  bare-cache materialization under the lease path.
- **Local HTTP control plane**: `apps/pylon/src/node/control-server.ts`
  (`startControlServer`) — loopback, bearer-token, SSE events + commands
  (wallet ops, `assignments.poll`, `assignments.accept`). An external
  orchestrator can talk to a running Pylon. Updated in #4870: the command
  set now includes `session.spawn`, `session.list`, `session.events`, and
  `session.cancel`, backed by `control-sessions.ts`. Sessions reject local
  danger modes, resolve per-session account/workspace selectors, retain
  path-safe proof/failure artifacts under the Pylon home, and expose a
  redaction-scanned per-session SSE stream.
- **Work-order surface**: `pylon work submit --adapter
  codex|claude_agent|fable --commit <sha> --repo --branch --verify`
  (#4843) — programmatic, but routed through the platform work-order
  lane, not local parallel session spawning.

### Worktree support — done
- `apps/pylon/src/workspace-materializer.ts`: adapter-neutral, native
  `git worktree` with a shared bare-repo cache, assignment-scoped detached
  worktrees, TTL/retention, cleanup receipts (#4798/#4799 closed). Warm
  cross-adapter materialization ~70ms. The 2026-06-11 worktree audit's
  conclusion stands: worktree is the implementation strategy; the
  API-level contract is `git_checkout`.
- Identity/evidence contract: `pack-c-repo-worktree-identity.ts`
  (workspace/worktree/branch refs, pinned commits, cleanliness, sandbox
  profile) — the receipt shape per-session worktrees should cite.

### Multi-ACCOUNT machinery — done on the WORKER, absent on the PYLON
- **M8 account pool (live)**: `GET /api/provider-accounts/pool`
  (`provider-account-pool-routes.ts`) — per-account status, health,
  eligibility with reasons, active leases + limits, cooldown-until,
  low-credit flags, failure class, last-selected/probe timestamps,
  next-selection explain. Web panel in `/settings/connections`.
- **M9 rate-limit rotation (live-proven)**: rate-limited account → typed
  `timed_cooldown` → next account leased → work-order context preserved
  (gate record `2026-06-12-m9-live-rate-limit-rotation-gate-record.md`,
  CI smoke `autopilot-rate-limit-rotation-smoke.ts`).
- **M13 multi-provider (live)**: Gemini BYOK selectable via
  `requiredProvider` in lease policy. (Subscription-account offering for
  Anthropic/Gemini is deliberately out — ToS boundary; Codex OAuth device
  login + refresh probing exists in `provider-account-client.ts`.)
- **The pylon-side primitive exists but is unplumbed**:
  `codex-agent.ts` `detectCodexCliLogin()` reads `env.CODEX_HOME` (falls
  back `~/.codex`). **Different `CODEX_HOME` per process = different Codex
  account.** Nothing above it (config, CLI flags, control server, proof
  drivers) lets a caller SET it per session; it is inherited process
  state. Claude-side: presence detection only
  (`localClaudeSessionPresent` — `~/.claude` credentials or keychain),
  no per-session account selection of any kind.
- Pylon state (`state.ts`) and bootstrap config carry **no account
  refs** — a Pylon instance has exactly one implicit account per
  provider.

### Usage surfaces — the weakest area
- `pylon context --json` / `dev doctor --json`: adapter **presence and
  readiness** (credentialSourceRef, modes, blockers) — no usage, no
  quota, no rate-limit state.
- Worker token-usage ledger (`token-usage.ts`,
  `token-usage-ledger-routes.ts`): meters platform-side usage with an
  optional `accountRef` filter, but leaderboards aggregate by team/user;
  no per-account budget or subscription-tier surface.
- The M8 pool route DOES carry the nearest things to usage truth the
  platform owns: low-credit flags, cooldowns, lease load, failure
  classes. Pylon does not consume it.
- **Nothing anywhere queries Codex/ChatGPT subscription usage or Claude
  plan limits.**

## 3. The GAPS, ranked (what is NOT done)

1. **Per-session account selection on the Pylon side** — **implemented
   in source main for #4868 after this audit was written.** Pylon now has
   a minimal `dev.accounts` registry, `--account-ref`, `--codex-home`,
   and `--claude-config-dir` support in `dev-proof-run.ts`, per-session
   `CODEX_HOME` / `CLAUDE_CONFIG_DIR` child environments in the Codex and
   Claude composer/executor paths, and retained proofs record only hashed
   account refs. The public assignment/provider lanes remain unchanged.
   The remaining goal-(a) work starts at the concurrent spawner.
2. **A parallel multi-session spawner** — **implemented in source main for
   #4869 after this audit was written.** `multi-session-run.ts` is the
   scriptable local orchestration seam for N concurrent (workspace,
   account, objective, verify) sessions. It still shells out to the
   retained single-session proof runner rather than exposing a daemon API;
   that is now gap 3.
3. **Control-server verbs** — **implemented in source main for #4870 after
   this audit was written.** A running Pylon now exposes
   `session.spawn { adapter, accountRef|accountHome, repoRef|worktreePath,
   objective, verify }`, `session.list`, `session.events`, and
   `session.cancel` over the loopback bearer-token control plane. The script
   path remains useful for batch proofs; the daemon path is now available for
   an external orchestrator already attached to a Pylon node.
4. **`pylon accounts` CLI (goal b)** — does not exist. Honest layering:
   - `pylon accounts list --json`: enumerate locally connected
     credential homes (default + any registered alternates) per provider
     with readiness — buildable today from existing detection fns.
   - `pylon accounts usage [--account <ref>|--all] --json`: three truth
     tiers, each labeled: (i) **platform truth** — proxy the worker's
     M8 pool route (low-credit, cooldown, lease load) and token-usage
     ledger filtered by accountRef; buildable now. (ii) **local
     session truth** — Codex CLI/SDK surfaces rate-limit state in
     session events when requests are made (the M9 rotation detects
     exactly this); a lightweight probe can report
     last-observed-rate-limit per account. Claude: local credential
     presence + last-session metadata only. (iii) **provider truth** — CORRECTED
     after reading the Codex source (owner was right): both CLIs get
     usage from the provider, piggybacked on inference responses. Codex:
     every ChatGPT-backend response carries rate-limit headers —
     `x-codex-primary-used-percent` / `-window-minutes` / `-reset-at`
     (the 5-hour window), `x-codex-secondary-primary-used-percent` etc.
     (weekly), `x-codex-credits-has-credits`/`-unlimited`/`-balance`,
     `x-codex-active-limit`, `x-codex-rate-limit-reached-type` — parsed
     in `projects/repos/codex/codex-rs/codex-api/src/rate_limits.rs`
     into `RateLimitSnapshot { primary/secondary: RateLimitWindow {
     used_percent, window_minutes, resets_at }, credits }` (protocol.rs
     ~:2021) and held in session state (`state/session.rs
     set_rate_limits`). Claude Code surfaces usage the analogous way
     (rate-limit/usage data on OAuth-session API responses). So the
     design is: CAPTURE the snapshot from every session our executors
     run (the Codex SDK stream and exec output carry it), STORE it
     per-account with an observedAt, and when stale allow an explicit
     `--refresh` that runs one minimal inference to pull fresh headers.
     There is still no standalone quota endpoint we call without an
     inference — the truth tier is "last-observed from provider, aged
     N minutes," labeled as such in the schema — but the numbers are
     real provider numbers, not platform inference.
5. **Per-account usage attribution in the ledger** — token-usage rows
   keyed to provider-account refs (the filter exists; the attribution
   discipline and per-account budget surfaces do not).

## 4. Shortest path to the owner's two goals

Goal (a) — N parallel Codex sessions, distinct accounts, distinct
worktrees, driven from a chat orchestrator:
- **First-class local script path is now present (#4868/#4869):** write a
  JSON plan for `multi-session-run.ts` with each session's adapter,
  `accountRef` or direct credential home, `repoRef` or `worktreePath`,
  objective, and verification argv. The script handles bounded
  concurrency, per-session proof output, failure artifacts, and a
  redaction-scanned run summary.
- **Daemon path is now present (#4870):** an external orchestrator with the
  node bearer token can call `session.spawn`, inspect `session.list`, open
  the returned `/sessions/<sessionRef>/events` SSE stream, and cancel with
  `session.cancel`.

Goal (b) — usage command:
- One PR: `pylon accounts list|usage --json` with the three-tier truth
  model above: provider truth captured from session rate-limit
  headers/events (the Codex `RateLimitSnapshot` mechanism, stored
  per-account with observedAt + an optional `--refresh` minimal-
  inference ping), local session truth, and the worker M8 pool route
  as platform truth.

## 5. Recommended issue set (not yet filed)

1. `pylon: per-session account selection (--codex-home/--account-ref) through composer + dev-proof-run` (gap 1) — filed as #4868 and implemented in source main.
2. `pylon: concurrent multi-session spawner over worktrees × accounts (M10-grade artifacts)` (gap 2) — filed as #4869 and implemented in source main.
3. `pylon: control-server session.spawn/list/events verbs` (gap 3) — filed as #4870 and implemented in source main.
4. `pylon: accounts list/usage CLI with three-tier truth (platform/local/provider-unavailable)` (gap 4)
5. `worker: per-account usage attribution + budget surfaces in the token ledger` (gap 5)

## 6. Sources consulted

Recently closed: #4843 (work submit pinning/adapter), #4798/#4799
(materializer + native worktrees), #4839–#4846 (composer/danger/doctor,
both lanes), #4847/#4860 (proof driver + retained proofs), #4768 comment
trail (M10 harness, run live), M8 #4766 / M9 #4767 / M13 #4771 gate
records, #4858/#4859 (rc2 publish + packaged-rc evidence). Code:
`apps/pylon/src/{codex-agent,claude-agent,codex-composer,claude-composer,context-projection,dev-doctor,state,bootstrap,workspace-materializer,node/control-server}.ts`,
`apps/pylon/scripts/{dev-proof-run,overnight-proof-run}.ts`,
`workers/api/src/{provider-account-pool-routes,provider-account-client,provider-account-effective-config,token-usage,token-usage-ledger-routes}.ts`,
`docs/autopilot-coder/2026-06-11-autopilot-worktree-support-audit.md`.
