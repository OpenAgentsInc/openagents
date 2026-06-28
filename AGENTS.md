# OpenAgents Agent Contract

## Scope

This repository is the new OpenAgents Bun and Effect monorepo.

Preserve `docs/transcripts/`. It is the retained transcript archive from the
previous repository shape.

## Autonomous Loop: Constant Motion (owner mandate)

When running the autonomous AFK loop (`/loop`, see
`docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md` — read it every
iteration), the **top operating rule is CONSTANT MOTION**:

- **Never sit idle. Never sleep on a minutes-long timer.** Do real work every
  moment the loop is active. There is always more work (active product
  integration, the issue backlog, the terminal-agent-systems well, the clarity
  sweep) — "nothing to do" is never true.
- **Long `ScheduleWakeup` idle waits are banned.** Keep working in the SAME turn:
  finish a unit → immediately start the next. If you must yield, prefer a fanout
  whose watcher re-invokes you instantly; only if truly unable to proceed this
  instant, use a SHORT wakeup (≤120s), never minutes.
- **Blocked on the owner? Pull other work.** Write a clear `NEEDS-OWNER:` note
  and immediately continue on a non-blocked item. An owner-gated step never
  stalls the loop. The owner's reply interrupts and takes priority, but you do
  not wait for it.

## Repo Layout

- `apps/openagents.com/` owns the `openagents.com` product surface, including
  the current Autopilot, Forum, Sites, and public proof implementation
  material.
- `apps/openagents-world/` is the Cloudflare Worker + Region Durable Object
  home for live Verse world projection, presence, local interaction,
  interest-scoped fanout, world WebSocket transport, D1 projection rows, queue
  markers, and DO alarm expiry. New world-backend work belongs there, using
  Effect, Effect Schema, D1, hibernatable WebSockets, and the shared world
  packages below.
- `packages/world-contract/` is the shared Effect Schema contract home for
  public-safe world rows, commands, deltas, cursors, moderation decisions, and
  WoC-style read-model projection types.
- `packages/world-client/` is the shared desktop/web Verse world client that
  mirrors snapshots and deltas into a read-only `WorldReadModel`.
- The old self-hosted SpacetimeDB `openagents-world` module was deleted during
  the Cloudflare Verse World cutover. Do not re-clone, regenerate bindings for,
  or add production world features to that path; port useful historical schema
  or reducer ideas into the Cloudflare/Effect world service instead.
- `apps/forum/` owns the forum extraction target for
  `openagents.com/forum`. The live Forum routes stay inside the
  `openagents.com` Worker for now because they share auth, D1, payment
  receipt, and public projection boundaries.
- `apps/forge/` owns the separate `forge.openagents.com` UI app. It reuses
  shared `@openagentsinc/ui` primitives/tokens but does not route through the
  main `openagents.com` logged-in page tree.
- `apps/pylon/` owns the Pylon contributor app imported from the standalone
  Pylon repository. It bundles the former Probe runtime as
  `@openagentsinc/pylon-runtime`.
- `apps/nostr-relay/` owns the Nostr relay surface.
- `packages/probe/` owns the Probe runtime imported from the standalone Probe
  repository.
- `packages/nip90/` owns the NIP-90 protocol library for the compute, data,
  and labor market rails.
- `docs/promises/` owns product-promise records, launch-promise source sets,
  verification gates, copy gates, and user/agent report templates.
- `docs/refactor/` owns migration plans, cutover notes, and architectural
  cleanup records for this repo reset.
- `docs/transcripts/` owns the retained transcript archive for episodes
  001-234 of the build series, with a theme guide in
  `docs/transcripts/README.md`.
- `docs/tassadar/` owns the Tassadar research essays on exact-execution
  LLM computers and verification by replay.
- `docs/autopilot-coder/` owns Autopilot Coder status audits, smoke runbooks,
  and the paid L402 boundary notes.
- `docs/forum/`, `docs/nostr/`, and `docs/research/` own dated audits for
  those areas.

## Live Public Reference Surfaces

- Agent onboarding instructions: <https://openagents.com/AGENTS.md>
- Product promises page: <https://openagents.com/docs/product-promises>
- Agent-readable promise registry:
  <https://openagents.com/api/public/product-promises>
