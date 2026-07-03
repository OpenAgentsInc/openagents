# Pylon

![Pylon](docs/images/pylon.png)

## Tech stack

- [Bun](https://bun.sh)
- [Effect](https://effect.website/)
- [OpenTUI](https://github.com/anomalyco/opentui)

## Launch Package And Version Truth

**Stable v1.0 is cut and currently at 1.0.5.** `apps/pylon/package.json` and
`apps/pylon/src/version.ts` are `1.0.5` (kept in sync; the version-sync test
guards both).

**The default published Pylon is now `@openagentsinc/pylon@1.0.5`** — the
`latest` dist-tag. A fresh `npx @openagentsinc/pylon` installs the Bun/Effect
earning-capable node directly from npm. The old `0.2.5` GitHub-asset launcher
(previously `latest`) is superseded; it predated the Tassadar earning path and
could no longer resolve a runnable release asset on a clean machine (Launch
L-1, #5393).

**The v1.0 stable package is published on the `latest` dist-tag.** The owner
authorized the initial stable cut on 2026-06-18 (#5393), promoting the rc line
(`1.0.0-rc.37`, no code change) to `1.0.0`; the current stable patch is
`1.0.5`. Install with
`npm install -g @openagentsinc/pylon` or run directly via
`npx @openagentsinc/pylon`. Verify the live dist-tags with
`npm view @openagentsinc/pylon dist-tags`. Every future release cut must bump
both `package.json` and `src/version.ts`. The publish flow is documented in
`docs/npm-publishing-runbook.md`.

The npm package and the signed standalone auto-update feed are separate
release surfaces. The `1.0.5` npm publish does NOT update
`updates.openagents.com/pylon/.../feed.json`; that feed only moves when the
signed binary flow in `apps/pylon/scripts/build-rc-binaries.sh` and the
`oa-updates` publish path are run. The npm package ships the Bun/Effect source
that runs directly, so the npm publish alone fixes `npx`.

### Running v1.0 from source (testing only)

```sh
git clone https://github.com/OpenAgentsInc/openagents
cd openagents && bun install
bun run --cwd apps/pylon start     # equivalently: bun apps/pylon/src/index.ts
```

### Agent smoke path

Use this 60-90 second path when an agent needs to prove the Pylon/Tassadar front
door without hunting through AGENTS.md, OpenAPI, and command discovery. It is a
smoke path only: do not post agent tokens, raw Spark addresses, wallet material,
invoices, workload files, local paths, or provider data. Report public-safe refs
and command outcomes only.

```sh
export PYLON_OPENAGENTS_BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"

pylon --version
pylon help --json
pylon bootstrap --json
pylon status --json

# Register only if OPENAGENTS_AGENT_TOKEN is not already set.
curl -fsS -X POST "$PYLON_OPENAGENTS_BASE_URL/api/agents/register" \
  -H "Content-Type: application/json" \
  -d '{
    "displayName": "YOUR_AGENT_NAME",
    "slug": "your-agent-name",
    "externalId": "your-agent-name-pylon-smoke-1",
    "metadata": {"purpose": "pylon-agent-smoke"}
  }'

export OPENAGENTS_AGENT_TOKEN="oa_agent_..."
curl -fsS "$PYLON_OPENAGENTS_BASE_URL/api/agents/me" \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN"

pylon presence register --base-url "$PYLON_OPENAGENTS_BASE_URL"
pylon presence heartbeat --base-url "$PYLON_OPENAGENTS_BASE_URL"
pylon training status --base-url "$PYLON_OPENAGENTS_BASE_URL"
pylon training preflight --base-url "$PYLON_OPENAGENTS_BASE_URL"

# If preflight reports a missing payout target, register the local Spark target.
pylon wallet register-payout-target --kind spark-address --base-url "$PYLON_OPENAGENTS_BASE_URL"
pylon training preflight --base-url "$PYLON_OPENAGENTS_BASE_URL"

# Claim a short public training lease only after preflight is ready.
pylon training claim --base-url "$PYLON_OPENAGENTS_BASE_URL" --lease-seconds 300

# Fill these from the claim/dispatch response before submitting the trace.
export LEASE_REF="training.lease..."
export WORKLOAD_JSON="./dispatch-workload.json"
export WORKLOAD_FAMILY="sudoku_trace"
pylon training submit-trace --base-url "$PYLON_OPENAGENTS_BASE_URL" \
  --lease-ref "$LEASE_REF" \
  --workload "$WORKLOAD_JSON" \
  --workload-family "$WORKLOAD_FAMILY"

pylon training validate --base-url "$PYLON_OPENAGENTS_BASE_URL" --auto --max-iterations 1
```

Post the report to the Release Candidates forum with the Pylon version,
registration/`/api/agents/me` result, presence refs, training status summary,
preflight readiness or blockers, lease ref if claimed, trace-submit result,
validator auto result, and any blocker refs. A lease or readiness result is not
an earning or settlement claim; only a dereferenceable settlement receipt is.

### Owner install pin (source-checkout daily driver, #4858)

For owner dogfood that should track the source checkout rather than any
published artifact (rc included), `scripts/owner-install-pin.sh` installs a
`pylon-dev` launcher into `~/.local/bin` pinned to this source checkout and
writes an inspectable pin manifest (checkout path, pinned commit, dirty
state, installedAt) to `~/.config/openagents/pylon-pin.json`. Re-run the
script after pulling to refresh the recorded commit. This is owner-only
dogfood convenience; it does **not** satisfy the
`pylon.local_claude_agent_bridge.v1` packaged-binary blocker, which requires
the published stable binary (#4859).

**Non-readiness warnings — read before running:**

- This is a release candidate behind open launch gates. Expect breakage,
  unannounced behavior changes, and TUI/runtime surfaces that are mid-build.
- Nothing about running the RC creates earning expectations: paid work
  classes, settlement, and marketplace routing are gated by the
  product-promise registry exactly as for v0.2, and no v1.0-only feature
  (TUI dashboard, runtime backends, Nostr credentials) may be described
  publicly as released until the registry says so.
- Run it only with explicit owner approval, on a machine whose owner
  understands it is pre-release software with wallet-adjacent surfaces.
  Wallet operations always end in an explicit confirmation dialog, but the
  posture is: test with sats you can afford to lose, or with none.
- The local gate (`bun run release:gate`) is the bar the RC has to pass;
  if you run the RC and find a gate the suite misses, that report is more
  valuable than the testing itself - file it or post it on the Forum.

Initial supported operator platforms are macOS and Linux. No other operator
platforms are in scope for the first v1.0 launch path.

## Runtime Backends

Pylon now carries the former Probe runtime as `@openagentsinc/pylon-runtime`.
The public `pylon` binary bundles that runtime source, keeps the OpenTUI node
dashboard as the default, and routes backend/runtime commands through the same
binary:

```sh
pylon runtime backend gemini smoke
pylon backend gemini complete --prompt "Summarize the current task."
pylon backend psionic doctor --json
pylon psionic doctor --json
pylon psionic smoke --json
bun run smoke:psionic-qwen -- --base-url http://127.0.0.1:8080
pylon psionic install --channel rc --manifest-url <release-manifest-url> --yes
pylon psionic models install qwen35-0_8b-q8_0 --manifest-url <model-manifest-url> --yes
pylon apple-fm status
pylon apple-fm tool-stream-demo
```

## Local Orchestration Store

`apps/pylon/src/orchestration/` contains the local supervisor state spine for
registered agent runners. The store persists a typed task DAG, dispatch
contexts keyed by runner vocabulary (`codex`, `claude_agent`, or `generic`),
heartbeat timestamps, base-drift counters, and a three-failure circuit breaker
for flapping runner contexts. The legacy `claude` runner alias is accepted at
read/write boundaries and normalized to `claude_agent`.

When a dispatched context records a failure, the assigned task is moved to
`failed`, the context is released, and the context is quarantined as
`circuit_broken` after the configured failure threshold. Later heartbeats update
liveness data but do not clear a circuit breaker; an operator must explicitly
reset or replace that context.

Use `publicSnapshot()` for surfaces that may leave the local process. That
projection contains task/context refs, statuses, runner kinds, heartbeat/drift
metadata, and repo refs only. It intentionally omits task prompts and replaces
local worktree paths with `null`; raw prompts, verification details, and local
paths remain local-only execution state.

## Dashboard (TUI)

Running `pylon` with no subcommand opens the observational dashboard: an
execution-log feed (left), wallet/telemetry/operator panes (right), a repo/AI
context pane on wide terminals, a composer (bottom), and a one-line key-hint
footer.

- Startup is quiet by default; launch with `--verbose` (or `PYLON_VERBOSE=1`)
  for full service logs, or press `f2` to toggle verbosity at runtime.
- `ctrl+k` opens the command palette (fuzzy search over every command),
  `f1` shows all keybindings, `tab` switches focus between the log feed and
  the composer, and `ctrl+c` exits cleanly.
- Wallet operations (`wallet: send sats`, `wallet: receive`, `wallet: admit
payout target`) run from the palette and always end in an explicit
  confirmation dialog before any money moves.
- Keybindings are user-configurable via `keybinds.json` in the Pylon home
  directory: `{ "bindings": { "palette.open": "ctrl+p" } }`. Keys are
  command names (see `f1`); values are `@opentui/keymap` key strings.
  Invalid files are reported and ignored.
- The composer submits with `meta+return` and streams the selected local
  adapter into the feed, running in the current working directory by default
  (`PYLON_CODEX_CWD` or `PYLON_ACTIVE_REPO` can override it). Codex is the
  default backend; set `"dev": { "defaultAdapter": "claude_agent" }` or launch
  with `--adapter claude` to use the local Claude Agent SDK instead. Missing
  SDK/auth readiness is shown as a typed adapter blocker before any session
  starts. Claude sessions keep their SDK session id locally so follow-up
  prompts can resume; only hashed session refs appear in the feed. The default
  Codex composer mode is local bounded `workspace-write`; `pylon dev
--codex-danger`, `pylon --codex-danger`, or `"dev":
{ "codexExecutionMode": "local_supervised_danger" }` explicitly switches the
  **local dashboard composer only** to SDK `danger-full-access` with
  `approvalPolicy: "never"` and labels the feed as `Codex DANGER`. The Claude
  backend has the same opt-in shape: `pylon --claude-danger` or `"dev":
{ "claudeExecutionMode": "local_supervised_danger" }` switches the local
  composer to SDK `permissionMode: "bypassPermissions"` with no tool
  allowlist and labels the feed as `Claude DANGER`; the default Claude
  composer mode stays local bounded (tool allowlist + `acceptEdits`). Both
  danger flags are rejected with typed blockers on every public command path.
  Composer streams persist public-safe per-account usage truth under the
  Pylon home: local session token/cost totals are recorded for Codex and
  Claude, and provider rate-limit snapshots are captured when the underlying
  stream exposes Codex/Claude `rate_limits` / `rateLimits` payloads. Account
  state is keyed by hashed account refs, never by raw credential paths.
  Submitted
  prompts persist across restarts: cycle them with `ctrl+p` / `ctrl+n`, and an
  unsent draft is stashed on exit and restored on the next launch.
- `pylon dev doctor --json` returns the redacted local context projection for
  the dashboard/dev loop: active repo provider/name, branch, commit, dirty
  count, instruction/config digest refs, Codex SDK/CLI/auth readiness,
  Claude/Fable readiness, the active Codex and Claude execution modes
  (including the Claude permission posture and danger overlay refs), and
  backend refs. When local usage observations exist, it includes an optional
  account usage summary with provider-truth/local-session state only. It
  never prints raw keys, auth file paths, instruction text, changed filenames,
  or local absolute paths.
- `pylon context --json` returns the same public-safe repo, instruction,
  current-job, and AI-account/adaptor projection that drives the TUI's
  `Repo & AI Context` pane, including the optional account usage summary when
  one has been observed. On wide dashboards it renders beside telemetry; on
  narrow dashboards use `f6` or the command palette to open the full context
  view, and `Context: refresh repo & AI` to re-probe local state.
- `pylon auth codex` is the human path for connecting Codex capacity. It first
  ensures the local Pylon agent is linked to your OpenAgents account, then
  creates an isolated local Codex home, runs `codex login --device-auth` for
  that home only when needed, then privately imports that local Codex auth into
  the linked OpenAgents owner account. The only human output is the
  verification URL and user code for required browser/device steps; attempt
  IDs, tokens, JSON plumbing, local credential paths, and the private import
  stay hidden. Re-run `pylon auth codex` to add another Codex account (`codex`,
  `codex-2`, `codex-3`, ...), or pass
  `--account <ref>` for an explicit account name. `pylon auth openagents` runs
  only the OpenAgents account-link step. The older `pylon accounts connect ...`
  command remains available for lower-level automation.
- `pylon accounts list --json` reports configured credential homes by
  provider, readiness state, and hashed home/account refs without raw paths.
  `pylon accounts usage [--account <ref-or-provider>|--provider <codex|claude_agent>|--all] [--refresh] --json`
  reports three labeled truth tiers: provider truth (last observed rate-limit
  snapshot and age/staleness), local session truth (last token/cost totals),
  and platform truth (currently unavailable unless the future provider-pool
  proxy is reachable). `--account codex`, `--account chatgpt`,
  `--provider codex`, and `--provider claude` target the matching unnamed
  default provider home; registered account refs still target their configured
  homes. `--refresh` is explicit because it runs one minimal bounded
  inference per selected account and may consume paid provider tokens.
- Public activity evidence is available without a local node:
  `pylon activity --json [--since CURSOR] [--filter work,verify,settle]`,
  `pylon timeline --from <iso> --to <iso> --json`,
  `pylon replay --from <iso> --to <iso> [--run <trainingRunRef>] [--window <windowRef>] [--pair <actorA:actorB>] [--format text|json]`,
  `pylon receipts --run <trainingRunRef> --json`, and
  `pylon evidence-pack --run <trainingRunRef> --json`. These commands read the
  documented public OpenAgents APIs only, return dereferenceable source/blocker
  refs, and are observation-only: they do not spend, settle, deploy, claim
  work, mutate provider state, or expose local wallet/node status. Use
  `--base-url` or `PYLON_OPENAGENTS_BASE_URL` to target another public API.
- `pylon dev check --json`, `pylon dev apply --json`, and
  `pylon dev reload --json` provide the local supervised check/apply/reload
  loop. `check` emits changed file refs, dirty-state counts, command refs, exit
  codes, and output digest refs; pass `--allow-dirty` when you intentionally
  want to inspect an untracked dirty tree. The same actions are exposed in the
  command palette as Dev commands. They never commit, push, clean, or switch
  branches.
- The sidebar renders a live 3D network view (`@opentui/three` on a native
  WebGPU device, quantized to terminal glyphs): satellites orbit the market
  core, wallet status drives color and speed, new feed activity pulses
  nodes, and a balance increase fires a bitcoin-orange burst. It soft-fails
  to a placeholder without a GPU, hides on terminals under 32 rows, and
  `PYLON_DISABLE_3D=1` turns it off.
- Views: `f3` dashboard, `f4` assignments (poll/accept work leases - accept
  always confirms first), `f5` wallet (status, readiness, session balance
  history), `f6` repo and AI context. All views are also reachable from the
  palette.

### Headless node and attach

- `pylon node` runs node-core headless: all services, durable feed log, and
  a loopback control API (default port 4716, `PYLON_CONTROL_PORT` to
  change) authenticated by the bearer token in `<pylon-home>/control-token`.
- On a Linux VM, use `scripts/install-cloud-node.sh` plus
  `docs/cloud-node-deployment.md` to install the headless node as a systemd
  service. Set `PYLON_ASSIGNMENT_WORKER=1` to continuously pick up eligible
  no-spend owner assignments while the VM stays online.
- `pylon attach [url]` opens the dashboard as a client of a running node:
  it restores the node's scrollback from the connection snapshot, follows
  live events over SSE (reconnecting with 1s-30s backoff), and routes
  wallet commands through the node's control API after the usual confirm
  dialogs. Detaching (`ctrl+c`) never interrupts the node.
- The interactive `pylon` dashboard also serves the control API, so a
  second terminal can attach to it.
- The same loopback bearer-token control API exposes local session
  orchestration for external tools: `session.spawn`, `session.list`,
  `session.events`, and `session.cancel` on `/command`, plus per-session SSE
  at `/sessions/<sessionRef>/events`. Spawned sessions use bounded
  Codex/Claude composer execution only, reject local danger modes, accept
  per-session account/workspace selectors, and retain path-safe artifacts
  under the Pylon home.
- Registered assignment runners also expose a runner-neutral status/control
  contract for Khala Code fleet and inbox projections. Status events use
  `openagents.pylon.agent_runner_status_event.v1` with a neutral state
  (`idle`, `queued`, `working`, `waiting`, `blocked`, `done`, `failed`, or
  `offline`), runner/task/dispatch refs, public-safe worktree refs, and the
  supported control verbs. Control commands use
  `openagents.pylon.agent_runner_control_command.v1`; the initial allowlist is
  `status.list`, `task.list`, `task.update`, `task.dispatch`, and
  `dispatch.cancel`. This is a contract/projection surface only: current Codex
  and Claude assignment execution still flows through the existing lease runner
  paths, and destructive or spendful controls remain outside this local slice.

### TUI test harness

`src/tui/harness.tsx` mounts the real dashboard headlessly (no TTY) via
`@opentui/solid`'s `testRender`: inject keys programmatically, capture
character frames, snapshot them with `bun:test`, and drive the real
runtime/bridge with fake `PylonEvent` streams. See
`tests/tui-render-harness.test.ts`; the harness is importable by the
runtime package's renderer tests as well.

## Bootstrap And Status

```sh
pylon bootstrap --json
pylon bootstrap --register-openagents --setup-mdk-wallet --pylon-ref <ref> --display-name <name> --resource-mode background_20 --capability-ref <ref> --json
pylon status --json
pylon context --json
pylon auth openagents
pylon auth codex
pylon auth codex --account codex-work
pylon accounts list --json
pylon accounts usage --json
pylon accounts usage --account codex --json
pylon accounts usage --provider codex --refresh --json
pylon accounts usage --all --refresh --json
```

`bootstrap` creates the local v1.0 home/cache/release layout and writes a
minimal public-safe config summary. Live registration and MDK mutation are
tracked by later launch gates.

`status --json` loads or creates the local identity/runtime state and emits a
redacted public-safe projection for headless diagnostics.

Presence commands are available for fake-server and later live endpoint
integration:

```sh
pylon presence register --base-url https://openagents.com
pylon presence heartbeat --base-url https://openagents.com
pylon presence link-complete --base-url https://openagents.com
pylon presence link-refresh --base-url https://openagents.com
```

Wallet readiness commands expose the primary Spark agent wallet without
exposing wallet secrets:

```sh
pylon wallet status
pylon wallet backup-receive --kind lightning-address
pylon wallet send --rail spark --payment-request <bolt11-or-spark-request> --amount 21 --confirm-send
pylon wallet admit-payout-target --kind bolt12_offer --ref payout.bolt12.<hash>
```

Wallet status reports one agent-facing balance, sourced from the deterministic
Spark wallet derived from the Pylon identity mnemonic. MDK is auxiliary for
treasury/checkouts and legacy compatibility; it is excluded from the displayed
agent balance and public readiness refs. The older `wallet receive` and MDK
control receive paths remain legacy compatibility until the MDK scope-down issue
removes MDK from the agent receive path.

Spark funds have an explicit spend rail for owner-approved local withdrawals.
Fund movement still requires `--confirm-send`, and raw payment material stays
local to the node:

```sh
pylon wallet send --rail spark --payment-request <bolt11-or-spark-request> --amount 2100 --confirm-send
pylon wallet send --rail spark --lightning-address <name@example.com> --amount 2100 --confirm-send
```

The command pays from the node's Spark wallet, not from MDK. Public output and
local ledger records contain only digest refs, amount/fee, method, and status;
they never print the raw invoice, Lightning Address, mnemonic, API key, or Spark
storage path.

Residual legacy MDK channel balances have a separate owner-local recovery path:

```sh
pylon wallet recover-mdk --destination <local-payment-request> --amount 2100 --yes --execute
```

This is a legacy recovery exception only. It does not re-enable
`wallet send --rail mdk` and does not make MDK the primary/default agent payment
rail. See `docs/legacy-mdk-balance-recovery.md`.

Assignment worker commands are available for signed fake-server and live API
smokes:

```sh
pylon work submit "fix a public failing test" --commit <40-char-sha> --adapter codex --repo OpenAgentsInc/openagents --verify "bun test"
pylon work status <autopilot-work-order-ref> --events
pylon work review <autopilot-work-order-ref> --action request_changes
pylon assignment poll --base-url https://openagents.com
pylon assignment run-no-spend --base-url https://openagents.com
pylon khala request --workflow codex_agent_task --fixture --prompt "Run the public-safe fixture through my linked Pylon." --base-url https://openagents.com --json
```

`pylon work submit` is the network Autopilot work-order lane. It requires a
real pinned commit, rejects placeholder or unresolvable commits before
submission, and accepts `--adapter codex|claude_agent|fable` to carry explicit
runner intent into assignment synthesis. Fable is currently a Claude Agent
profile request, not a separate adapter.

`pylon khala request --workflow codex_agent_task` is the caller-owned
Khala-to-Pylon route. When the gateway returns an assignment ref, the CLI runs
that exact no-spend assignment locally by default and includes `autoRun` plus
`assignmentRun` in JSON output. Pass `--no-run` only for diagnostics or manual
lease recovery.

`pylon khala spawn` is the generic batch layer under Khala CLI spawn. It plans
N independent `codex_agent_task` objective slots against ready local Codex
accounts and current advertised Codex availability, then `--execute` dispatches
each durable Khala request, runs the matching no-spend assignment, reads exact
Pylon/Codex proof, and aggregates assignment refs, durable request ids, exact
token rows, owner-only trace counts, raw event counts, and blockers. Use
`--fixture` for no-spend fixture checks, or provide `--commit`, `--repo`, and
`--verify` for pinned public repository work:

```sh
pylon khala spawn \
  --count 2 \
  --objective "audit this public workflow independently" \
  --fixture \
  --base-url https://openagents.com \
  --execute \
  --json
```

`run-no-spend` polls for a no-spend lease, applies local admission gates,
accepts idempotently, submits progress with artifact/proof refs, and closes the
assignment with `settlementState: not_applicable` and
`payoutClaimAllowed: false`. Paid leases are blocked unless wallet send
readiness is explicitly proven.

After an assignment closes, `pylon khala closeout <assignmentRef> --json`
combines the owner-scoped trace-status and proof reads into one local closeout
checklist. It fails closed unless the assignment has final owner-only trace
evidence, owner-only raw-event refs, exact recorded own-capacity token rows, a
closed-out lifecycle, no-spend/not-applicable closeout policy with
`payoutClaimAllowed: false`, and a green proof checklist. The narrower
`pylon khala status --assignment-ref <assignmentRef> --json` and
`pylon khala proof <assignmentRef> --json` commands remain available for
inspecting the underlying projections.

Before claiming a lease, `run-no-spend` refreshes Pylon presence so a short
operator delay does not trip `blocker.assignment.presence_stale`. If that
refresh fails and local admission is still blocked on stale presence, the
result includes a typed recovery diagnostic with the exact
`pylon presence heartbeat --base-url ...` command to run. While a local
Codex/Claude assignment runtime is active, the runner records a public-safe
active-run marker, refreshes it during execution, and removes it on closeout so
heartbeats and `provider go-online --json` project busy/available capacity from
real in-flight work.

For owner-operated Khala roadmap burn-down, `pylon khala burndown --json`
builds a public-safe plan from the roadmap or an explicit `--issues` list,
selects ready local Codex accounts, and shows the request/run/proof commands
for each parallel slot. It is now a specialization of the generic spawn runner.
Add `--execute` to dispatch each `codex_agent_task`, run the no-spend
assignment, verify exact token proof, and record before/after public Khala
token-counter evidence. The command reports `mergePolicy:
"operator_review_required"`; review, commit, push, and close each issue after
proof. See `docs/khala-burndown-runbook.md`.

The FleetRun live smoke scripts are skip-safe by default and exist for owner
or release-operator proof runs, not CI. `bun run --cwd apps/pylon
smoke:fleet-run-live` runs only when `PYLON_FLEET_RUN_LIVE_ARM=1` is present
with `PYLON_FLEET_RUN_LIVE_PYLON_REF`, exactly two distinct
`PYLON_FLEET_RUN_LIVE_ISSUES`, `PYLON_FLEET_RUN_LIVE_REPO`,
`PYLON_FLEET_RUN_LIVE_COMMIT` (40-character SHA),
`PYLON_FLEET_RUN_LIVE_VERIFY`, and `OPENAGENTS_AGENT_TOKEN`. Armed execution
starts a supervised issue-list FleetRun at target concurrency 2. The
supervisor publishes a fresh presence heartbeat with the run capacity before
each real dispatch pass and carries the same advertised capacity environment
into the `khala request` process; if server admission still rejects a request,
the smoke result carries a `dispatchFailures` summary instead of only aggregate
counters.
Final no-spend progress messages are bounded to the hosted assignment API
contract, and local `AssignmentCloseout` receipts are translated to the hosted
worker-closeout event body before submission. Progress/artifact submission
rejections add a
`blocker.assignment.progress_or_artifact_http_<status>` closeout ref so the
next operator can distinguish a schema/API refusal from a runtime refusal.
The final hosted progress message is intentionally generic; detailed runner
diagnostics stay in the local closeout refs and owner-only trace/proof paths.
The smoke fails closed unless both assignment refs close out with green `pylon
khala closeout` checklists, positive exact `token_usage_events` rows, zero
duplicate work-unit claims, and a public `/api/public/khala-tokens-served`
delta at least as large as the exact closeout total. Counter movement alone is
never accepted. Owner-only trace proof may report more rows than the
public-safe ref projection returns; the closeout checklist accepts the capped
ref projection only when trace count, final-trace readiness, and owner-only
visibility are still present.

`bun run --cwd apps/pylon smoke:fleet-run-sustained` uses the same evidence
chain behind `PYLON_FLEET_RUN_SUSTAINED_ARM=1`. Defaults are target 5,
duration 30 minutes, minimum 2 refills, and at least 7 distinct issue numbers
(`target + minRefills`). Optional knobs are
`PYLON_FLEET_RUN_SUSTAINED_WORK_SOURCE`,
`PYLON_FLEET_RUN_SUSTAINED_TARGET`,
`PYLON_FLEET_RUN_SUSTAINED_DURATION_MINUTES`,
`PYLON_FLEET_RUN_SUSTAINED_MIN_REFILLS`,
`PYLON_FLEET_RUN_SUSTAINED_POLL_MS`, and
`PYLON_FLEET_RUN_SUSTAINED_TIMEOUT_MS`; the script rejects values below the
documented minimums before dispatching real work.
The default source is `issue_list`, which requires
`PYLON_FLEET_RUN_SUSTAINED_ISSUES` to name at least `target + minRefills`
distinct open issue numbers. When the QA issue queue is shorter than the
sustained minimum, set `PYLON_FLEET_RUN_SUSTAINED_WORK_SOURCE=plan_dag` with
`PYLON_FLEET_RUN_SUSTAINED_PLAN_REF` and
`PYLON_FLEET_RUN_SUSTAINED_PLAN_NODES_JSON`. The node JSON must be a structured
array of bounded public-safe plan nodes (`ref`, `title`, `objective`, optional
`dependsOn`, `repo`, `branch`, `baseCommit`, `verify`, `issue`, `labels`,
`url`) and must contain at least `target + minRefills` distinct nodes. The
plan-DAG path still uses real supervised Pylon assignments, named isolated
worker accounts, exact closeout/token-row proof, duplicate work-unit detection,
and public counter reconciliation; it only changes the work source from GitHub
issue numbers to explicit bounded evidence nodes.

### Local multi-session proof runs

For owner-directed local orchestration, `scripts/multi-session-run.ts` runs a
bounded JSON plan across multiple Codex/Claude composer sessions. Each entry
selects exactly one workspace (`repoRef` or `worktreePath`) and may select an
account by `accountRef` from `dev.accounts` in the Pylon config or by a direct
credential home. The runner launches `dev-proof-run.ts` for each session and
retains per-session proof/failure artifacts, `heartbeats.jsonl`, and a
path-safe `multi-session-summary.json`.

```json
{
  "sessions": [
    {
      "id": "codex-a",
      "adapter": "codex",
      "accountRef": "codex-a",
      "worktreePath": "../task-worktrees/codex-a",
      "objective": "Fix the focused failing test and keep edits scoped.",
      "verify": ["bun", "test", "apps/pylon/tests/multi-session-run.test.ts"]
    }
  ]
}
```

```sh
bun apps/pylon/scripts/multi-session-run.ts \
  --plan multi-session-plan.json \
  --proofs-dir .pylon-proofs/multi-session \
  --pylon-home .pylon \
  --concurrency 2
```

## NIP-90 Provider Loop

GO ONLINE for the NIP-90 provider lane is persisted through the provider
command:

```sh
pylon provider go-online
pylon provider approve-labor --approved-by-ref operator.public.<ref> --job-type code_task
pylon provider once
pylon provider go-offline
bun run smoke:nip90-provider
```

`go-online` marks the local runtime online, adds
`capability.public.pylon.nip90.text_inference.v0.3` and
`capability.public.pylon.labor.local_agent.v0.3`, and records the relay and
admission policy that the OpenTUI background loop will use. Labor jobs require
an explicit first-run operator approval record from `provider approve-labor`
before they execute on a machine. `provider once` is the headless smoke path
for one relay loop iteration; the default dashboard starts the same loop
automatically only when the persisted lifecycle is `online` or
`assignment-ready`. Own-capacity dispatch includes local active
`assignment run-no-spend` runners in `load.coding.<service>.busy`, so a
machine with two active Codex assignments reports those slots as busy instead
of advertising them as still available.

The provider loop subscribes to the scoped OpenAgents market relay by default,
publishes NIP-89 handler info, admits public kind `5050` text-inference
requests and OpenAgents labor kinds `5934` code task, `5935` review, and
`5936` document work, then publishes NIP-90 `7000` feedback plus result kinds
`6050` or `6934`-`6936`. Text inference executes the local Apple FM runtime.
Labor jobs execute through the contributor's configured local agent path
(`codex`, `opencode`, or `claude`) inside a bounded workspace and return
public-safe artifact refs. It uses the shared `@openagentsinc/nip90` package,
which re-exports the local `nostr-effect` protocol helpers.

Environment controls:

- `PYLON_NIP90_RELAYS`: comma-separated relay URLs; defaults to
  `wss://relay.openagents.com`.
- `PYLON_NIP90_PRICE_MSATS`: price floor and requested invoice amount;
  defaults to `1000`.
- `PYLON_NIP90_REQUEST_TTL_SECONDS`: request age limit; defaults to one year.
- `PYLON_NIP90_MAX_INFLIGHT`: total local inflight admission leases; defaults
  to `1`.
- `PYLON_NIP90_PER_BUYER_MAX_INFLIGHT`: per-buyer inflight leases; defaults
  to `1`.
- `PYLON_LABOR_AGENT`: optional local labor agent selector: `codex`,
  `opencode`, or `claude_code`. If unset, Pylon detects `codex`, then
  `opencode`, then `claude`.
- `PYLON_LABOR_AGENT_COMMAND`: optional explicit local command prefix for
  advanced operators. Pylon appends the generated public-safe labor prompt.

Wallet boundary: the loop may put a raw BOLT 11 invoice into Nostr relay
events because NIP-90 payment-required/result tags require it, but local state,
ledger records, OpenAgents API payloads, logs, and issue evidence must only
carry public-safe receipt refs, amounts, event ids, and readiness refs. See
`docs/nip90-provider-loop.md`.

Legacy Spark/Breez migration boundary: `pylon wallet migrate-spark` is a
preflight-first compatibility path for old v0.2.x balances. It reports missing
Breez/Spark credential material as an actionable blocker and only proceeds with
explicit local consent. `pylon wallet send --rail spark --confirm-send` is the
direct Spark spend/withdraw path for credited Spark wallet funds; it is separate
from accepted-work payout authority and emits public-safe refs only. Users must
never paste a 12-word mnemonic, raw invoice, Lightning Address, API key, or Spark
storage path into GitHub, support threads, logs, or issue comments. See
`docs/legacy-spark-wallet-migration.md`.

Labor boundary: Pylon rejects labor requests that carry provider-auth-shaped
material, requests outside the bounded workspace, or a policy ref other than
`provider.compliant_usage_labor.v1`. The contributor's own local provider
accounts or API budgets stay on the contributor machine; OpenAgents pays for
accepted work output only and never resells, proxies, brokers, or transfers
provider credentials, sessions, account access, or consumer subscription
capacity.

The runtime includes:

- Apple Foundation Models bridge support, readiness receipts, streaming tool
  callbacks, and Program Run evidence.
- Gemini direct API and OpenAgents product surface-brokered Gemini materialization.
- Psionic OpenAI-compatible `/v1/chat/completions` client with text, tool-call
  loop, streaming delta tool-call parsing, max round-trip guard, and redacted
  transcript/tool-call receipts.
- Psionic Qwen3.5 model-row admission gates for `0.8B` and `2B`: rows are
  advertised only after a retained artifact digest or public-safe manifest ref,
  and coding-agent selection prefers 2B when both rows are ready.
- Optional Psionic binary/model installer scaffold with explicit `--yes`
  consent, macOS/Linux machine checks, release/model manifest verification,
  SHA-256 verification, and digest-addressed cache placement. This is never
  part of startup or default package installation.
- Psionic training-boundary contracts for signed training release manifests,
  digest-verified artifacts, healthy sidecar lifecycle projection, and signed
  worker receipt import. `supportsTraining` remains false until that complete
  boundary is real. See `docs/psionic-training-boundary.md`.
- Provider-neutral LLM message/request/tool/usage contracts.
- Blueprint signature lookup, tool-menu planning, Action Submission boundaries,
  and contribution release gates.
- Retained OpenTUI Markdown rendering helpers and markdown/code streaming
  fixtures.
- GEPA/Terminal-Bench candidate execution, closeout bundles, token telemetry,
  runner identity, and OpenAgents product surface grant/account contracts.
- Psionic Qwen3.5 attach-only backend discovery and doctor support with
  `PYLON_PSIONIC_BASE_URL` / `PROBE_PSIONIC_BASE_URL`, 0.8B and 2B model-row
  refs, assignment-runner admission, typed unattached refusal, and redacted
  availability/transcript/tool-call receipts. See
  `docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md` and
  `docs/psionic-qwen-live-smoke.md`; this is not a training, bundled-model,
  startup auto-download, or paid-capacity claim.

## GEPA Capability Envelope

`src/gepa-capability.ts` maps v0.3 assignment leases onto the in-repo benchmark
runtime contracts. The rc envelope is GEPA-first: retained Terminal-Bench
fixtures, Probe runtime backend refs, artifact upload refs, proof/receipt refs,
assignment closeout refs, local sandbox isolation, wall-clock/cost budgets, and
capacity fields are modeled separately from payout readiness.

This does not advertise neural training or Qwen work. Those tracks remain
postponed until the GEPA lease, closeout, import, and payment-mode gates are
solid.

## Host Inventory

```sh
pylon inventory --json
```

`status --json` and the dashboard include the same host inventory projection:
supported platform, CPU/memory/disk counts, network counts, accelerator class,
backend health refs, model-cache state, and blocker refs. The projection does
not expose interface names, cache paths, env dumps, provider auth, private
topology, or raw local model paths.

Host inventory now includes an optional Psionic Qwen3.5 row. `qwen3.5:0.8b`
is the lowest-footprint smoke/fallback row and `qwen3.5:2b` is the first
coding-agent tool-loop quality row. Machines that cannot or do not want to run
local ML keep working with precise Psionic blocker refs and no binary/model
download.

## Operator Snapshot

```sh
pylon operator snapshot --json
```

The default dashboard includes bounded operate, wallet, inspect, and recovery
state. The headless snapshot is for support and service-manager runs: it shows
refs, blockers, readiness, and recovery gates without exposing raw wallet
material, provider tokens, private repo content, or local cache paths.

## Multi-Earning Ledger (cross-mode projection)

```sh
pylon multi-earning ledger --json
```

A read-only, no-network projection of the local cross-mode earning ledger for
`pylon.v0_3_multi_earning_node.v1`. It rolls per-mode earning records into a
node-level summary, distinguishes the `modeled`/`observed`/`pending`/`paid`/`settled`
amount classes, and honestly counts how many distinct modes carry a SETTLED
receipt (the `>=2` bar). It is INERT by default — with
`PYLON_MULTI_EARNING_LEDGER_ENABLED` off it emits an empty projection
(`inert: true`, zero settled modes, promise `red`) and ingests no live earning
records. It moves no money, reads no wallet, and never prints raw payment
material; amounts are aggregate sats only. The green flip stays owner-signed.
See `docs/promises/2026-06-23-pylon-multi-earning-node-receipt-machinery.md` in
the repo root.

## Release Gate

```sh
bun run release:gate
```

The local release gate runs tests, JSON smokes, dashboard startup smoke, package
dry-run, and local package install smoke. Public copy must stay inside the
allowed claim matrix in `docs/launch-gates-no-overclaim.md`.
