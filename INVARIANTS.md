# OpenAgents Invariants

This is the root invariant ledger for the rebuilt `openagents` Effect workspace.
More specific invariant ledgers apply inside imported apps and packages.

## Preserved Transcript Archive

- `docs/transcripts/` is retained historical material and must not be deleted,
  renamed, rewritten, or used as runtime private data.
- New refactor docs belong in `docs/refactor/`; do not mix migration planning
  into the transcript archive.

## Effect Workspace Boundary

- New production TypeScript code in this repo must use Effect. Retained server,
  CLI, test, and repository-tooling code targets the owner-selected Node 24 LTS
  destination; browser, Electron-renderer, React Native/Hermes, and native
  hosts keep their explicit runtime boundaries.
- The [`Node/pnpm/Vite Plus conversion contract`](docs/sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md)
  is complete. No supported runtime, build, test, package, hook, release,
  deploy, container, or operator path may require the retired runtime.
  Historical evidence, negative guard fixtures, and detection of third-party
  installation layouts are the only reviewed textual exceptions.
- External boundaries must be modeled with typed data structures or Effect
  Schema. Do not add ad hoc keyword routing for user intent, CRM/database
  query routing, retrieval routing, or tool selection.
- Shared runtime contracts belong in `packages/*`. App-specific UI, Worker,
  CLI, or deployment composition belongs in `apps/*`.
- The `openagents.com` deploy topology guard must keep the main product,
  Worker, shared packages, and Foldkit runtime on the tracked Effect v4 line.
  A separate isolated app may carry an older third-party Effect dependency only
  when the guard names that exact package chain, documents the exception, and
  prevents it from becoming OpenAgents.com runtime, settlement, payout, Forum,
  Pylon assignment, or product-promise authority. The current isolated
  exception is `apps/nostr-relay` through `nostr-effect@0.0.12` only.

## No GitHub-Hosted CI / Cloud Actions

- Never add GitHub Actions workflows or any GitHub-hosted CI to this
  repository. `.github/workflows/` must contain no workflow files
  (no `on: push`, `on: schedule`, `on: pull_request`, or any other
  GitHub-runner automation).
- CI, scheduled jobs, freshness re-runs (e.g. study-packet restudy), and any
  recurring automation run on OpenAgents-owned infrastructure (our GCE / cloud
  runners and cron), not on GitHub-hosted compute.
- Rationale: keep build, test, scheduling, and automation on owned infra —
  consistent with the no-Expo/EAS-cloud mobile policy — and avoid handing repo
  automation, secrets, or scheduling to third-party GitHub-hosted runners.