- Product Promises Forum:
  <https://openagents.com/forum/f/product-promises>
- Strict bug form:
  <https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml>

## Help a user connect their Codex fleet to Khala

Use this when an outside/community user wants to connect their own Codex
account(s) to Khala so a per-user Artanis can burn down a backlog across their
fleet ("Artanis as a Service"). It is intentionally DEAD SIMPLE — one short
command, no long-string pasting, no `PYLON_HOME`/`bun`/repo-path knowledge.

Walk the user through exactly this:

```sh
npm install -g @openagentsinc/khala     # Node 20+ or Bun; npm works for everyone
khala fleet connect                     # connect a Codex account (paste-free device login)
khala fleet status                      # see your fleet
```

What the user will see: `khala fleet connect` drives the standard
`codex login --device-auth` flow — it opens the browser to the device URL and
shows a SHORT code to enter (no long auth string to copy). It then confirms with
the linked account email. The `codex` CLI must be installed
(`npm install -g @openai/codex`); if it is missing, `khala fleet connect` prints
a friendly install hint.

- **More accounts = more throughput.** Each `khala fleet connect` auto-assigns
  the next ref (`codex`, then `codex-2`, `codex-3`, …); pass `--account <ref>`
  to name one. Distinct ChatGPT accounts have distinct rate budgets, so each new
  *distinct* account is real added concurrency.
- **`khala fleet status`** (alias `khala fleet list`) prints a table of
  connected accounts with readiness (`ready` / `credentials-missing`) and email.
- **Safety (always true):** each account uses an ISOLATED home under
  `<pylon home>/accounts/codex/<ref>`; the flow NEVER touches the default
  `~/.codex` home (that would wipe a live session); credentials stay on the
  user's machine and tokens are never printed. Accounts are registered into the
  user's Pylon config (`<pylon home>/config.json`), so a local Pylon, the codex
  supervisor, and the server dispatch gate all see the fleet.

This is the onboarding front door; the request/proof contract for routing actual
coding work through the connected fleet is the runbook below.

## Khala -> Pylon -> Codex Coding Delegation Runbook

Use this when a user wants coding work routed through Khala to the user's own
local Pylon, with Pylon executing the assignment through the local Codex-capable
session. The deeper smoke doc is
`docs/khala/2026-06-25-bare-agent-pylon-mcp-khala-e2e-smoke.md`; the invariant
ledger is `apps/openagents.com/INVARIANTS.md` under "Khala Coding Delegation
Through Pylons". For running this engine 24/7 at scale (standing pylon, codex
supervisor, identity/token footguns, and stall diagnosis), see the operations
runbook `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`.

> **DO NOT clobber the owner's live Codex session.** NEVER run `codex login` /
> `codex login --device-auth` (or `pylon auth codex`) against the **default
> `~/.codex` home**. `codex login` CLEARS `~/.codex/auth.json` at flow-start, so
> running it (or killing it mid-flow) against the default home **wipes the
> owner's live `codex` session** and breaks their active work with
> "access token could not be refreshed ... sign in again". When testing or
> debugging Pylon auth / the codex device-login, ALWAYS isolate it to a throwaway
> `CODEX_HOME` (e.g. `CODEX_HOME=$(mktemp -d) codex login --device-auth`). The
> real `pylon auth codex` flow already uses isolated per-account homes
> (`<pylon home>/accounts/codex/<ref>`) and must never write to `~/.codex`.
> To inspect connected accounts, use `pylon accounts list` (human view: email +
> last linked) or `pylon codex accounts list --json` (public-safe alias for
> `pylon accounts list --json`) — never re-run a login to "check".

Prerequisites:

- The caller has a valid `OPENAGENTS_AGENT_TOKEN` in the environment. Never
  print it, paste it into issue comments, or commit it.
- The local Codex login exists, normally `~/.codex/auth.json`. Treat it as
  private local credential material.
- The Pylon command may be either installed `pylon` or, from this repo,
  `bun apps/pylon/src/index.ts`. Examples below use `$PYLON` for either form:

```sh
export PYLON_OPENAGENTS_BASE_URL="https://openagents.com"
export PYLON="bun apps/pylon/src/index.ts"
```

Run `$PYLON` from a clean worktree at current `origin/main`. If the normal
`/Users/christopherdavid/work/openagents` checkout is dirty or behind, create or
reuse a clean detached worktree and set `PYLON` from that directory instead of
the dirty checkout. For one-shot proof runs, set `PYLON_DISABLE_DAEMON_ROUTING=1`
so a stale loopback `pylon node` cannot answer with old source code. If a node is
already listening from a stale checkout, stop or restart it from the clean
current worktree before using it as evidence for Pylon/Codex delegation.

0. Confirm the linked Codex account inventory:

```sh
$PYLON codex accounts list --json
```

Expected output lists each configured Codex account with
`readiness.state: "ready"` and `capability.pylon.local_codex` before you route
work to it. Use this before parallel delegation and after every new
`pylon auth codex` login. The older `$PYLON accounts list --json` path remains
equivalent; prefer the Codex namespaced alias in Pylon/Codex runbooks so the
operator intent is unambiguous. For a specific account proof, run the refresh
path explicitly:

```sh
$PYLON accounts usage --account "<codex account ref>" --refresh --json
```

That refresh consumes a minimal provider call and should return a
`truth.localSession.usage` record for the selected account. It proves the local
Codex login works, but it is not the Khala counter proof; still perform the
delegation and `token_usage_events` checks below.

1. Bring the owner Pylon online and publish fresh capacity:

```sh
$PYLON provider go-online
$PYLON presence heartbeat
```

`provider online` is accepted as an alias for `provider go-online`. The
heartbeat should return a `pylonRef`, `registered: true`, a fresh
`lastHeartbeatAt`, and no blocker refs. The public Pylon projection should show
Codex refs such as `capacity.coding.codex.available=1`,
`capacity.coding.codex.ready=1`, `load.coding.codex.busy=0`, and
`load.coding.codex.queued=0`. Counted capacity refs with `=N` are valid and must
not be stripped. To exercise same-account parallel work, set and publish a
capacity greater than one, for example:

```sh
OPENAGENTS_PYLON_CODEX_CONCURRENCY=2 \
OPENAGENTS_PYLON_CODEX_BUSY=0 \
OPENAGENTS_PYLON_CODEX_QUEUED=0 \
$PYLON presence heartbeat
```

The dispatch gate admits at most the heartbeat-advertised available Codex slots
for that caller-owned Pylon. If a second fresh request is refused while fewer
than that many assignments are active, inspect the Pylon assignment rows and the
projected `capacity.coding.codex.available=N` refs before proceeding.

2. Capture the public counter baseline:

```sh
curl -fsS https://openagents.com/api/public/khala-tokens-served
```

The homepage counter with `data-counter="khala-tokens-served"` is backed by
this endpoint and the matching public sync feed.

3. Issue a typed Khala coding request against the caller-owned Pylon:

```sh
$PYLON khala request \
  --prompt "Run the public-safe fixture task through my linked local Codex Pylon." \
  --workflow codex_agent_task \
  --pylon-ref "<owner pylon ref>" \
  --fixture \
  --json
```

Expected output includes `ok: true`, `assignmentRef`,
`durableRequestId`, `durableStreamUrl`, `workflow: "codex_agent_task"`, and a
delegation frame naming the targeted Pylon. The CLI immediately follows a
returned assignment ref by running the matching local no-spend assignment and
adds `autoRun` plus `assignmentRun` to the JSON output; use `--no-run` only for
diagnostics when you intentionally want to create a lease without executing it.
If the request falls through to a model/provider path instead of returning a
delegation frame, stop and debug the delegation preconditions before running
spendful or unrelated work.

For real repository work, pin the public checkout and verification command so
the Pylon materializes a fresh bounded Git workspace instead of the fixture:

```sh
$PYLON khala request \
  --prompt "Implement public issue #NNNN and run the named verification." \
  --workflow codex_agent_task \
  --pylon-ref "<owner pylon ref>" \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "<current origin/main sha>" \
  --verify "bun run --cwd apps/openagents.com/workers/api test -- src/path.test.ts" \
  --json
```