- **Enforced** by `check:no-github-actions` (in `check:deploy`): it fails if any
  `.github/workflows/*.yml` exists. PR-evidence / autonomous-QA on a PR is **agent- or
  manually-triggered** — an agent runs `apps/qa-runner/src/pr-comment-run.ts` (or the
  `qa-runner` directly) and posts the verdict/trace/video comment itself (e.g. PR #6224) —
  never a `pull_request`/`push`/`schedule` workflow.

## QA Swarm Run Projection Evidence

- `packages/qa-swarm-contract` owns
  `openagents.qa_swarm.run_projection.v1`. QA Runner and the OpenAgents web
  consumer must decode that same Effect Schema; an app-local schema with the
  same version string is not a compatible contract.
- A job id, configured cap, target label, or receipt-looking string is not an
  observed artifact or admitted receipt. Unpublished/unresolved trace, video,
  coverage, perf, distilled-test, and verdict refs stay absent. The projection
  remains `inconclusive` with typed blocker refs when required evidence is
  absent or its resolver is unavailable.
- A board link may be `evidence_backed` only when the receipt resolver admits
  the exact requested ref. Missing, rejected, unavailable, or substituted
  resolution cannot light the edge. Enforced by
  `packages/qa-swarm-contract/src/index.test.ts`, the QA Runner cross-package
  assertions in `apps/qa-runner/src/control.test.ts`, and the web false-green
  tests in `apps/openagents.com/apps/web/src/page/qa-swarm.test.ts`.
- Generated public boards are published only through the authenticated
  `PUT /api/operator/qa-swarm/runs/{runRef}` boundary. The Worker decodes the
  shared contract, requires the path and document `runRef` to match exactly,
  rejects private-looking material, and stores only that public-safe document
  in the owned artifacts bucket. Public reads at
  `GET /api/public/qa-swarm/runs/{runRef}` disclose no existence beyond a
  published document. The two committed Khala Code samples remain fixtures,
  not runtime lookup authority.
- A generated `/qa/{runRef}` board decodes that public read through the shared
  contract and polls only while the optional execution state is `scheduled` or
  `running`, with a hard client-side poll bound. Unknown, invalid, private, and
  unavailable runs retain the same non-disclosing unavailable shell.
- An observed discovery becomes a regression candidate only when its exact
  observation receipt is bound into a replayable, public-safe trace and the
  deterministic distiller accepts it. A candidate is `validated` only after an
  exact source-digest-bound rerun passes; failed, unavailable, substituted, or
  non-rerunnable candidates remain `INCONCLUSIVE`.
- Repository mutation for a validated regression is absent by default and can
  occur only through explicitly injected SCM authority. A `proposed` projection
  requires a scoped issue, commit proposal, and pull-request ref bound to the
  same candidate digest. Only resolver-confirmed reviewed merge evidence for
  that exact digest and pull request can produce `landed` state or populate the
  landed-only `distilledTests` projection. Enforced by
  `apps/qa-runner/src/discovery-regression-lifecycle.test.ts` and
  `packages/qa-swarm-contract/src/index.test.ts`.

## QA Swarm Assurance Execution

- The current Desktop swarm target is exactly `apps/openagents-desktop` under
  `openagents.desktop.current`; deleted Khala client paths are never execution
  targets or evidence authority.
- `apps/qa-runner/src/assurance-swarm.ts` partitions every exact Assurance
  Manifest unit once across the six typed scripted-browser, seeded-monkey,
  LLM-explorer, performance, terminal, and macOS-native lanes. Each lane has an
  independent action, duration, and model-token budget. Observed adapter output
  emits independently digestible normalized Assurance Receipts. Those receipts bind the exact
  ProductSpec, AssuranceSpec, admission, Manifest, environment, locked adapter,
  execution unit, command, source, native report, and artifact commitment.
- Real execution, provider spend, and native control require explicit arming.
  Unsupported, missing, failed, or unarmed adapters remain `INCONCLUSIVE` with
  no fabricated native report, artifact commitment, or Assurance Receipt; they
  may not be inferred from configured jobs or artifact slots. A lane adapter
  must equal every assigned Manifest unit's locked adapter. Non-model lanes
  report exact zero provider usage; an executed model lane must report exact
  observed input and output usage within its declared cap. The orchestrator is
  evidence-only: it cannot admit, accept, merge, deploy, or promote a promise.

## Observer Semantic Planning

- `packages/assurance-spec/src/semantic-planner.ts` owns the provider-neutral
  semantic-planner request, response, and deterministic proposal compiler.
  Planning starts only from an explicit accepted ProductSpec identity pin; the
  exact repository-relative path, revision, document digest, and ordered
  criterion set must match the supplied ProductSpec bytes.
- Planner output disposes every exact criterion once. Unknown, duplicate, or
  missing criterion ids, changed input/subject bindings, malformed designed
  proof, self-verification, and label-only seams fail closed. A planner cannot
  supply or rewrite source-claim snapshots or digests; the compiler copies
  those only from the checked request. Weak proof may remain an explicit
  `needs_design` disposition but may not masquerade as designed.
- Semantic planner implementations are injected Effect programs. Model and
  provider calls remain outside request parsing and deterministic compilation.
  The provider-free fixture planner deliberately returns only `needs_design`.
- Every compiled semantic result is an AssuranceSpec with lifecycle
  `proposed`. It cannot self-admit, execute, verify, release, or change a public
  promise. Review annotation and admission of an exact later revision/digest
  remain separate boundaries. Enforced by
  `packages/assurance-spec/test/semantic-planner.test.ts` and the Observer CLI
  assertion in `packages/assurance-spec/test/cli.test.ts`.

## Product Surface Ownership

- `apps/openagents.com/` owns the `openagents.com` product surface and retains
  its local invariant ledger.
- `apps/forum/` owns forum-specific code and must mount under `/forum` when it
  is served by `openagents.com`.
- `apps/forge/` owns the separate `forge.openagents.com` UI surface. It may
  consume shared Effect Native primitives/tokens and Forge API contracts,
  but it does not own runtime promotion, settlement, payout, accepted-work
  authority, or the main `openagents.com` logged-in route tree.
- `apps/pylon/` owns contributor-node UX, CLI, and local runtime orchestration.
  It owns no wallet, payout, settlement, or paid-capacity authority.
- `packages/probe/` owns Probe runtime code and evidence submission helpers.

## OpenAgents Cloud (in-repo)

- Managed Cloud infrastructure (`oa-codex-control`, `oa-node`, `oa-workroomd`,
  `openagents-cloud-contract`) lives in this monorepo under `crates/*`.
- The private `OpenAgentsInc/cloud` repository is historical source only and
  must not receive new feature work; see `docs/cloud/MIGRATION.md` and issue
  #8591.
- Agent Computer, Cloud-VM, GCE capacity, workroom, capability, and receipt
  code builds from public openagents source. Live secrets, topology, and host
  paths remain runtime/Secret Manager only.
- Cloud daemons execute and emit redacted non-money receipts. They do not own
  user credit ledgers, public claim promotion, wallet, payout, payment, or
  settlement authority. The former Worker and MDK/Nexus money authority is
  retired under VP-1; preserved records are recovery evidence only.
- Fake GCE and fake Cloud-VM provisioners are the default. Live Firecracker
  and live GCE lanes are explicit env-gated owner modes.
- Detailed Cloud invariants live in `docs/cloud/INVARIANTS.md`.

## Authority Boundaries

- Public UI does not own settlement, payout, runtime promotion, or accepted
  outcome authority.
- Payments, markets, Sites, tipping, wallet custody, payout, billing credits,
  and settlement are not part of the accepted MVP. VP-1 retires rather than
  ports them: mutation surfaces return typed `money_surface_retired`, runtime
  authority stays at zero, and paid external capacity must fail closed rather
  than become free. Applied migrations, redacted receipt refs, private recovery
  archives, and deliberately frozen obligations remain evidence, never active
  product authority. Any revival requires a fresh owner-approved design,
  custody model, invariant change, and proof program.
- Probe evidence does not authorize deployment, spend, provider mutation, or
  public claim promotion without a separate approved authority path.
- Retained Pylon history may describe old payment states, but active assignment
  admission supports no paid mode and exposes no earning or payout claim.
- Pylon local supervised danger modes (Codex `danger-full-access`, Claude
  `bypassPermissions`) are explicit owner-local opt-ins only: local composer
  and authenticated loopback control sessions may honor the local dev overlay.
  Claude bypass is additionally a revocable, process-opaque authority bound to
  one exact local Pylon, run/turn/session, operation/assignment, and named
  account. Serialized/restarted, bridge, org-cloud, public, remote, mismatched,
  expired, and revoked paths cannot inherit it; public audit evidence carries
  policy/authority/proof refs, never the permission-mode literal.
  Fleet supervisor loops and their Codex/Claude dispatches are owned by the
  exact run scope: stop/close aborts and joins them before releasing the Pylon
  slot, while late lifecycle remains retained. Completed/accepted publication
  waits for the matching exact verifier and terminal closeout evidence; a
  delayed, cancelled, restarted, rejected, or stale attempt cannot be promoted.
  Exact own-capacity usage keeps provider counters truthful: uncached input,
  output, reasoning, and cache-read tokens are independently nonnegative;
  cache reads may exceed uncached input (as in Claude warm prompts), while
  total tokens remain exactly uncached input plus output.
  Caller-owned Khala -> Pylon -> Codex coding delegation is also owner-local:
  when the caller is routed to their own linked Pylon, the local Codex executor
  uses the SDK equivalent of
  `--dangerously-bypass-approvals-and-sandbox` (`danger-full-access` plus
  approval policy `never`) so real GitHub/worktree operations can complete.
  Codex selects that posture only when the trusted local no-spend runner passes
  its non-serializable process-local own-capacity control and the lease is also
  `no-spend`; lease/task/config fields alone cannot mint the exception. Other
  Codex executor calls remain bounded with provider network access disabled.
  In that exact owner-local, own-capacity, no-spend posture, a long-lived SCM
  credential finding remains an honest typed closeout disclosure/caveat; it
  cannot retroactively convert completed, verified, or already-landed work into
  a credential-policy rejection. The Claude twin may use the same disclosure
  posture only when its live process-opaque owner-local authority and no-spend
  lease both match the execution. Missing, paid, bounded, remote, labor, and
  otherwise non-owner postures receive no such exception and fail closed before
  provider execution when the preflight scan detects credential material.
  Untrusted labor, provider, and public command paths still reject caller-
  supplied danger flags with a typed blocker, and assignment-safe config
  loaders never read a permissive mode from public wire/config.
- Real Khala fleet-run coding dispatch must use named, isolated account refs
  from the caller-owned Pylon account registry. Automatic real-work fanout must
  not route through the display/default Codex account or omit `--account-ref`;
  that would write rollouts under the operator's default `~/.codex` home.
  Named Claude readiness is provider-proven, not inferred permanently from a
  credential directory: an exact provider-disabled refusal removes the local
  execution capability and suppresses stale local-session availability until
  a later successful bounded provider probe clears that health record. Failed
  Claude turns never manufacture a zero-token successful-usage observation,
  and public projections expose only the bounded provider-disabled blocker.
  A runtime-intent supervisor rooted at an explicit Pylon home must also bound
  implicit sibling-home discovery to that Pylon's account root; it may widen
  discovery only through an explicit account-home root and must never infer
  the owner's default `~/.codex` or `~/.claude` custody from `$HOME`.
  Local Pylon control sessions must also prefer healthy connected Codex account
  registry entries before falling back to the default Codex home. Provider
  usage exhaustion, rate limiting, and auth revocation are typed account-health
  failures; they must update the local health/quota ledger, surface as
  account-specific failure classes, and retry another healthy connected Codex
  account when one exists.
  Delegate dispatch failures are classified as typed transient or permanent
  reasons before retry decisions. Pylon persists account/lane breaker state
  from those failures, treats permanent credential/safety failures as
  quarantines, treats transient failures as bounded cooldowns, and feeds active
  breakers into delegate readiness/capacity so background fanout does not keep
  dispatching into a known bad account lane.
  Fleet-run supervisor ticks are serialized per Pylon/run handle so startup
  status reads and the background loop cannot over-dispatch past the target
  concurrency. Regression coverage lives in Pylon's `src/orchestration`
  manager, supervisor, owned-runner, and standing-executor tests; deleted
  desktop adapters are recoverable at `c7044f5a28` only.
  Hybrid FleetRun placement is work-unit authority, not run-wide inference:
  each plan unit may carry the shared typed target, quota, marginal-cost,
  data-posture, repository, and task constraints. The one supervisor evaluates
  those constraints before claiming, uses one work-claim registry across
  owner-local and managed-cloud capacity, and projects the selected capacity
  class on every v2 execution event. Explicit targets never substitute; an
  `auto` decision retains its complete typed skip history. Managed-cloud units
  still require the broker-authorized Agent Computer adapter and may never
  inherit owner-local subscription or credential authority.
- Khala Code mobile-only MVP cloud execution uses OpenAgents-owned Agent
  Computers: Firecracker microVMs on our GCE capacity, assigned per admitted
  work context and metered with refs-only lifecycle/resource receipts. Agent
  Computers are not user Pylons, are not another user's capacity, and are not
  wallet or payout authority. They may receive only scoped runtime credentials
  and SCM broker-issued repo credentials; assignment payloads must carry only
  broker refs, never embedded tokens, PATs, credentialed URLs, or credential
  helper output. Raw user OAuth tokens, provider master keys, wallet material,
  raw GCE topology, guest IPs, prompts, repo content, and private traces must
  not enter public projections, docs, issue comments, tests, fixtures, or logs.
- Agent Computer result writeback must use the same user-authorized SCM broker
  path. Git operations may create/push only scoped task branches; they must not
  force-push, must not push to the base branch, and must surface permission
  failures as typed public-safe refs. Branch and pull-request URLs may be
  projected only as thread-scoped runtime event metadata or public-safe closeout
  refs, never with raw credentials or diff payloads. Whether writeback opens a
  pull request is a user-controlled preference: the default pushes the branch
  and opens a PR, while a `branch_only` preference pushes the same scoped branch
  and opens no PR. Both modes obey the no-force-push and no-base-branch rules and
  emit the same `writeback.recorded` thread-scoped runtime event (with a
  `branch_pushed` status and no pull-request fields for branch-only).
- Agent Computer placement is bound to a single work-context ref. The public
  Worker must fail closed when the control plane omits or mismatches that ref,
  and it may treat cleanup as reclaimed only when receipt refs prove scratch
  wipe and microVM destruction.
- Agent Computer admission is an additive mobile org-cloud gate. It must require
  a mobile user bearer session, positive user credit balance, per-user
  rate/concurrency allowance, and OpenAgents-owned Agent Computer capacity
  before placement. It must refuse with typed `insufficient_credit`,
  `rate_limited`, or `org_capacity_unavailable` outcomes, and it must never use
  caller-supplied Pylon/user-capacity selectors to route through another user's
  machine.
- Secrets, wallet material, raw prompts, private repo content, provider
  payloads, and private customer data must not be committed or written into
  docs, tests, fixtures, logs, or public projections.

## Desktop Release Artifact Authority

- Khala Code Desktop public distribution is not a code-complete claim until the
  owner records public-safe receipts for the signed app, notarized app, stapled
  app, recreated/signed/notarized/stapled DMG, updates-feed upload, GitHub
  release, and clean-Mac first-run smoke. The smoke must prove the app boots
  from the DMG and shows the honest Codex install/login path when Codex is
  missing or unauthenticated.
- Khala Code Desktop releases publish only to the product-specific feed
  `desktop/khala-code-desktop/<channel>/feed.json` and tag as
  `khala-code-desktop-v<version>`. The legacy `/desktop/<channel>/feed.json`
  route remains the default Autopilot Desktop lane. RC/prerelease Khala builds
  must use the `rc` channel and GitHub `--prerelease --latest=false`; stable
  latest eligibility is reserved for non-prerelease versions.
- Public Khala Code install or download surfaces may route users to Codex
  prerequisites, the npm `khala` CLI, and source-build instructions, but must
  keep desktop DMG availability marked pending until the receipt set above
  exists. Public download counters must be exact grouped
  `khala_code_download_events` rows or an explicit empty response with blocker
  refs; page views, feed presence, or planned artifacts are not install counts.
- (CUT-26, #8706) The legacy Electrobun desktop serving lanes above are
  historical: the **legacy desktop lockout**
  (`apps/oa-updates/src/legacy-desktop-lockout.ts`) is ARMED BY DEFAULT, and
  every legacy desktop feed/OTA route (`/desktop/<channel>/feed.json`,
  `/desktop/{khala-code-desktop,autopilot-desktop}/...`, and the flat
  Electrobun `/desktop/<file>` OTA route) answers one typed `410`
  `openagents.desktop.legacy_lockout.v1` document instead of content. Only
  the exact `OA_LEGACY_DESKTOP_LOCKOUT=disarmed-historical-read-only` value
  re-enables archival read-only serving; every other value stays armed (fail
  closed). Publishing new legacy releases remains refused independently
  (`assertDesktopReleaseProductPublishable`), and the deprecated clients
  receive no new features — including no remote kill-switch capability.
- (2026-07-14 owner supersession) The `apps/autopilot-desktop` source tree
  itself was deleted at owner direction ("OpenAgents desktop supercedes it");
  recover via `git show c7044f5a2870110b331c5a7288caceb85488290a:<path>`. The
  CUT-26 lockout routes above are unaffected: `updates.openagents.com` keeps
  answering the legacy `autopilot-desktop`/legacy-feed routes with the typed
  `410` lockout document — the route tombstones outlive the source tree.
  `clients/khala-code-desktop` was deleted in #8793 after its Pylon and QA
  dependents migrated. No executable package, import, smoke, or QA matrix may
  depend on that path again; `scripts/khala-code-desktop-removal.test.ts`
  enforces the bounded absence oracle. Historical evidence resolves through
  git at `c7044f5a28` and the backroom supersession intake.
- (CUT-26, #8706) OpenAgents Desktop releases publish ONLY through the
  scripted flow (`apps/openagents-desktop/scripts/publish-release.ts`) into
  the `openagents-desktop-release.json` descriptor + signed
  `update_manifest.v1` shape served at
  `/desktop/openagents/<channel>/manifest.json` + `manifest.sig.json` +
  `release.json`. Version monotonicity and channel rules (strictly newer
  only; no pre-release on stable; downgrades refused unconditionally) are
  enforced at publish time, the signed bytes are self-verified through the
  exact client verification seam before staging, the seed boundary re-checks
  the manifest digest at boot, and the client verifies against its pinned
  release key — all fail closed. The ed25519 release private key enters the
  publisher only through the documented env seam
  (`OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D` + `_KID` or
  `OPENAGENTS_RELEASE_SECRETS_PATH`) and is never printed; tests use
  in-process fixture keypairs only, and a key claiming the production kid
  whose derived public key differs from the committed pin is refused at
  publish time. The unsigned `artifactUrl` is transport only — the download
  is gated by the SIGNED sha256/byteLength.
- OpenAgents Desktop macOS artifacts carry the product-owned
  `resources/openagents-icon.icns` bundle. Finder, Dock, ZIP, and DMG output
  must never fall back to Electron's atom icon; the packaging contract test
  validates both the Forge input and the ICNS container before release.
- Khala Code outside-user run evidence is opt-in only. The desktop may offer a
  "post run receipt" control, but it must not phone home or submit evidence on
  startup, refresh, harness inspection, or page view. Public run receipts may
  contain only app version, platform, architecture, distribution channel, and
  bounded harness readiness; they must not store or project paths, prompts,
  logs, tokens, account identifiers, machine identifiers, request body blobs, or
  user identity. A run receipt is evidence only: it does not replace the signed
  DMG/notary/update-feed receipt set and does not by itself flip a product
  promise green.

## Background Agent Definition Tool Authority

- Harness-agnostic background agents are defined by
  `openagents.agent_definition.v1` in
  `packages/agent-runtime-schema`. The durable definition owns the standing
  workflow contract: name, goal, harness hint, lane, triggers, budget,
  escalation, source refs, and the explicit toolset.
- The harness field is never authority. Codex, Claude Code, Khala, hosted,
  custom, or fixture adapters may execute only after their local or cloud
  tool boundary compiles and enforces the definition's toolset.
- `decideAgentDefinitionToolAuthority` is the shared deny-by-default contract:
  explicit deny rules beat ask and allow rules, ask rules create an operator
  escalation record without authorizing execution, allow rules authorize only
  the matched tool ref, and unmatched tools are denied.
- `compileAgentDefinitionToolRuntimePolicy` materializes that contract as
  `openagents.agent_definition_tool_runtime_policy.v1` before a lane starts
  executing tools. Local Khala tool execution must enforce the compiled policy
  against both the tool name ref and authority ref before any tool body runs.
- Forge tenant git tokens for definition-backed work compile requested git
  scopes through the same policy boundary. `git:receive-pack`,
  `git:upload-pack`, and `git:admin` map to Forge git tool refs; denied scopes
  are rejected before token mint, and ask scopes create operator escalation
  instead of minting a token. Definition dispatch records only token refs on the
  Forge work/run rows, scopes receive-pack tokens to the task repository/ref
  when minted, and revokes those refs on Pylon closeout. Regression coverage
  lives in `workers/api/src/agent-definition-run-routes.test.ts`,
  `workers/api/src/forge-tenant-git-auth-store.test.ts`, and
  `workers/api/src/forge-git-intake-routes.test.ts`.
- Definition-backed Pylon `git_checkout` workspaces that need Forge SCM access
  must receive only ref-only broker metadata (`scmAuthBroker`), never an
  embedded SCM token, PAT, credentialed URL, or long-lived secret. The Pylon
  workspace materializer owns the worker-side Git credential helper install:
  helper config lives under Git's private admin directory, is scoped by
  protocol + host + path, uses a bounded short cache, reads control-plane auth
  only from the Pylon process environment, and fails closed unless the
  assignment explicitly allows anonymous read-only fallback. Runtime
  materialize/run/closeout paths must enforce
  `scanLongLivedScmCredentials`: Codex and Claude git-checkout runners scan the
  bounded workspace plus selected isolated account home after materialization,
  immediately before provider execution, and again before verification or PR
  publication. Non-owner and labor findings are typed fail-closed refusals; an
  exact owner-local own-capacity no-spend finding is instead a typed closeout
  disclosure that does not contradict completed or landed work. Lease cleanup
  still removes token-leaked workspaces even when they are dirty.
- Pylon prepared-worktree reuse is local-only and keyed by repository full name
  plus pinned baseline commit. Cleanup may snapshot only clean, credential-free
  workspaces with the typed `post_completion_snapshot` reason. Restore must
  validate the prepared entry's metadata, Git root, HEAD, and clean status, then
  perform local clone + `git reset --hard` + `git clean -ffdx` with the typed
  `restore_quick_sync_reset` reason before handing the workspace to an executor.
  Dirty, stale, or malformed prepared entries are removed rather than reused,
  and prepared-cache disk usage is bounded by byte-budget eviction of oldest
  entries. Regression coverage lives in
  `apps/pylon/tests/workspace-worktree.test.ts` and the enforced
  `background_agents.warm_dispatch.prepared_worktree_cache.v1` behavior
  contract.
- Pylon prebuilt-baseline reuse is local-only and keyed by repository full name
  plus branch. A prebuilt registry row may be refreshed only through the bounded
  upstream staleness cadence, records the observed upstream commit and setup
  result, and must keep honest hit/miss counters. A matching cold dispatch may
  start from the prebuilt directory only when the requested pinned commit equals
  the registry baseline; otherwise it records a miss and falls back to normal
  `git_worktree` materialization. Prebuilt setup artifacts are allowed only as
  ignored local cache material, registry rows and paths are never public
  authority, and refresh/setup/restore failures must degrade to a miss rather
  than blocking normal materialization. Regression coverage lives in
  `apps/pylon/tests/workspace-worktree.test.ts` and the enforced
  `background_agents.warm_dispatch.prebuilt_baseline_cache.v1` behavior
  contract.
- Runtime runs may link back to `agentDefinitionId` as evidence that a run was
  definition-backed, but that link alone grants no tool, spend, dispatch,
  payout, settlement, public-claim, provider-account, or external-send
  authority.
- Durable definition triggers are persisted as
  `openagents.agent_definition_trigger.v1` rows in the owner-scoped
  `agent_definition_triggers` table. The definition still owns the trigger
  contract; the trigger table owns only operational scheduler state:
  `next_run_at`, enable/pause state, pause reason, and consecutive failure
  count.
- `next_run_at` is a precomputed scheduler hint for cron triggers, not
  dispatch authority. A due, enabled trigger must still pass the scheduler,
  definition-run route, lane/toolset policy, accounting, and owner-scope gates
  before any work starts. Inbound webhook trigger rows likewise store typed
  source/condition configuration only; verified ingress, normalization, and
  condition evaluation are separate authority steps.
- Cron trigger dispatch must be serialized through the named
  `AGENT_DEFINITION_SCHEDULER` Durable Object woken by Worker `scheduled()`.
  Request isolates, routes, webhook ingress, or ad hoc workers must not scan
  and dispatch due cron rows directly. Each scheduler tick processes due rows
  oldest-first under a bounded cap, and every attempted cron dispatch must move
  `next_run_at` to the next cron instant before another tick can consider the
  row again. Refusals and failures increment/preserve the failure streak rather
  than retrying in a tight duplicate loop.
- Auto-pause after 3 consecutive failures; `maxRunsPerDay` /
  `maxRunSeconds` / `maxCreditsPerDay` are enforced at dispatch with typed
  refusals - a buggy background watcher must never be a money pump. Dispatch
  refuses invalid budgets, refuses owner+definition rows that already hit the
  UTC daily run cap, refuses rows whose reserved daily credits exceed the
  configured credit cap, and writes the definition's run-second cap into the
  Pylon assignment timeout. Trigger failure recording atomically pauses the
  owner-scoped trigger row on the third consecutive failed/refused attempt.
- Inbound definition webhooks must verify the source signature before parsing
  or normalizing provider payloads. GitHub ingress is owned by
  `/v1/agent-definitions/webhooks/github`, verifies the `x-hub-signature-256`
  HMAC with the configured webhook secret, drops invalid requests before
  reading trigger rows, normalizes through
  `@openagentsinc/agent-runtime-schema/webhooks`, and evaluates only typed
  conditions on the bounded normalized event. Raw webhook bodies, signatures,
  and provider payloads must not become model-visible trigger payloads.
  Dispatch remains owner-scoped: matching trigger rows read the definition
  with the row's `ownerAgentUserId` before using the shared definition-run
  dispatch helper.
- Forum-triggered definition runs use the same bot-integration template, with
  source-specific authority. Forum ingress is owned by
  `/v1/agent-definitions/webhooks/forum`, verifies
  `x-openagents-signature-256` before parsing, verifies that the bounded Forum
  event names an existing readable source post/topic/forum, normalizes only
  public-safe Forum refs through
  `@openagentsinc/agent-runtime-schema/webhooks`, and dispatches matching
  `inbound_webhook` rows through the same owner-scoped definition-run helper.
  The Forum completion callback route may not accept an arbitrary topic/post
  target from the caller: it reads the stored definition-run trigger payload,
  decodes the Forum callback descriptor written at dispatch time, and posts
  only back to that source thread through Forum writer context, topic/forum
  lock checks, idempotency, and write-policy enforcement.
- GitHub @mention definition runs are limited to signed
  `issue_comment.created` source events with a configured bot mention. The
  GitHub webhook route may use the raw comment body only to extract that
  bounded mention fact; raw comment text, webhook body, signatures, and
  provider payloads must not become model-visible trigger payloads. Matching
  triggers dispatch through the same owner-scoped bot-integration template and
  store a GitHub completion callback descriptor on the run trigger payload.
  The GitHub completion callback route may not accept an arbitrary repository,
  issue, pull request, or comment target from the caller: it reads the stored
  definition-run trigger payload, decodes the GitHub callback descriptor
  written at dispatch time, and posts at most one idempotent result comment
  back to that source issue or PR conversation through the GitHub issue
  comments API. It must not create new GitHub issues or loose bug reports.
- Per-definition run history and manual run-now endpoints remain
  registered-agent, owner-scoped views over stored definition-run rows.
  `GET /v1/agent-definitions/:id/runs` must first read the definition for the
  authenticated owner, then list only that owner+definition's rows with status,
  trigger, and opaque receipt/evidence refs. `POST
/v1/agent-definitions/:id/run-now` may dispatch only through a definition's
  explicit `manual` trigger and must reuse the same dispatch, budget, lane,
  toolset, Pylon, Forge, and exact-accounting gates as any other trigger.
  Manual run-now must not become an owner-scope bypass or a second dispatch
  path.
- A per-run live Durable Object is not a default background-agent transport.
  Durable Streams remain the default run-live/resume surface until WS-10 grows
  an explicit client-facing live channel and an operator enablement gate opens
  the thin-DO candidate. Any future live object must be keyed by
  owner+definition-run, act only as a thin transport shell around injected
  services, track in-object SQLite migrations through `_sql_schema_migrations`
  rather than `PRAGMA user_version`, persist hibernatable WebSocket attachment
  metadata without raw prompts/provider payloads/tokens/secrets, and multiplex
  all scheduled work through one durable alarm task table. Regression coverage
  for the design gate lives in
  `apps/openagents.com/workers/api/src/agent-definition-live-surface-spike.test.ts`.
- `event_ledger.v1` rows for the background-agent unified inbox are private,
  owner-scoped account-boundary data. GitHub and Slack source events may enter
  only after source-specific signature verification and typed normalization,
  and only matched owner triggers provide the owner boundary for ledger ingest.
  Queue messages and D1 rows store source refs, external refs, actor refs,
  content refs, subject refs, bounded summaries, and timestamps; they must not
  store raw webhook bodies, raw comment/message text, provider payloads,
  secrets, signatures, tokens, or training/eval consent. The per-owner
  `EVENT_LEDGER_OWNER` Durable Object owns ordering and dedupe before D1
  persistence.
- Handled-state is now part of the private event-ledger contract. Ledger rows
  may move only among `open`, `handled`, `responded`, and `ignored`, and any
  handled-state mutation must record the owner-scoped definition run and
  definition that touched the row. Definition-backed reads must go through the
  authenticated event-ledger gateway, must pass the compiled definition
  toolset (`tool.openagents.event_ledger.read` for reads and
  `tool.openagents.event_ledger.handled_state.write` for state changes), and
  must redact according to the definition `secretPolicy`. Cross-owner reads,
  unrelated-run touches, public projection, raw source payload disclosure,
  model-visible unredacted context, and training-data use remain forbidden.
  Regression coverage lives in
  `apps/openagents.com/workers/api/src/event-ledger.test.ts`,
  `apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts`,
  and `packages/agent-runtime-schema/src/webhooks.test.ts` plus
  `apps/openagents.com/workers/api/src/agent-definition-event-ledger-routes.test.ts`.
- Any Worker, Pylon, desktop, or cloud-workroom executor that claims
  definition-backed tool enforcement must use this contract or a formally
  equivalent compiled policy at the execution boundary, with regression tests
  for deny precedence, ask escalation, allow, and default-deny behavior.
- Regression coverage starts in
  `packages/agent-runtime-schema/src/index.test.ts`,
  `packages/agent-runtime-schema/src/webhooks.test.ts`,
  `packages/khala-tools/src/dispatcher.test.ts`,
  `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.test.ts`, and
  `apps/openagents.com/workers/api/src/agent-definition-trigger-store.test.ts`,
  plus
  `apps/openagents.com/workers/api/src/agent-definition-scheduler.test.ts` for
  singleton tick semantics, cap handling, owner scope, and next-run advancement,
  and
  `apps/openagents.com/workers/api/src/agent-definition-webhook-routes.test.ts`
  for signature-gated GitHub ingress, typed condition matching,
  owner-scoped dispatch, GitHub @mention runs, GitHub completion idempotency,
  Forum-triggered runs, and Forum completion callbacks,
  and
  `apps/openagents.com/workers/api/src/agent-definition-run-routes.test.ts` for
  dispatch budget refusals, capped assignment timeouts, owner-scoped run
  history, receipt refs, manual run-now, and ref-only SCM auth broker
  projection, plus
  `apps/pylon/tests/workspace-materializer.test.ts` for broker metadata
  validation, worker-side Git credential helper installation, and the
  long-lived SCM credential scanner, plus
  `apps/pylon/tests/workspace-worktree.test.ts` for closeout cleanup of leaked
  dirty workspaces,
  `apps/pylon/tests/codex-agent-executor.test.ts` and
  `apps/pylon/tests/claude-agent-executor.test.ts` for pre-execution non-owner
  workspace/home credential-policy refusals, post-run containment, and exact
  owner-local own-capacity disclosure closeouts, plus
  `packages/agent-runtime-schema/src/index.test.ts` for reusable fixtures that
  cover every supported trigger type.

## Connector Authority And Redaction

- Connector sidecars never own workspace, payment, email, membership,
  settlement, identity, or broad provider-account authority. The platform
  remains authoritative for those state changes; connectors may only emit
  source-verified, bounded events and execute explicitly modeled per-connector
  tools.
- Before any connector event reaches model context, session history, logs, or
  outbound provider mutation, provider credentials, authorization headers, raw
  webhook bodies, raw payloads, signatures, and webhook secrets must be
  excluded or redacted. Public or model-visible connector envelopes may carry
  only typed subjects, source refs, redacted refs, booleans, timestamps, and
  blocker/caveat refs.
- Outbound connector mutation must pass an app-owned idempotency gate before
  dispatch. Provider retry keys alone are not enough; the OpenAgents connector
  contract owns the dedupe key and the bounded receipt/projection.
- Generic provider tools are forbidden. Tool authority must name the connector,
  provider, subject kind, and operation, and it must stay bound to the verified
  event subject, such as one issue or one pull request.
- Regression coverage for the BF-6 connector gate lives in
  `packages/connector-sidecar/src/index.test.ts`, including denial cases for
  raw provider material in context/history/logs, missing app-owned idempotency,
  generic provider tools, and platform-authority widening.

## Cloudflare Verse World Service

- Live Verse world work belongs to `apps/openagents-world/`, a Cloudflare
  Worker + Region Durable Object service written in TypeScript, Effect, and
  Effect Schema. Durable Objects are the coordination atoms for live presence,
  local interaction, interest-scoped fanout, hibernatable WebSockets, handshake
  buffering, sequence acknowledgements, TTL expiry, and per-region world state.
- `packages/world-contract/` owns public-safe world schemas and command/delta
  contracts. `packages/world-client/` owns the desktop/web client projection
  that mirrors snapshots and deltas into a read-only `WorldReadModel`.
- Worker/D1 public product surfaces remain authoritative for public training
  truth, product promises, receipt-backed proof claims, settlement/payout
  projection, and Forum/product state. The Verse world service owns only
  public-safe presence, local interaction, interest-scoped fanout, diagnostic
  rows, and replayable projection rows derived from public source refs.
- The world service and client projection do not own settlement, payout,
  training truth, product promises, receipt validation, accepted-work authority,
  wallet state, provider credentials, private prompts, private repo content,
  private customer data, or unpublished provider payloads.
- Public world rows and deltas may expose only public-safe refs, labels,
  positions, timestamps, staleness metadata, movement caveats, moderation state,
  and dereferenceable proof URLs that are already safe for public OpenAgents
  surfaces.
- Browser/user commands may update only explicitly modeled interaction state,
  such as joining/leaving a region, bounded avatar pose, focus, local chat,
  emotes, and ephemeral intent. Service-only commands that create or mutate run,
  entity, edge, proof, settlement, event, cursor, bridge-health, or projection
  rows must require an allowlisted service identity.
- Actor command authority is modeled in
  `docs/game/2026-06-22-cloudflare-world-actor-command-authority-model.md` and
  enforced by `packages/world-contract` plus `apps/openagents-world` command
  tests. Counterexamples must become tests before broadening command authority.
- `/tassadar` authority remains the Worker/D1 public summary path until a later
  invariant change explicitly promotes a different authority. The Verse world
  service may enrich or animate the scene only from public refs or timestamped
  projection transitions.
- The deleted self-hosted world module is historical source material only. Do
  not reintroduce it for production world behavior; port useful schema or
  reducer ideas into the Cloudflare/Effect world service.

## Public Projection Staleness

- Every public projection in this workspace carries `generatedAt` (or an
  equivalent rebuild timestamp) plus a declared staleness contract, and either
  rebuilds on the state transitions that matter or composes live at read. A
  projection that cannot meet its declared staleness must say so in the
  payload rather than serve stale data as current.
- The `openagents.com` worker-surface contract vocabulary, the enumerated
  projection inventory, and the enforcing check tooling live in
  `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness
  Declaration") and `apps/openagents.com/scripts/check-zero-debt-architecture.mjs`
  (epic #4751).

## Product Promise Claims

- User-facing and agent-facing product claims belong in the product-promises
  system under `docs/promises/` before copy broadens beyond implementation
  notes.
- A product promise is green only when its evidence refs, authority boundary,
  projection safety, freshness, and copy gate are all satisfied for the exact
  claim being made.
- Planned, partial, stale, blocked, manually gated, or canary-only behavior
  must stay red, yellow, degraded, or explicitly scoped in public and
  agent-readable copy.
- Product promise mismatch reports from users and agents are Forum-first. The
  default public intake is the Product Promises Forum at
  `https://openagents.com/forum/f/product-promises`.
- GitHub issues may be opened only for concrete, reproducible bugs that
  satisfy the strict bug report template. Blank issues are disabled, and
  malformed, broad, or loose reports should be rejected by the issue form or
  moved back to the Forum rather than becoming normal product-promise intake.
- This initial promise system is documentation-backed. Runtime enforcement must
  be added before treating the registry as an automated product gate; until
  then, `docs/promises/checks-and-gates.md` is the model-boundary record.

## Commit Metadata Privacy

- Commit messages, commit trailers, and other committed metadata must not
  include individual people’s names unless the user explicitly requests a
  legally or historically required attribution.
- Prefer neutral product, team, source, operator, reporter, maintainer, or role
  wording in commits and committed process records.

## Khala Sync Replication Substrate

- Khala Sync (Cloud SQL Postgres → per-scope Durable Object hubs → SQLite
  clients) carries scoped state under the nine invariants of
  `docs/khala-sync/SPEC.md` §7: dense server-assigned versions, no
  optimistic effects in durable client stores, attributable changelog
  entries, idempotent at-least-once apply, single-transaction mutators,
  MustRefetch behind the retained window, scope access control,
  exact-source public projections, and redacted post-images.
- The full registration — per-invariant statements, the exact enforcing
  test files and test names, and honest `partial`/`pending` statuses with
  blocking issue refs — lives in `apps/openagents.com/INVARIANTS.md`
  ("Khala Sync (SPEC §7 invariant set)"), because that Worker owns the sync
  routes and the hub DO. Substrate and client enforcement lives in
  `packages/khala-sync-server` and `packages/khala-sync-client` test
  suites named there.
- Desktop `node:sqlite` and mobile Expo SQLite are thin host adapters over the
  same `packages/khala-sync-client` store core. The host owns the database
  handle and installation identity, closes the authenticated Sync session
  before the store on process/OTA teardown, and exposes only bounded phase/
  freshness state to Effect Native views; a local cache is never authenticated
  or server-authoritative Sync.
- Catch-up pages and advancing live deltas must cover every dense server-
  assigned scope version from the durable cursor through the advertised
  cursor. A sparse or non-progressing batch is a protocol failure: the client
  keeps its durable cursor and reconnects for authoritative replay; retained-
  window loss enters the existing MustRefetch/snapshot-replacement path.
  Duplicate or stale frames remain idempotent no-ops.
- Every awaited bootstrap/log response is fenced by the requesting scope
  generation before it can replace or advance durable state. Unsubscribe,
  close, and proven revocation invalidate that generation; a late push response
  cannot acknowledge or publish rejection state after revocation. On the
  server, a runtime event must equal the turn's durable next `event_count`
  (the first event is sequence `0`; after `N` admitted events the next is
  sequence `N`) and match its lifecycle state: only `turn.started` can leave `queued`, a second
  start cannot mutate `running`, and no provider event can mutate a completed,
  failed, interrupted, or closed turn. A stale hosted worker is settled as one
  durable `turn.interrupted` event and is never re-run through inference.
- The shared local store records `store_schema_version` independently of the
  Sync protocol/client identity version. The current app migrates the supported
  unversioned legacy store in place, preserving rows/cursors/queue, but inspects
  this marker before additive SQL and refuses newer/invalid versions with typed
  `incompatible_version` recovery guidance. Desktop, Expo/mobile, and Web must
  preserve that typed refusal rather than opening or rewriting the cache.
- After native-session verification, Desktop main and the mobile Expo host
  alone may compose the shared HTTP/WebSocket session and subscribe
  `personalScope(serverDerivedOwnerUserId)`. The access-token callback is
  re-read by the transport so bounded rotation does not expose a token; owner
  refs, credentials, transport/session objects, store handles, and raw rows
  never enter either view program.
- Native conversation clients use the one shared client implementation of the
  server's canonical `chat.createThread` / `chat.appendMessage` mutators and
  `chat_thread` / `chat_message` schemas. View-facing confirmed projections
  omit owner identity and carry stable thread/message refs, server entity
  versions, scope cursor, phase, and pending count; optimistic content is never
  labeled confirmed, and denial/sign-out removes the conversation capability.
  Optional image bytes on `chat_message` remain inside the owner-private exact
  thread scope: at most four closed-type PNG/JPEG/GIF/WebP payloads, at most
  2 MiB decoded each, with decoded length, file signature, and SHA-256 checked
  by the server before storage/changelog admission. Native paths and picker
  URIs never enter Sync. The trusted Pylon reader may materialize an image only
  into a turn-scoped private scratch path and must delete it after dispatch;
  no public/view receipt projects base64 bytes.
  Mobile coding execution targets come only from the strictly decoded,
  authenticated personal target catalog. The canonical device-local composer
  draft persists the exact lane/provider/model/account/execution-target refs;
  a missing, stale, revoked, offline, or unadvertised selection preserves the
  draft and withholds Send. A new turn carries that exact target, while an
  already-running turn remains pinned to its confirmed lane. Neither path may
  silently substitute another provider or account.
- Native clients create one immutable device-local identity before OpenAuth.
  Its `scope.device_local.*` rows live in separate `local_entities` tables with
  `LocalRevision`; they are local Source Authority and are never readable as
  server-confirmed rows, assigned `SyncVersion`, or sent through hosted Sync.
  A server-verified account link is additive and reversible: link/unlink never
  rewrites the local identity or deletes local rows. Revocation still purges or
  hides the revoked owner's server cache, not the device-local authority.
- Native provider-neutral timelines use the shared client reader over the
  existing `agent_run` / `agent_run_event` entities on one exact agent-run
  scope. Only live confirmed run state and the newest 500 ordered event facts
  may project; owner/objective/repository/runtime/backend, event source, raw
  payload JSON, external callback refs, and non-live cached rows stay hidden.
- Desktop Runtime Gateway is the only renderer path to conversation and agent-
  timeline and provider-native history capabilities. Protocol v7 retains v4's
  bounded confirmed catalog/thread/history queries and exact-run timeline, and
  adds the deterministic canonical runtime-turn path used by #8676 plus an
  exact `intentId`/thread command-outcome query. Enqueue returns
  `pending_reconcile`, never completion. Runtime commands use the existing
  durable Sync mutation ledger and semantic control-intent identity: exact
  retries cannot repeat an effect, conflicting same-ID bytes cannot mutate
  state, and a server-clock-expired intent is durably projected as `expired`
  while remaining ineligible for runtime dispatch. The real named-account/
  physical-phone #8676 receipt remains open. Timeline attachment uses only the
  confirmed `agent_run.routeId` returned as `routeRef`—clients do not derive it
  from `runRef`. Owner/private/raw-provider/auth/store/session/transport fields
  and generic IPC remain unrepresentable in the contract.
- Desktop Runtime Gateway operation correlation is public-safe and ref-only:
  `operationRef`, Desktop lifecycle `sessionRef`, `correlationRef`, and optional
  `runRef` use the bounded public-ref grammar, survive schema decoding through
  preload/main/gateway responses, and enter runtime Sync only as private
  causality refs. Paths, URLs, prompts, bodies, owner fields, credentials,
  provider payloads, native handles, and raw errors are forbidden. The
  process host owns replaceable runtime/workspace/Sync/account/history slots;
  WorkContext/session/window replacement and app teardown close each owned
  finalizer once, abort in-flight native sign-in, and leave zero active slots.
  Regression and built-host coverage lives in
  `apps/openagents-desktop/src/desktop-host-lifecycle.test.ts`,
  `apps/openagents-desktop/src/desktop-operation-context.test.ts`,
  `apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts`, and the normal
  Desktop Electron smoke. Dense-gap, store-version, and native adapter parity
  regressions live in `packages/khala-sync-client/src/session.test.ts`,
  `packages/khala-sync-client/src/sqlite-store.test.ts`, and
  `apps/openagents-desktop/tests/native-timeline-fault-convergence.e2e.test.ts`.
- A Desktop workspace exists only after an explicit directory-picker grant and
  remains one WorkContext-owned main-process capability. The new recursive
  tree/search projections carry the opaque grant ref, relative path refs,
  bounded page/result counts, and a declared cache key/epoch/freshness fact;
  they never carry the selected absolute root, a native handle, or an ambient
  current directory. Hidden, Git-ignored, secret-shaped, binary, unreadable,
  traversal, and symlink-escape entries are withheld. One recursive watcher is
  opened only while subscribers exist; each change, explicit refresh, or
  unlocated/overflow event advances the epoch and invalidates tree/search
  caches. Every bounded search executes in its own WorkContext-owned worker;
  watch/refresh invalidation terminates stale tasks before advancing authority,
  only the unchanged epoch may populate the cache, and caller cancellation,
  worker failure/exit, WorkContext replacement, or app disposal settles each
  task exactly once. Worker results are schema-decoded and never return the
  selected root. Subscriber close, WorkContext replacement, and app disposal
  close the watcher exactly once. The core boundary and adversarial fixtures
  live in
  `apps/openagents-desktop/src/workspace-service.ts` and
  `apps/openagents-desktop/tests/workspace-service.test.ts`; worker lifecycle
  and real built-artifact coverage live in
  `apps/openagents-desktop/src/workspace-search-host.test.ts` and
  `apps/openagents-desktop/tests/build.test.ts`. Fixed tree,
  refresh, subscribe/unsubscribe, and decoded change-event channels now cross
  main/preload only for the trusted top-level bundled renderer. Preload
  reference-counts local consumers; main keeps one exact subscription per
  webContents, rebinds it after an explicit WorkContext replacement, and
  closes it with the window/app lifecycle. The built Electron smoke proves a
  relative tree page, a newer refresh event, unsubscribe, and zero active host
  slots. Fixed decoded search/start-cancel operations also cross only that
  bridge. Main owns at most one active search per webContents: replacement
  cancels the prior task, an exact request ref cannot cancel another window or
  request, and window/app teardown closes the owner. The built smoke proves a
  real relative-ref worker result at the refreshed epoch plus fail-closed
  foreign cancellation. Root-private create-file/create-directory,
  revision-bound rename, revision-bound non-recursive delete, and host-injected
  reveal operations accept and return only relative refs. Hidden, secret,
  Git-ignored, traversal, symlink, stale-revision, existing-target, non-empty-
  directory, and permission-loss cases fail with typed outcomes; only confirmed
  mutations advance the WorkContext epoch. Their adversarial core fixtures live
  in `apps/openagents-desktop/tests/workspace-service.test.ts`. Fixed decoded
  create/rename/delete/reveal main-preload operations now cross only for the
  trusted top-level bundled renderer; Electron main injects reveal authority
  into each selected WorkContext and no absolute path returns. Desktop Files
  composes the accessible relative tree/search/mutation surface; preload reduces
  native-picker completion to a boolean and exposes none of the legacy root
  summary/list/read/save/Git-diff methods. The real Electron smoke clicks Files,
  proves no selected-root text or legacy editor, and tears down with zero active
  owners. CUT-18 document open/save/save-as core operations echo the exact workspace
  grant plus a relative path ref, return only bounded UTF-8 content and a
  content revision, classify invalid/missing/directory/binary/large/encoding/
  permission/revoked outcomes, and atomically refuse stale revisions. Save As
  creates with exclusive-create semantics and never overwrites an existing
  target, including a target that appears during the write race. Confirmed
  workspace renames retarget matching open file and descendant tabs without
  dropping drafts. Editor recovery persists only bounded relative path refs,
  revisions, and drafts under an opaque coding-session ref; it persists no root
  or grant, reopens through the current grant, and surfaces changed or missing
  files as explicit conflicts before any save. These paths never return the
  selected root or accept ambient/absolute renderer paths.
- Desktop Git review requests are closed typed operations over the current
  canonical WorkContext repository. Status projects only relative paths plus
  opaque repository, HEAD, and exact status snapshot refs. Diff and discard
  requests must echo the matching repository/status refs; any concurrent HEAD,
  index, or tracked-worktree change makes the request stale. Diff projection is
  bounded to 120 KB and rejects binary, secret-shaped, and unavailable output
  before renderer or provider context. Discard is confirmation-gated, applies
  only to an unstaged tracked non-conflicted path, and uses fixed
  `git restore --worktree -- <relative-path>` semantics—never reset, checkout,
  untracked deletion, arbitrary argv, or automated commit/push/PR publication.
  Composer review context is one explicit removable next-turn attachment,
  labeled untrusted before provider delivery and cleared on accepted submit;
  receipts never contain its path or diff content.
- Provider-native Codex history remains owner-local and read-only. Desktop main
  indexes active and archived rollouts off the main thread and Runtime Gateway
  v4 projects only bounded catalog/page data: stable thread relationships,
  source-order typed items, explicit redactions, and explicit gaps. Raw JSONL,
  rollout paths, encrypted/raw reasoning, credentials, and filesystem/provider
  authority never cross preload; local history is never uploaded to Khala Sync
  by default. For every decoded thread, source records equal rendered source
  records plus wholly redacted records plus explicit gap records.
- Windowed history prepends preserve the first visible keyed row synchronously
  inside the shared DOM commit. Variable-height rows added above the viewport
  may not produce an intermediate stale `scrollTop` frame or a delayed
  post-paint correction; ordinary appends and unrelated rerenders retain their
  existing numeric scroll position.
- Desktop selects its chat authority once at renderer boot: confirmed account-
  linked Sync when the current gateway catalog is live, otherwise explicit
  local-only mode.
- The first ProductSpec-native Desktop MVP composer is fixed to the ordinary
  logged-in Codex session. It exposes no provider/account, model,
  reasoning-effort, image-attachment, plugin, MCP, or voice selector. Internal
  post-MVP provider/media substrates do not authorize visible affordances and
  may never silently substitute another provider for Codex.
- The MVP visible-surface allowlist is the exact owner-approved User
  Experience: chat/session navigation, repository grant/session home, bounded
  files/review, typed question/approval/plan controls, Open in Codex, commands,
  update/rollback, diagnostics, and keyboard settings. ProductSpec and
  AssuranceSpec remain internal authoring/verification tooling and have no
  user-facing route, screen, dock item, command, or native-menu destination.
  Fleet, OpenAgents/Pylon account controls, Terminal, Inbox, MCP, plugins,
  provider/model/reasoning selection, attachments, and voice are absent from
  dock, sidebar, composer, Settings, command palette, and native Commands menu.
- AUDIO-0 #8733 is a planned, not-yet-live exception for the future Mic path.
  When AUDIO-4 #8737 lands, native capture/playback may run only in the signed
  process-opaque `crates/oa-desktop-audio` Rust helper authorized by the
  Effect/Rust audio decision. `packages/audio-contract` Effect Schema remains
  canonical; Electron main supervises a closed public-safe control protocol;
  raw media and the direct authenticated media socket stay inside the helper;
  renderer, preload, Runtime Gateway events, Khala Sync, command authority,
  storage policy, Google adapters, and UI remain Effect/TypeScript. This bullet
  does not make Mic visible in the MVP: AUDIO work retains its own acceptance
  gates and cannot expand the ProductSpec surface allowlist.

### Persistent Voice and Raw Media

- A voice generation starts only after an explicit, versioned disclosure; stop,
  revoke, and restart fail closed, and restart never silently resumes capture.
- Capture, network egress, retention, and playback are separate authoritative
  facts. Mute synchronously stops capture and egress; it never implies deletion
  or retention, and no preference alone enables retention.
- Every frame is fenced by exact owner, device, thread, session, generation,
  and monotonic sequence. ACKs never regress or acknowledge unsent data.
- Reconnect replay is delivery-only. It cannot rerun ASR, inference, writes,
  final publication, proposals, confirmation, execution, or outcomes.
- Raw audio retention requires a matching unexpired disclosure/policy receipt
  for the exact generation. A prior-generation or cross-session receipt fails.
- ASR hypotheses, transcripts, assistant/model prose, and TTS are display or
  delivery data, never command proposal, confirmation, execution, or outcome
  authority. Only typed actions and durable outcome refs carry that truth.
- Raw media never enters Runtime Gateway projections, Khala Sync, logs,
  analytics, traces, or support bundles.
- Rust may own native capture/playback and the bounded media envelope only. It
  owns no transcript, command, Sync, storage, retention-policy, or outcome
  schema; the Effect audio contract is canonical.
- The executable regressions live in
  `packages/audio-contract/src/lifecycle-model.test.ts`, with shared
  Effect/Rust accept/reject vectors in `fixtures/audio-contract/media-v1.json`.
- Desktop conversation navigation is globally ordered by descending
  `updatedAt` across hosted and app-local threads, never grouped by source.
  The converging host owns the canonical merge order and renderer hydration
  defensively reapplies it, with deterministic thread-ref tie-breaking.
- Desktop local-lane transcript notes preserve provider event arrival order
  both while streaming and after durable reload. Consecutive assistant text
  deltas may coalesce only until the next display-bearing non-text event; that
  event closes the assistant segment, and later text starts a new segment
  after it. Tool results update the matching invocation card in place at the
  invocation's original position rather than moving the card or surrounding
  assistant text. Persisted model, reasoning, lane, tool, and assistant notes
  must reopen in the same relative sequence the user saw live.
- An accepted Desktop-local provider turn is durable before provider dispatch.
  Electron main owns a private mode-0600 journal keyed by exact thread, turn,
  and lane; it records the selected account, provider session identity,
  lifecycle phase, bounded assistant segments/cursor, recovery generation, and
  one terminal disposition. Checkpoint writes are cadence-bounded and atomic,
  and deterministic message keys preserve live text/tool ordering without
  duplicate prompt or assistant segments after reload. Startup reconciles each
  nonterminal record once. Codex may issue one continuation on the exact
  recorded account/thread and marks `resumed_after_restart`; this is semantic
  same-thread continuation, not byte-level attachment to the dead process's
  stream. The current Claude Agent SDK cannot reattach an interrupted query,
  so Fable records `interrupted_by_restart` and requires an explicit retry
  instead of silently replaying the prompt. Renderer process state is never
  recovery authority, and restart never auto-starts microphone capture.
- Desktop top-level local Codex turns have no automatic wall-clock deadline.
  Long or temporarily quiet coding work continues until Codex completes,
  fails, or the owner explicitly uses Stop; elapsed time alone must never send
  SIGTERM or manufacture a provider-unavailable failure. A host deadline may
  be injected only by bounded tests and is not production configuration.
- Desktop captures its process working directory at launch and uses that exact
  directory as the default coding cwd for top-level local Codex and Claude
  turns. It must not silently replace that cwd with an Application Support
  per-thread directory. The runtime accepts the cwd through a host getter so a
  future explicit directory setting can replace the launch default; provider
  probes, account custody, and delegated scratch workers remain isolated.
- Desktop's mixed runtime/provider conversation sidebar has one canonical
  target order for rendering, Command/Ctrl+1–9 hints, and keyboard activation;
  numbering never restarts at a source boundary. Selecting a runtime/app-local
  row always unmounts any provider-history page before projecting that row's
  transcript, so a successful selection cannot leave stale chat content visible.
- Desktop's application, component, state, projection, and typed-intent model
  remain Effect Native. React 19 owns the renderer root, lifecycle, synchronous
  snapshot consumption, and declared ordinary-element lowerings through the
  shared `@effect-native/render-dom/react` renderer. The bounded Desktop MVP
  transition may also define ordinary renderer-private React workbench
  components only in the explicitly scanned `renderer/react-primitive-adapters.tsx`,
  `renderer/react-timeline.tsx`, `renderer/react-composer.tsx`, and
  `renderer/react-review.tsx` hosts; they consume the same Effect-owned
  `DesktopShellState` snapshot and existing intent keys and may retain only
  ephemeral focus/overlay/scroll-anchor/IME/palette-query mechanics. They do not define a
  second domain store, runtime client, command identity, persistence path,
  token system, or Vercel AI SDK/model-stream authority. One authoritative
  surface selects exactly one whole-surface backend for its lifetime: declared
  React lowerings or the proven direct catalog compatibility backend, never
  both. Desktop selects React for an ordinary launch; the catalog backend is
  reachable only through the explicit `renderer=compatibility` fallback for
  retained specialist surfaces and their acceptance oracle. The Effect-owned
  stream opens once outside React effects; Strict Mode
  replay may reattach React listeners but cannot duplicate the host
  subscription, command effect, or terminal outcome. Unmount closes the
  selected backend, subscription, root, and token stylesheet. Tailwind and any
  Base UI adoption are renderer-private implementation tools. The owner-picked
  shadcn preset `b3Zg9L0M8A` (`base-vega`, zinc/blue, Oxanium/Geist, small
  radius, Lucide) is the preferred source-component layer when it supplies an
  appropriate control. Generated components stay under Desktop `src/components/ui`,
  while `shadcn-khala.css` maps every semantic palette role onto canonical
  `--en-*` variables; the preset may not install a parallel light/dark palette.
  Tailwind defaults remain disabled rather than becoming a second theme.
  Portable renderer modules remain
  `.ts`, React-free, and free of `className`/`ReactNode`; no Zustand, Effect
  Atom React, TanStack router, Zod, arbitrary JSX component, or second theme/
  icon system may become application authority. The Electron renderer remains
  tokenless and Node-free. This boundary and `.tsx` scanner coverage are
  enforced by `apps/openagents-desktop/tests/electron-boundary.test.ts` and
  `apps/openagents-desktop/src/renderer/design-conformance.test.ts`.
- Desktop React repository review consumes only grant-scoped, root-relative
  paths plus opaque repository/status correlation already held by the Effect
  Git state. It remains read-only: no edit, discard, stage, commit, branch,
  push, PR, terminal, arbitrary Git argv, absolute root, or raw diff fallback.
  Review refusals and runtime recovery banners project typed Effect
  dispositions; React never classifies raw provider/runtime error strings.
- The MVP sidebar never projects connected provider accounts or usage. Any
  retained account/Fleet state is non-visible post-MVP substrate only.
- Desktop chat context rails are genuinely pointer-resizable from 280–480px.
  The shared SplitPane renderer targets the explicitly sized pane adjacent to
  a divider (including a trailing/right pane), and the renderer-owned typed
  resize intent persists that width across rerenders.
- Opening message details below a taller live-agent graph makes the exact right
  rail its own overflow owner and synchronously reveals the selected message's
  unique keyed inspector marker after generic scroll restoration. This is a
  one-shot reveal per changed selection: it never moves the transcript scroll
  owner and never continuously re-pins against later manual reader scrolling.
- Desktop New Chat is never a silent no-op. The dock action, command palette,
  and platform Command-N chord dispatch one typed `DesktopNewChat` intent. A
  new thread is created first through the app-owned durable local store and
  pinned to local authority; live Sync pending reconciliation is never on the
  New Chat critical path. Runtime creation is fallback-only when the local
  bridge cannot create. Only a real thread may clear the loaded history page;
  success always mounts an empty transcript and focuses the composer. This is
  enforced by the `new_chat_always_exits_history` UX contract,
  converging-host unit coverage, and both built-Electron input paths.
- Desktop provider runtimes are package-owned, version-explicit capabilities.
  Codex launches the native executable resolved from the exact pinned
  `@openai/codex` optional platform package, never an ambient PATH executable;
  Claude execution imports the exact pinned Agent SDK. The compatibility
  catalog fails closed unless observed versions equal the bundled/tested lock
  versions, and reports missing, malformed, or unverified updates without
  paths, raw output, credentials, or provider-home data. Version probing does
  not read or mutate default `~/.codex` or `~/.claude` homes. Provider turns
  use account-scoped isolated-home environments except the Claude composer's
  explicit current-session lane, which delegates ordinary `~/.claude` auth
  resolution to the pinned SDK without reading or mutating credential data. An
  explicit Desktop provider target is the exact provider, bundled model, and
  named account ref for that turn: the channel rejects cross-lane or unknown
  model pairings, and the runtime must fail on that account rather than rotate
  or silently substitute another account. An omitted target alone preserves
  automatic health-ordered account selection and visible rotation. For the
  Claude composer, the omitted/default target prefers the currently
  authenticated local Claude Code session and uses isolated Pylon Claude homes
  only as fallback; explicitly selecting a named Pylon account pins it.
  The Codex composer MVP uses only the ordinary authenticated local Codex
  session and exposes no named-Pylon linking, rotation, or isolated-home
  fallback. Current-session launches must clear an inherited `CODEX_HOME` so
  stale Pylon selection cannot override `~/.codex`. Desktop smoke, proof,
  fixture-path, startup-trace, and isolated-profile environment controls are
  host-only and must be removed from every Codex provider subprocess; a proof
  turn must see the same provider environment as an ordinary user turn.
  ProductSpec skills and dynamic tools are advertised to Codex only while the
  host owns an admitted ProductSpec work context; an ordinary coding chat must
  not inherit proposal workflow instructions or tools.
  Delegated Fleet children
  may retain explicit named-account custody under their separate fleet
  contracts, but that authority cannot enter the local workroom lane.
  A delegated child is a conversation, not merely graph metadata: its bounded
  exact prompt and final response must remain attached to the child card and
  appear in the selected-agent rail. Selecting a child must never degrade to a
  status-only inspector while its transcript is available.
  User-local Claude plugins are host-owned capabilities: absolute plugin paths
  remain in the owner-only main-process registry, while preload/renderer expose
  only opaque plugin refs plus bounded provenance, scope, readiness, enablement,
  restart, next-turn-use, and capability metadata. Only enabled directories
  with a valid `.claude-plugin/plugin.json` may enter the pinned Agent SDK
  options; missing, invalid, duplicate, unknown-ref, and disabled entries fail
  closed and cannot be substituted by a provider-default plugin source.
  Skills never auto-route from prose or keywords. Only the modeled leading
  `/skill <plugin>/<skill> <prompt>` grammar can select a skill already present
  in the typed enabled-plugin catalog; main re-resolves its opaque plugin ref
  and exact `skills/<name>/SKILL.md` before enabling the SDK Skill tool for
  that turn. Malformed, missing, disabled, duplicate-name, empty-prompt, and
  stale selections fail closed without running an unskilled substitute turn.
  Local permission posture is explicit per conversation. Fable defaults to
  `owner_full` (the established default-mode allow-all tool posture with the
  real question flow) and may be narrowed to SDK-enforced `plan_only`, which
  cannot execute tools. Codex remains honestly full-execution-only until its
  bundled runtime exposes an equivalent plan authority; a plan-only request on
  that lane fails closed. No stored or renderer-supplied mode can select SDK
  `bypassPermissions` or broaden authority beyond the owner-full default.
  It never merges the two catalogs in one renderer lifetime. Sync-mode create/
  append waits for the exact generated ref to appear confirmed; timeout remains
  pending and is never converted into success.
- Native live conversation reconciliation is driven by the shared Sync
  content/state notifications and durable scope cursor, never by transcript
  interval polling. Each subscription carries an exact subscription ref,
  generation, ordered local sequence, thread/run/message/event correlation
  refs, and `provisional | confirmed | interrupted` delivery. Resume gaps
  collapse to one bounded authoritative snapshot; a future cursor interrupts
  fail-closed. Slow consumers retain at most one newest pending snapshot, and
  expose source/delivery/coalescing/maximum-pending/latency metrics; close
  removes both observers and the owned thread scope exactly once. The
  shared contract and mobile no-poll boundary are enforced by
  `packages/khala-sync-client/src/live-conversation.test.ts` and
  `apps/openagents-mobile/tests/mobile-conversation.test.ts`. The Desktop host
  registry additionally bounds active slots, closes a prior generation before
  replacement, fences stale unsubscribe, and disposes all slots once. Runtime
  Gateway v8 now schema-decodes the full bounded update and exposes typed
  subscribe/resume/unsubscribe outcomes through the existing event channel;
  main resets the registry before Sync authority replacement or sign-out and
  gateway disposal closes it. The Desktop runtime chat consumer registers that
  decoded listener before append, uses one fenced subscription through exact-
  message and terminal confirmation, and closes with one exact unsubscribe.
  It retains only one-shot queries for initial catalog/detail or final timeout
  diagnosis; no recurring 100 ms timeline loop remains. This boundary is
  enforced by `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`.

**Planned live-agent portability model boundary:**

- `openagents.live_agent_graph.v1` is now the registered provider-neutral
  graph schema boundary. It gives every node stable session/thread/transcript/
  run/agent refs, explicit root/agent/unknown parentage, known-or-unknown
  provider/runtime/worktree/tool facts, status/attention/terminal state,
  attachment generation, per-agent activity cursor, and monotonic version.
  Parent/tool edges are stable typed records. Its reducer accepts exact replay
  idempotently and rejects cursor gaps, stale/future attachment generations,
  identity/version/cursor/timestamp regression, terminal reopening, missing or
  conflicting parents, orphan edges, and cycles. The schema/reducer is
  enforced by `packages/agent-runtime-schema/src/live-agent-graph.test.ts`.
  Provider-specific Codex app-server and Claude Agent SDK observation adapters
  now exhaustively map their distinct status/tool vocabulary into equivalent
  graph facts and loss-account omitted facts explicitly. Their canonical
  output now has a registered Khala Sync full-post-image entity boundary:
  one validated `live_agent_graph` per canonical
  `scope.thread.<threadRef>`, keyed by stable `graphRef` and advanced only
  through the shared exact-cursor reducer. The top-level session and canonical
  thread refs stay distinct; provider-native node thread refs cannot choose
  Sync authorization scope. A named fail-soft server projector now validates,
  structurally redacts, and appends this post-image through the normal dense-
  version transaction writer. Live provider call-site binding is now active for
  the existing server-authoritative Codex/Claude runtime transaction: start/
  control/event transitions append the root graph atomically,
  provider identity stays explicit unknown until a real event source supplies
  it, and a retry after terminal advances attachment generation instead of
  reopening the terminal node. The confirmed client read model accepts graph-
  valid post-images only from the exact live thread scope, caps one snapshot at
  eight graphs / 2,000 nodes / 4,000 edges, and emits matching graph refs
  through Runtime Gateway protocol v8 on the existing cursor-aware
  subscription. Exact resume and one bounded authoritative refetch use the
  durable Sync cursor; non-live scopes expose no cached graph authority. The
  shared runtime contract now also carries body-free `agent.child.*` events.
  Real Claude Agent SDK subagent task messages populate stable child nodes and
  parent edges in the same transaction, preserve them through root settlement,
  and discard them only when retry advances attachment generation. The
  installed Codex SDK 0.139.0 public union exposes no typed child event, so
  Codex live child production stays explicitly unsupported rather than inferred
  from tool text. Redacted named runs prove the Claude lifecycle and the current
  Codex app-server's typed `subAgentActivity` source; Pylon transport convergence
  and a named confirmed-reconnect receipt remain CUT-11 work.
  The shared client presentation model now converts canonical post-images into
  deterministic hierarchy rows with typed action/attention/elapsed/terminal
  facts, explicit unavailable facts, historical control refusal, stable focus
  fallback, and a named large-graph remainder. Khala Mobile reads only the exact
  thread-scope `live_agent_graph`, renders at most 40 rows, exposes the remainder
  count, and uses ordinary accessible button/selection semantics for inline
  inspection. OpenAgents Desktop projects the same confirmed Gateway v8 graph
  through that shared model, hydrates through one bounded subscription rather
  than a timeline/graph poller, and dispatches pointer, keyboard, and screen-
  reader inspect/focus actions through one schema-checked intent carrying the
  stable agent ref. Desktop-local Claude/Fable and Codex turns consume their
  schema-decoded preload graph snapshot plus push stream through the same
  presentation model, fence stale cursors, retain the graph on the matching
  local thread, and render it in the chat's right context rail. Historical
  graphs remain inspection-only on both clients.
  The greenfield OpenAgents Mobile app now carries the same boundary: its Sync
  host exposes the confirmed thread-scope graph reader, the conversation
  adapter forwards `live_agent_graph` post-images into the thread snapshot, and
  the Effect Native surface mounts one accessible agent stack above the
  transcript with attention auto-open, tap select/inspect of the exact typed
  agent ref, deterministic replacement fallback, a named 40-row remainder, and
  no runtime-control intent reachable from a graph row. Per-node token
  attribution presents only typed truth: a node is `exact` solely when every
  recorded attribution carries a complete well-formed usage split, a mix names
  the exact recorded total plus the unreported turn count, and everything else
  is loss-accounted `Unreported` — token facts are never synthesized from the
  canonical graph, which deliberately carries no usage fields.
  This CUT-12 client boundary is enforced by
  `packages/khala-sync-client/src/live-agent-graph-presentation.test.ts`,
  `clients/khala-mobile/tests/live-agent-graph-panel.test.tsx`,
  `apps/openagents-mobile/tests/mobile-agent-graph.test.ts`,
  `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`, and
  `apps/openagents-desktop/src/renderer/runtime-agent-graph.test.ts`; physical-
  device equivalence remains pending.

- CUT-13 canonical coding identities are durable product refs, never host,
  filesystem-path, process, provider-session, credential, or transport
  identities. `coding_project`, `coding_repository`, `coding_worktree`,
  `coding_session`, and `coding_navigation` post-images carry an exact
  authority scope and explicit availability/grant facts. Hosted authority is
  only `scope.user.*` or `scope.team.*`; signed-out Desktop rows are explicitly
  `scope.device_local.*`, remain in `local_entities`, and are never submitted
  to hosted projection. Their raw filesystem binding stays in a separate
  owner-private main-process file and never enters the post-image or renderer.
  Restore validates all
  project/repository/worktree/session relationships before returning ready;
  ambiguous aliases, owner mismatch, missing work, archive, revoked grants,
  and unprojected grant truth produce typed recovery. Opaque aliases may resolve
  to a canonical ref but never become authority. Duplicate tabs collapse to
  one canonical session; typed conversation/editor/terminal/agent focus is
  preserved. Catalog text retrieval is not implemented with string matching;
  only bounded structured filters are allowed at this layer. The shared schema,
  structural redaction, 64-state fail-closed model, and restart resolver are
  enforced by `packages/khala-sync/src/coding-session.test.ts`. Server
  projection accepts only one bounded same-owner relationship-valid bundle,
  rejects private-shaped material before storage, and appends sequentially
  through one transaction so every changed entity shares one dense scope
  version; its unit and real-Postgres oracles live in
  `packages/khala-sync-server/src/coding-session-projection.test.ts`.
  Confirmed client reads validate entity-id/owner agreement, apply explicit
  aggregate bounds, and expose no cached catalog until the exact owner scope is
  live; malformed and cross-owner rows are ignored and the shared resolver
  revalidates the resulting relationship graph. This is enforced by
  `packages/khala-sync-client/src/coding-session.test.ts`. Desktop persistence
  now uses the same schemas through the local-authority store; process restart,
  duplicate open, typed focus/route restore, structured query, archive, missing-
  worktree recovery, IPC redaction, and built renderer reload are enforced by
  `apps/openagents-desktop/tests/desktop-coding-catalog.test.ts`,
  `apps/openagents-desktop/src/renderer/shell.test.ts`, and the normal Electron
  smoke journey.

- Master Roadmap and
  `docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md`
  define target contracts for a canonical live agent graph, graph-wide
  attachment fencing, portable per-child transcript/activity cursors, replay
  repair, and one typed click/tap/hotkey action path. PORT-00 #8745 freezes the
  versioned public-safe schemas, bounded cross-record invariant audit, and
  real-host journey in `packages/portable-session-contract`; its enforced
  behavior contract is
  `openagents_apps.portable_session_contract_freeze.v1`. This is schema/model
  authority only. PORT-01 supplies persistence and PORT-02 supplies broker
  redemption plus the owner-local/OpenAgents-managed adapter seam. Dispatch,
  target enrollment, movement, mobile control, and owner acceptance remain
  pending #8748–#8753.
- Current provider-native Codex topology remains owner-local, read-only, and
  loss-accounted. The landed inline child card is a bounded history projection;
  it does not prove live Khala Sync topology, host movement, child rehydration,
  or graph-wide fencing.
- The PORT-00 schema/model boundary rejects host/path-shaped refs, non-owner-
  minted identity, root-catalog child leakage, missing/cyclic parent edges,
  duplicate or simultaneous live attachment generations, incomplete
  descendant fences, mismatched checkpoint generations, non-excluded secret/
  process state, lease-scope mismatch, stale movement commands, missing
  destinations, and silent target changes. Production implementation must
  decode these schemas and run equivalent pre-mutation checks; passing the
  package audit alone never grants execution authority.
- Before any later portable/live-agent implementation claims enforcement, its
  bounded issue must register the production authority, update this ledger and
  the relevant app/Worker ledger, add deterministic fault tests for replay and
  repair, then add the required real-host/physical-device receipt. No client or
  runtime may infer topology from prose, upload raw local history, or describe
  #8674/#8675 as portability proof.
- PORT-01 #8746 makes Cloud SQL the durable portable-session authority through
  `packages/khala-sync-server/src/portable-session-authority.ts` and migration
  `0066`. Owner-minted session identity, the complete parent/child graph,
  per-thread cursors, authorized target membership, attachment generation,
  secret-free checkpoint refs/digests, event log, repairable current rows, and
  command outcomes commit transactionally. The partial unique index permits at
  most one work-accepting attachment; every event and command checks the exact
  attachment generation; movement fences the complete descendant set before
  advancing it. Exact command/completion retries reconcile without executing
  twice, conflicting identity reuse fails closed, and an owner retention purge
  cascades authority rows while appending Sync tombstones. The hub/stream is
  acceleration only: restart, stream gaps, or `MustRefetch` rebuild from Cloud
  SQL, never from a socket or current projection. The real-Postgres oracle is
  `portable-session-authority.test.ts`. This does not grant target credentials
  or execute a host move; those remain PORT-02/PORT-03.
- PORT-02 #8747 implements the general target-scoped capability broker in
  `packages/portable-session-contract/src/capability-broker.ts` without
  changing PORT-00's frozen lease schema or PORT-01's durable authority.
  Provider, SCM read/write, MCP/tool, and bounded API leases bind one owner,
  session, attachment generation, target, capability, optional account/tool,
  least-privilege permission set, and bounded TTL. Issue, redeem, renew,
  revoke, reissue, release, and wipe are exact-operation-ref idempotent;
  conflicting replay fails closed. Source revoke and target wipe must complete
  before a destination lease is issued, and reissue requires a fresh
  destination source-grant ref. Raw material is visible only inside the
  injected vault-to-target callback and cannot enter broker state, Sync,
  checkpoints, prompts, logs, snapshots, diagnostics, artifacts, or public
  receipts. Expiry, target denial, broker outage, mid-move revocation, and wipe
  failure retain durable refs-only evidence and never grant destination
  authority. Focused enforcement is
  `packages/portable-session-contract/src/capability-broker.test.ts`. This is a
  real local/managed adapter integration seam, not proof that a process or
  session moved; PORT-03 owns that real-host movement receipt.
- PORT-03 #8748 composes those two authorities in
  `packages/khala-sync-server/src/portable-session-move.ts`. A movement command
  requires migration `0067`'s owner/session execution binding and preserves
  the exact canonical run, repository, and pinned-base refs through checkpoint,
  stage, activation, and replay; a legacy unbound row remains readable but
  cannot move, and no host path may synthesize the binding. A movement command
  is admitted against the exact current attachment generation; the runtime
  quiesces every canonical descendant; PORT-01 persists the graph-wide fence;
  and only then may the destination stage and verify the checkpoint while
  `acceptingWork:false`, with every planned fresh destination lease ref bound
  to that exact staged resource. Only after that stage succeeds may PORT-02
  revoke/wipe each source lease and issue/redeem its destination-generation
  lease into the staged resource. Stage failure has zero broker effects; a
  later broker failure aborts the staged destination and releases every
  attempted destination lease. The destination cannot activate until the
  PORT-01 completion transaction has independently required the exact durable
  event cursor, recomputed the complete canonical graph digest, detached the
  source, and advanced the sole live attachment generation. Source cleanup
  must cover every agent plus process, scratch, and port state before that
  commit. Pre-commit failure leaves the source quiesced and the session in
  `recovery_required`; newly issued destination leases are released and a
  refs-only typed outcome is durable. A lost activation acknowledgement
  reconciles by replaying one stable activation operation after the already-
  completed durable command, never by running the move or accepting a parent/
  child turn twice. The real-Postgres fault oracle is
  `portable-session-move.test.ts`. Migration `0069` and
  `portable-capability-broker-store.ts` additionally make the broker's complete
  refs-only state, exact operation evidence, revision CAS, and one active move
  claim a single Postgres transaction. Every write locks and revalidates the
  exact owner/session/move/command/source-generation/destination claim; a stale
  revision or competing claim writes neither state nor evidence, and raw
  credential or host material is rejected before the transaction. The split
  state/evidence sink remains a deterministic-test compatibility seam only;
  production portable movement must use the atomic store. The owner-local
  Pylon source adapter binds that same exact session/attachment/generation to
  the canonical root/child control sessions. Quiescence rejects new replies,
  aborts and joins every bound executor, and retains the one workspace until a
  secret-free repository/diff/graph checkpoint is durably stored in the local
  SQLite operation ledger. Ordinary cancellation continues to retain dirty
  workspaces; only portable cleanup carrying the exact checkpoint ref and
  SHA-256 digest may reclaim a dirty Pylon-owned lease and record its cleaned
  state. Checkpoint and cleanup replay after SQLite reopen do not repeat
  process or workspace effects. This source-side boundary is enforced by
  `apps/pylon/tests/portable-session-target.test.ts` and
  `apps/pylon/tests/portable-session-operation-ledger.test.ts`. The binding
  itself is also private SQLite authority, not process memory: it stores the
  exact root/child parent edges, control-session refs, workspace refs, process/
  workspace lifecycle, and owning runtime epoch without storing a local path.
  A new Pylon epoch recovers an unfinished binding only as `quiesced`, marks
  the prior process epoch absent, and returns one typed refs-only recovery
  outcome. Every old epoch rechecks SQLite before accepting a reply, so a
  process that outlives restart cannot accept another parent or child turn.
  Same-byte recovery replays; a conflicting recovery ref or attachment
  generation fails closed. This restart fence is enforced by
  `apps/pylon/tests/portable-control-session-recovery.test.ts`. Managed → local
  failback stages through `apps/pylon/src/portable-session-destination.ts`.
  Before any restore effect it requires the exact active remote attachment from
  PORT-01, then verifies the repository post-image, binary diff, complete
  root/child graph, per-thread cursors, and fresh destination capability refs
  while the local destination remains non-accepting. It may activate only after
  PORT-01 reports the exact checkpoint-bound local attachment at the next
  generation. Its SQLite replay cannot reopen the old local generation; a
  staged abort must prove every agent and capability plus process, scratch, and
  port state released. Private paths and checkpoint bytes remain behind the
  local rehydrator and never enter ledger outcomes. This destination boundary
  is enforced by `apps/pylon/tests/portable-session-destination.test.ts`.
  The retained managed target prepares a nonaccepting Firecracker resource
  before receiving private checkpoint bytes. Its authenticated materializer
  derives that resource from target plus session, validates an exact
  digest-bound tar.zst containing only the Git bundle, manifest, and sorted
  post-image, and permits symbolic links only when their declared relative
  target has no empty/dot/dotdot/backslash component and resolves inside the
  repository. Resolver/upload/verification failure must run the same scoped
  `abortPrepared` compensation; teardown is durably marked pending before the
  VM effect, and replay treats an already-missing VM as successful cleanup.
  Activation alone accepts no fabricated work. The private continuation route
  requires exactly one unique turn for the canonical root and every child, an
  installed stage-planned provider lease, and one real bounded `oa-workroomd
codex session` execution per agent. Only agent/turn refs, monotonic thread
  cursors, evidence refs, and material exclusion may enter host or guest
  journals; same-operation replay cannot execute a second turn. Focused
  enforcement lives in `apps/pylon/tests/portable-session-control.test.ts` and
  `crates/oa-codex-control/tests/portable_managed_agent_computer_contract.rs`.
  The server does not treat that host acknowledgement as canonical activity.
  `PostgresPortableManagedContinuationAuthority` re-locks the exact owner,
  active attachment, generation, complete parent/child graph, agent activity
  cursors, and thread cursors in one transaction. For every stable agent/turn
  pair it appends an exact next `running` event and then a `waiting` settled
  event, advances that agent's activity cursor, and returns the canonical
  final per-thread cursors. A lost acknowledgement replays those same accepted
  refs, evidence refs, and cursors without another event; a partial operation,
  changed turn/evidence, cursor conflict, or stale attachment generation rolls
  the complete graph transaction back. Real-Postgres enforcement lives in
  `packages/khala-sync-server/src/portable-managed-continuation.test.ts`.
  Continuation is cursor-fenced: every expected agent/thread/activity/event
  cursor must equal retained pre-state, every accepted event cursor advances
  exactly once, and the retained resource commits the returned cursor set
  before acknowledgement. Reverse checkpoint export is allowed only for the
  exact quiesced checkpoint; it returns the same bounded archive contract,
  persists bytes only in a mode-0600 private host artifact store, and provides
  byte-identical replay under one operation ref while journals retain refs and
  digests only.
  These landed
  coordinator, store, and owner-local source pieces are implementation rungs
  only: #8748 remains
  open until #8636 is completed, the store is composed with real target
  adapters, and a direct real
  owner-local Pylon → accepted Agent Computer → owner-local journey proves the
  same refs, exact repository/diff digests, grants, cleanup, and rollback.
- Mobile selects a visible conversation authority after native-session recovery
  and before mounting one Effect Native Home program: confirmed personal Sync
  when live, otherwise the existing public-local conversation. Explicit auth
  transitions dispose and remount the program; catalogs never merge. Sync-mode
  create/append uses stable mobile refs, waits for the exact ref to become
  confirmed, labels drafts pending, and clears account-linked projections on
  denial or sign-out.
- Effect Native React Native stacks are structural layout containers, never
  aggregate accessibility controls. A stack may retain a catalog region/group
  label for diagnostics, but the native View is explicitly excluded from the
  TalkBack/VoiceOver focus order so it cannot collapse or hide interactive
  descendants. Buttons, fields, composer actions, agent rows, and other leaf
  controls remain independently discoverable. This boundary is enforced by
  `apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts` and
  an Android TalkBack Release-host receipt.
- Mobile coding navigation reads the CUT-13 catalog only from the exact
  server-verified personal scope while that scope is live. Signed-out, idle,
  refetch, unavailable, and denied phases expose no cached repository/session
  rows; denial is named as purged authority and other offline phases name the
  cache as withheld. Deep-link and notification payloads contain only bounded
  repository/session/thread refs and are revalidated against the live owner,
  relationships, availability, and grants before use. The selected stable refs
  may persist in device-local `local_entities` for process-death recovery, but
  never confer hosted authority, contain paths/credentials, or cross an owner
  relink. Selection generations close prior thread leases and discard late
  updates before a new scope/session can render. The hosted catalog is produced
  only by the authenticated
  `coding.publishCatalog` Sync mutator: Desktop re-encodes its validated
  device-local catalog with the caller's exact personal scope, never publishes
  the private worktree-path binding, and never treats the optimistic mutation
  as hosted authority. The server rejects any owner-scope mismatch before a
  changelog write. Mobile continues to consume only server-confirmed rows. The
  deterministic enforcement lives in
  `apps/openagents-mobile/tests/mobile-coding-navigation.test.ts` and
  `apps/openagents-mobile/tests/mobile-sync-host.test.ts`. The Effect Native
  drawer groups live sessions under confirmed repositories, and one typed
  session intent reopens the exact thread and binds a closeable conversation
  subscription; new-chat and ordinary-thread navigation close that lease.
  Process restart resolves the persisted target before choosing a conversation
  rather than inferring the first chat row. This integration is enforced by
  `apps/openagents-mobile/tests/authoritative-home.test.ts` and
  `apps/openagents-mobile/tests/mobile-conversation.test.ts`. Initial and live
  native URLs plus initial/live notification responses enter one 16-item
  bounded serial queue, wait while owner authority is unavailable, and pass
  through the same exact target resolver before activation. Stale/unauthorized
  targets are terminal, concurrent flushes coalesce, and host teardown removes
  both native listeners and clears the queue. This is enforced by
  `apps/openagents-mobile/tests/native-coding-target-delivery.test.ts`;
  physical iOS/Android receipts remain CUT-14 work.
- Desktop command metadata has one canonical typed registry at
  `apps/openagents-desktop/src/desktop-command-contract.ts`. Each command names
  its stable id, intent, scope, readiness, authorization, argument/result
  shape, default bindings, and palette visibility; the renderer palette derives
  from it rather than maintaining a second list. User chord aliases normalize
  to one bounded grammar, conflicting chords dispatch nothing until recovered,
  and malformed/unknown overrides are ignored. Overrides persist only in an
  owner-private atomic store beneath Electron `userData`; conflict recovery
  removes or resets overrides rather than guessing precedence. Deferred native
  menu/deep-link/second-instance/restore inputs decode through the closed v1
  envelope and still require current readiness and owner authority. Main owns
  admission, the single-instance lock, bounded pre-ready queuing, and duplicate
  suppression; the renderer can dispatch only the decoded typed intent. The
  isolated built-host second-process receipt enforces this boundary. Release
  signing and distribution remain CUT-26 work.
- Rich coding drafts use `@openagentsinc/composer-state`; apps must not create a
  second editor/attachment state machine. The private
  `openagents.coding_composer_draft.v1` snapshot carries only stable context and
  target refs around the structured document. Submission refuses unfinished
  attachments, stale context, and unavailable/revoked/offline targets. Exact
  duplicate queueing is idempotent, and retry preserves submission/intent/
  idempotency identity. Its bounded receipt never carries prompt, attachment,
  account, path, editor, or diff content. Native restart recovery persists the
  canonical snapshot only under the immutable identity's device-local
  `local_entities` scope, with exact owner/draft binding, bounded size/count,
  and stale/conflict/duplicate outcomes; it never enters hosted Sync. Native
  file/image acquisition is bounded before and after reading bytes, copies the
  selected file into the app document sandbox under its SHA-256 digest, and
  places only `attachment.native-local.sha256.*` metadata in the draft. Picker
  URIs and platform file handles never enter the draft, receipt, transcript,
  or hosted Sync.
- Provider questions, tool approvals, and plan reviews use the private
  `openagents.runtime_interaction.v1` authority and `runtime_interaction` Sync
  entity. New requests must match authenticated owner, exact thread/turn,
  durable provider lane, next event sequence, legal turn state, and a future
  server deadline. Decisions are kind-matched and exact-ref/idempotency-bound;
  exact retry reconciles, conflicting reuse rejects, and late/revoked decisions
  never resolve. Full post-images project only to the exact private thread;
  clients withhold cached rows unless that thread scope is live.
- Native OpenAgents user access/refresh tokens live only in platform credential
  custody: Expo SecureStore on mobile and the Electron main-process OS
  credential boundary on Desktop. Effect Native state receives only typed
  session phases; stored credentials are unverified until the server accepts
  them, and malformed or retired-epoch records are purged fail-closed.
- Desktop native-session custody uses Electron `safeStorage` in main plus one
  opaque encrypted record beneath the private `userData` root. Custody refuses
  unavailable OS encryption and the Linux `basic_text` backend; the directory
  and file are owner-private, replacement is atomic, and preload/renderer/
  Runtime Gateway schemas never carry owner or credential fields.
- Desktop recovered-session validation sends the refresh token only to the
  exact native-session GET, persists a valid OpenAuth rotation before
  projecting `session_ready`, and purges denial or server-derived owner
  mismatch. Transient network/server/schema failure retains encrypted custody
  while projecting unavailable; verified session state is not live Sync.
- Desktop interactive OpenAuth uses the distinct public client
  `openagents-desktop` and only an RFC 8252 loopback redirect shaped exactly as
  `http://127.0.0.1:{ephemeral-port}/auth/callback`. The issuer requires GitHub
  authorization code + PKCE S256, a bounded challenge, a non-privileged port,
  and no userinfo/query/fragment; it rejects localhost, non-loopback, HTTPS,
  custom schemes, missing ports, and mobile-client reuse.
- The Desktop callback listener binds only literal IPv4 loopback on an
  OS-assigned port, accepts only the exact path, method, state, and non-empty
  code, never reflects callback secrets, and closes after one terminal result
  or a bounded timeout. Electron main exchanges the verifier, verifies the
  server owner, saves any immediate rotation, and requires proof of both
  access and refresh revocation before clearing on sign-out.
- Desktop Effect Native Settings receives only the explicit Runtime Gateway
  session phase (`signed_out`, `unverified`, `session_ready`, `denied`, or
  `unavailable`). Its typed sign-in/sign-out intents send no arguments, disable
  while the host action is in flight, and never render callback, owner, or
  credential fields; `session_ready` is not presented as live Sync.
- Mobile recovery may send the refresh token only to
  `GET /api/mobile/auth/session` via the bounded `X-OpenAgents-Refresh-Token`
  header. The existing OpenAuth verifier owns rotation; replacement tokens are
  rewritten to SecureStore before the host projects `session_ready`. Denial or
  server-derived owner mismatch purges the record, while transient failure
  hides shared work without deleting a potentially valid credential.
- Mobile interactive sign-in uses exactly the public client
  `openagents-khala-mobile`, provider GitHub, authorization code, PKCE S256,
  and canonical redirect `openagents://auth`. One imperative AuthRequest owns
  callback state validation and its verifier; an error result is never
  exchanged. Explicit sign-out must obtain a server response proving both
  access and refresh revocation before the local SecureStore record is cleared.
- Operations (Cloud SQL monitoring, migration runner, compaction, capture
  daemon, hub reset, Hyperdrive saturation, secrets locations) are in
  `docs/khala-sync/RUNBOOK.md`.