Keep this prompt public-safe and bounded: cite public issue numbers, public file
paths, and public verification commands only. Do not include raw private prompts,
secrets, local paths, provider payloads, wallet material, or private repo
content. The Pylon runner receives the public objective summary plus the pinned
checkout refs; raw Codex events and local workspace paths stay on the device.
For caller-owned Khala -> Pylon -> Codex assignments, the local Codex runner uses
the SDK equivalent of `--dangerously-bypass-approvals-and-sandbox`: sandbox mode
`danger-full-access`, approval policy `never`, and network enabled. That full
access is an owner-local executor invariant so Codex can do real Git/GitHub work;
do not add it as a public wire field or use it for untrusted labor/provider
work.

4. Verify the local no-spend execution:

For the CLI path above, execution already happened in the same command. Expected
`assignmentRun` output: the lease is accepted, progress reaches `proof-ready`,
and the closeout status is `accepted` with
`settlementState: "not_applicable"` and `payoutClaimAllowed: false`. For the
public fixture, a successful run includes
`result.public.pylon.codex_agent_task.fixture_repair_passed`.

Then read the owner-scoped closeout proof:

```sh
$PYLON khala closeout "<assignmentRef>" --json
```

Expected `closeoutChecklist.ok: true` means the assignment trace-status and
proof projections agree on assignment, Pylon, and owner refs; final owner-only
trace and raw-event summaries exist; exact own-capacity token rows are
recorded; the worker closeout event proves `paymentMode: "no-spend"`,
`settlementState: "not_applicable"`, and `payoutClaimAllowed: false`; and the
lifecycle is closed out without rejection refs. Use
`$PYLON khala status --assignment-ref "<assignmentRef>" --json` and
`$PYLON khala proof "<assignmentRef>" --json` only when inspecting the two
underlying projections separately.

For MCP, bare-agent, or explicit `--no-run` diagnostic paths, execute the
assignment locally with no spend:

```sh
$PYLON assignment run-no-spend --json
```

For parallel delegation, run each assignment with an explicit assignment ref and
publish capacity first:

```sh
OPENAGENTS_PYLON_CODEX_CONCURRENCY=2 \
OPENAGENTS_PYLON_CODEX_BUSY=0 \
OPENAGENTS_PYLON_CODEX_QUEUED=0 \
$PYLON presence heartbeat --json

$PYLON assignment run-no-spend --assignment-ref "<assignmentRefA>" --json
$PYLON assignment run-no-spend --assignment-ref "<assignmentRefB>" --json
```

Current Pylon stores owner-local process and heartbeat evidence for accepted
no-spend leases. If a previous local run was interrupted, a fresh runner should
submit a public-safe stale closeout (`blocker.assignment.local_run_interrupted`)
before claiming new work, so abandoned accepted rows do not poison the
advertised Codex capacity until server lease expiry. If fresh dispatch is still
refused, inspect the Pylon assignment rows for non-expired active leases and
verify whether their local owner process is still alive before creating more
requests.

5. Verify durable resume:

```sh
$PYLON khala resume "<durableRequestId>" --offset 0 --json
```

Expected output includes the original delegation frame, `[DONE]`,
`streamClosed: true`, and `streamUpToDate: true`.

6. Confirm exact downstream Codex token rows and private traces.

The source of truth for this flow is no longer the chat/MCP handoff. The chat
route creates the Pylon assignment, then local Pylon posts each completed Codex
SDK turn to `POST /api/pylon/codex/turns`. That registered-agent ingest route is
the only place the downstream Codex tokens should be counted, and it stores the
matching redacted owner-only ATIF trace. Verify exact rows first:

```sql
SELECT id, idempotency_key, account_ref, actor_user_id, session_ref, task_ref,
       provider, model, input_tokens, output_tokens, reasoning_tokens,
       cache_read_tokens, total_tokens, usage_truth,
       demand_kind, demand_source
  FROM token_usage_events
 WHERE provider = 'pylon-codex-own-capacity'
   AND model = 'openagents/pylon-codex'
   AND usage_truth = 'exact'
   AND demand_kind = 'own_capacity'
   AND demand_source = 'khala_coding_delegation'
   AND task_ref = '<assignmentRef>'
 ORDER BY observed_at DESC;
```

Expected: one row per completed Codex SDK turn. The row must be owned by the
linked OpenAuth/user account (`actor_user_id`) while `account_ref` remains the
local Pylon agent account, and `total_tokens` must reflect the exact SDK usage
for that turn. For Codex rows, reasoning output tokens are counted into the
public served-token total while also preserved in `reasoning_tokens`.

Then verify the redacted owner-private trace:

```sql
SELECT trace_uuid, owner_user_id, agent_ref, session_id, trajectory_id,
       visibility, schema_version, step_count, demand_kind, demand_source
  FROM agent_traces
 WHERE demand_kind = 'own_capacity'
   AND demand_source = 'khala_coding_delegation'
   AND trajectory_id LIKE 'pylon_codex:<assignmentRef>:%'
 ORDER BY created_at DESC;
```

Expected: `visibility='owner_only'`, `schema_version='ATIF-v1.7'`, owner equals
the linked OpenAuth/user account, and the stored trajectory has been scrubbed
before tripwire. The trace projection may contain bounded agent messages,
reasoning summaries, tool labels, file-change counts, and command output byte
counts; it must not contain raw prompts, raw shell output, API keys, provider
credentials, local auth paths, wallet material, or private repo data.

Trace ingest failures are fail-soft: the local Codex task and exact token row
should still complete, with only a public-safe diagnostic returned by the ingest
route. Token-ingest failures are not acceptable proof; rerun or debug them until
the exact `token_usage_events` row exists.

The redacted ATIF trace is only the public-safe summary. While the Codex SDK
turn is still running, local Pylon streams raw SDK event chunks to
`POST /api/pylon/codex/event-chunks`; the Worker stores those chunks in private
owner-scoped blob storage under the Pylon/Codex raw-event-chunk prefix, with D1
metadata rows in `pylon_codex_raw_event_chunks` keyed by
assignment/session/owner/turn/chunk. Verify that chunk rows exist before
treating a long-running delegation as observable:

```sql
SELECT chunk_ref, assignment_ref, session_ref, turn_index, chunk_index,
       event_count, byte_length, demand_kind, demand_source, observed_at
  FROM pylon_codex_raw_event_chunks
 WHERE assignment_ref = '<assignmentRef>'
 ORDER BY turn_index ASC, chunk_index ASC;
```

At final turn closeout, Pylon also posts the complete ordered Codex SDK event
stream to `POST /api/pylon/codex/turns` as `rawEvents`; the Worker stores that
canonical whole-turn archive in `pylon_codex_raw_events` for audit and
idempotent replay checks. Raw chunks and final archives may contain prompts,
command/tool args, local paths, file-change details, and shell output; they
must never be copied into public traces, counters, issue comments, Forum posts,
product-promise output, or public closeout refs. Raw-event persistence is
fail-soft for the local coding task and should return only private-safe refs or
diagnostics. Token accounting remains exact-only: do not synthesize public
counter deltas from chunks; reconcile the counter against the exact
`token_usage_events` rows posted from `turn.completed.usage`.

7. Confirm the public counter projected those exact rows:

```sh
curl -fsS https://openagents.com/api/public/khala-tokens-served
```

The new `tokensServed` value must increase by at least the sum of the newly
inserted exact downstream Pylon/Codex rows. Counter movement by itself is never
proof, because other agents may be running. Treat the homepage and `/khala`
counters as public projections of `token_usage_events`, then reconcile the
projection back to the exact rows above.

When supervising parallel work, also verify the `pylon_api_assignments` rows
for each assignment reached `closeout_submitted` and `pylon_api_events` contains
one acceptance, progress, artifact/proof, and worker closeout event per
assignment.

Common failure signatures:

- `target_pylon_not_authorized` or "requested Pylon is not linked" means the
  token does not own or link to that Pylon, or caller-aware delegation regressed.
- `target_pylon_unavailable` means the Pylon is not active, heartbeat-fresh,
  Codex-capable, wallet-ready where required, or capacity-available.
- A provider error about extra `openagents` inputs means delegation did not
  happen and the request fell through to normal provider routing. Recheck
  `--workflow codex_agent_task`, target Pylon freshness, and caller ownership.
- A heartbeat validation error on `capacity.coding.*=N` means the counted
  capacity-ref schema regressed.

Known public-safe steering gaps to keep visible:

- The runbook proves caller-owned Pylon targeting through an explicit
  `--pylon-ref`. Do not treat it as proof of broad automatic steering from any
  Khala request to any linked capacity until the caller-scoped capacity resolver
  and router branch are verified in the same deployment.
- The authorization boundary is the token-resolved owner scope. A remote issuer
  must only read and target Pylons linked to that same owner scope; never widen a
  routing test to pooled, third-party, marketplace, or settlement-bearing
  capacity while validating this own-capacity path.
- The typed coding request path must remain explicit. If `--workflow
  codex_agent_task` or the equivalent typed MCP/tool field is missing, assume the
  request may fall back to normal model routing and stop before running spendful
  work.
- Counted capacity refs are part of steering correctness, not display-only
  telemetry. Before testing parallel dispatch, confirm the heartbeat projection
  carries `capacity.coding.codex.available=N`, busy, queued, and ready refs for
  the targeted Pylon, then verify active assignment rows do not exceed that
  advertised availability.
- The OpenAuth account to many-keys/many-Pylons aggregation is separate from the
  single owner-scoped execution invariant. Aggregation may make linked capacity
  easier to discover, but it must not allow one owner scope to execute against
  another owner's Pylon.
- Counter movement alone is never completion evidence for Pylon/Codex work. The
  workflow needs a first-class command that resolves `assignmentRef` to the
  exact `token_usage_events` rows and `agent_traces` rows, including provider,
  model, `usage_truth`, `demand_kind`, `demand_source`, visibility, and token
  totals, so agents do not have to query D1 directly.
- `assignment run-no-spend --json` should expose live progress while Codex is
  running: elapsed time, last progress event, current phase, and the assignment
  ref being worked. A long silent run is hard to supervise and hard to
  distinguish from a stuck executor.
- Assignment closeout should include the local workspace path or a safe local
  lookup command. Today the public-safe `previewRefs` are correct for reports
  but force the supervising agent to infer the cache path before inspecting the
  patch.
- Parallel delegation from one account is valid and should have an explicit
  runner command that accepts several assignment refs, leases up to advertised
  capacity, and reports per-assignment closeouts. Manual background shells are
  too easy to misattribute.
- `presence heartbeat` should exit cleanly after the heartbeat write. If a
  runtime handle keeps the process alive, operators currently need an outer
  timeout even though the heartbeat already succeeded.
- The Khala request safety guard should support an explain/dry-run mode for
  public issue work. During this run, ordinary safety words in an issue-summary
  prompt were rejected without naming the offending field, and an unsupported
  verifier shape returned a server 500 instead of a typed client error.

Report evidence with the deployment commit, Worker version, live `/` and exact
asset smoke, `pylonRef`, `assignmentRef`, `durableRequestId`, closeout refs, and
before/after counter values. Keep raw tokens, private prompts, wallet material,
and local Codex auth out of reports.

## Deploying & Releasing

- **`docs/DEPLOYMENT.md` is the single hub for every deploy / publish / release.**
  Read it first for any of: deploying the `openagents.com` Cloudflare Worker,
  publishing Pylon to npm, cutting a Pylon or Autopilot Desktop release (incl. the
  signed/notarized macOS DMG), the `updates.openagents.com` OTA feed, or the mobile
  app. It indexes the per-surface runbooks (the sources of truth), the one-line
  recipe for each, the GitHub release-tag convention, and where the signing
  secrets live (`~/work/.secrets/` + GCP Secret Manager, project `openagentsgemini`).
- Signing/notarization details live in `apps/oa-updates/docs/release-signing-runbook.md`
  (ed25519 release key + the `HQWSG26L43` Apple Developer ID) — read before any signed
  release. Publish/deploy only from a clean `origin/main`; RCs are pre-releases and
  never take the stable `latest` badge.

## Working Rules

- Read `INVARIANTS.md` before changing authority, routing, payment,
  projection, or public-claim surfaces.
- For work under `apps/openagents.com/`, also read
  `apps/openagents.com/AGENTS.md` and `apps/openagents.com/INVARIANTS.md`.
- **Leave it cleaner than you found it — clean up as you go, every phase.** When you
  touch an area and find pre-existing breakage (failing tests, lint, type errors,
  doc-coverage/OpenAPI/AGENTS.md drift, stale refs, dead code), **fix it even if you did
  not cause it** rather than stepping around it or deferring. Nothing accumulates: every
  phase, branch, and PR lands with the full relevant test suite **and** `check:deploy`
  green — not "green except the pre-existing reds." If a pre-existing failure is genuinely
  too large or out of scope for the current change, fix what is cheap and **explicitly
  flag the rest** (in the report, and a tracking issue if it will persist) — never
  silently leave a red, and never describe a partially-green run as clean.
- Keep new TypeScript implementation work on Bun, Effect, Effect Schema, and
  Foldkit where `apps/openagents.com` already uses it.
- Never stash, reset, checkout, restore, or otherwise move another agent's
  uncommitted work out of the way. If a checkout is dirty with concurrent work
  and you need a clean tree for tests, commits, or pushes, create a fresh
  worktree from clean `origin/main` and do the scoped work there. Leave the
  original dirty checkout intact and report the conflict or blocker honestly.
- Do not reintroduce the old Cargo or Tauri workspace unless the user asks for
  explicit historical compatibility work.
- **Mobile build/ship policy (owner mandate): NO Expo/EAS cloud.** The current
  mobile app is the **native SwiftUI** voice app `clients/mobile/Khala`
  (bundle `com.openagents.khala`) — see
  `docs/mobile/2026-06-26-khala-voice-app-spec.md`. It is pure local Xcode:
  build/run via `clients/mobile/Khala/Khala.xcodeproj`, archive with
  `xcodebuild`, and upload to TestFlight with Apple-native `xcrun altool`
  (ASC key in `.secrets/appstoreconnect.env`), under Apple Team `HQWSG26L43`.
  Native Swift has **no OTA path** (`updates.openagents.com` / `expo-updates`
  do not apply). Never run `eas build` / `eas submit` / `eas update`.
  The earlier Expo React-Native app `clients/mobile/AutopilotRemoteControl`
  was **retired** on 2026-06-26 and removed from the repo
  (`docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`); the Expo
  prebuild + own-OTA path (`apps/oa-updates/scripts/publish-ota.sh`) and the
  `expo` CLI only apply if an Expo app is ever reintroduced.
- Route new user-facing and agent-facing product claim systems through
  `docs/promises/` before broadening copy.
- Keep Claim Your Agent public identity flows tweet-first where possible:
  use the shared owner-claim/X verification routes, the friendly
  `Verifying my agent ... Code: ...` copy, and public tweet-author binding
  rather than adding a parallel identity-verification path.
- Keep product-promise report intake Forum-first. Agents and users should post
  loose reports, product-promise gaps, feature commentary, and discussion in
  the Product Promises Forum.
- GitHub issues are only for concrete, reproducible bugs that satisfy the
  strict bug issue form. Blank issues are disabled, and malformed or loose
  reports should be rejected by the issue form or moved back to the Forum.
- Do not commit secrets, dependency caches, build output, `target/`, `dist/`,
  `node_modules/`, or local runtime state.
- Before publishing ANY npm package from this repo, read
  `apps/pylon/docs/npm-publishing-runbook.md`. The scope is
  `@openagentsinc/` (never `@openagents/`), the auth token lives in
  workspace `.secrets/npm-publish.env`, `bun publish` is broken against
  npmjs (use `bun pm pack` + `npm publish <tarball>`), Pylon pre-stable
  releases publish under `--tag rc` only, and registry-CDN propagation
  makes fresh publishes look 404 to bun for minutes — the runbook covers
  all of it.
- Keep Git operations scoped to this repository when working here.
- Do not put individual people’s names in commit messages, commit trailers, or
  other committed metadata unless the user explicitly asks for a legally or
  historically required attribution. Use neutral product, team, source,
  operator, or role wording instead.
