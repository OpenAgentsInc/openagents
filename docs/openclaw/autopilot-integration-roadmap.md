# Autopilot <-> OpenClaw Integration Roadmap (Two-Way)

## Goal

Deliver full two-way communication between Autopilot (execution engine) and OpenClaw (messaging/control plane):

- OpenClaw can start, monitor, and control Autopilot runs.
- Autopilot can stream progress, request approvals, and post results into OpenClaw sessions.
- All interactions remain verifiable, logged, and replayable.

This roadmap is grounded in:
- OpenClaw integration surfaces (Gateway WS/HTTP, plugins, hooks, sessions, nodes).
- Autopilot Desktop event and guidance pipeline (AppEvent stream + guidance/* events).
- Verified Patch Bundle contract and tool execution invariants.

## Non-negotiable invariants

These are architectural contracts that the integration must respect:

- Verified Patch Bundle is canonical output: `PR_SUMMARY.md`, `RECEIPT.json`, `REPLAY.jsonl`.
- Tool execution validation + retries live in the runtime (not adapters).
- Tool calls must emit deterministic hashes + receipts.
- Session artifacts live under `${OPENAGENTS_HOME}/sessions/{session_id}/`.

If the integration introduces or changes any contracts (tool schemas, event schemas, protocol surfaces), it must ship with:
- an ADR update (docs/adr/)
- a glossary update if new terms are introduced
- updates to ARTIFACTS.md / REPLAY.md / PROTOCOL_SURFACE.md as needed

## Current integration surfaces (grounded)

### OpenClaw (external repo, from docs)

- Gateway WS protocol (operator or node clients).
- Gateway HTTP endpoints (`POST /tools/invoke`, `/v1/responses`).
- Plugin system: tools, gateway methods, HTTP handlers, services, hooks.
- Agent loop hooks (before_agent_start, agent_end, etc).
- Sub-agents + session tools (`sessions_spawn`, `sessions_send`).
- Nodes (device-style commands).
- Skills for prompt-time guidance.

### Autopilot (this repo)

- `apps/autopilot-desktop/src/main.rs`:
  - AppEvent stream + AppServerEvent forwarding.
  - Guidance events emitted as `guidance/status`, `guidance/step`, `guidance/response`, `guidance/user_message`.
  - Full-auto decision pipeline (DSRS signatures) with structured events.
- `apps/autopilot-desktop/src/full_auto.rs`:
  - Full-auto decision state and guardrails.
  - Turn summaries (plan, diff, approvals, token usage).
- `crates/autopilot_app`:
  - `AppEvent` typed stream and replay recorder.
  - Workspace/session abstraction.
- Autopilot artifacts:
  - Verified Patch Bundle spec and storage location.

## Target architecture (high-level)

Two-way integration is split into two planes:

1) Control plane (OpenClaw <-> Autopilot run lifecycle)
- Start/stop/status of Autopilot runs
- Approval requests and responses
- Job metadata (repo, model, lane, timeout)

2) Data plane (progress + artifacts)
- Streaming progress (guidance events, tool summaries)
- Final results (Verified Patch Bundle)
- Optional: replay snippets or links

Both planes should be authenticated and auditable. All interactions must emit receipts or logs where appropriate.

## Roadmap

### Phase 0: Decisions, schemas, and scaffolding (1-2 days)

Deliverables:
- Define the integration contract and schema IDs.
- Choose the default path for OpenClaw -> Autopilot invocation.
- Establish naming, config, and security defaults.

Tasks:
1) Decide the primary invocation model:
   - Option A: OpenClaw plugin tool shells `autopilot run` (fastest).
   - Option B: OpenClaw Gateway RPC method + service (better for long jobs).
   - Option C: Autopilot as Gateway node (device-style).

2) Define a stable request/response schema for `autopilot.run`.
   - Required inputs: `task`, `repo_path`, `access`, `timeout_seconds`, `lane`, `model`.
   - Optional inputs: `autopilot_loop`, `allowlist`, `env_overrides`, `openclaw_session_id`.
   - Output: `session_id`, `status`, `summary`, `artifact_paths`, `verification`, `diff_stats`.

3) Define a progress event schema (shared between Autopilot -> OpenClaw).
   - `run_id`, `session_id`, `thread_id`, `turn_id`
   - `phase` (guidance/status/step/response/tool/verify)
   - `signature`, `summary`, `confidence`, `timestamp`

4) Decide where the OpenClaw client lives in OpenAgents.
   - Recommended: new crate `crates/openclaw_client` (Gateway WS + HTTP helper).

Acceptance criteria:
- Schema and naming decisions documented.
- ADR drafted if new contracts are introduced.

### Phase 1: OpenClaw -> Autopilot (baseline execution path)

Goal: OpenClaw can invoke Autopilot and receive a structured result.

OpenClaw repo tasks:
- Implement a plugin tool `autopilot.run` (optional/allowlisted).
- Execute `autopilot run` with explicit args and timeout.
- Parse Verified Patch Bundle output from `${OPENAGENTS_HOME}/sessions/{session_id}/`.
- Return structured summary + artifact paths.

OpenAgents repo tasks (if needed):
- Ensure `autopilot run` emits a machine-readable summary or writes a stable session id to stdout.
- Add a helper command if necessary: `autopilot session show <id> --json`. (implemented)
- Make sure `OPENAGENTS_HOME` is resolved centrally and documented.

Acceptance criteria:
- OpenClaw tool can run Autopilot successfully in a local repo.
- Tool output includes session id + artifact locations.
- Verified Patch Bundle is produced and read without manual steps.

### Phase 2: Autopilot -> OpenClaw (progress streaming)

Goal: Autopilot streams progress into OpenClaw sessions in near-real-time.

Tasks:
1) Add an OpenClaw bridge in Autopilot Desktop:
   - Subscribe to `AppEvent` stream and `guidance/*` events.
   - Forward to OpenClaw Gateway WS or `/tools/invoke` with `message` or `sessions_send`.
   - Throttle `guidance/status` (debounce 1-2s) and emit `guidance/step` immediately.

2) Provide mapping from Autopilot run -> OpenClaw session:
   - Store `openclaw_session_id` in run metadata (env or config).
   - Attach `run_id` and `session_id` to all outgoing events.

3) Add a small OpenClaw plugin endpoint (optional) to receive structured progress events:
   - `autopilot.progress` RPC method that turns events into UI status cards or messages.

Acceptance criteria:
- OpenClaw shows live progress lines for a running Autopilot job.
- Progress events are signed/logged (at least via existing OpenClaw message logs).

### Phase 3: Approvals and user input (true two-way)

Goal: OpenClaw can approve Autopilot actions and provide user input mid-run.

Tasks:
1) Add an approval bridge in Autopilot:
   - When tool approval is requested, send a structured approval request to OpenClaw.
   - Block or pause Autopilot until a response is received.

2) Define approval response schema:
   - `run_id`, `turn_id`, `approval_id`, `decision` (approve/deny), `reason`.

3) Update Autopilot Desktop execution flow:
   - Use `AskForApproval::OnDemand` (or equivalent) instead of `Never` when OpenClaw integration is enabled.
   - Route approval responses back into the Codex app-server client.

Acceptance criteria:
- A tool approval request appears in OpenClaw and can be approved/denied.
- Autopilot continues or aborts based on the response.

### Phase 4: Job lifecycle + long-running orchestration

Goal: OpenClaw manages Autopilot jobs as first-class entities.

Tasks:
- Add OpenClaw plugin service to track job state, progress, artifacts, and timestamps.
- Implement `autopilot.status`, `autopilot.cancel`, `autopilot.jobs.list`.
- Persist linkage: OpenClaw job id <-> Autopilot session id.

Acceptance criteria:
- OpenClaw can list active and recent Autopilot jobs.
- Cancel requests terminate Autopilot runs cleanly.

### Phase 5: Autopilot as Gateway node (device-style integration)

Goal: Autopilot is discoverable and controllable as a node in OpenClaw.

Tasks:
- Implement node handshake and `autopilot.*` commands (run/status/cancel).
- Leverage OpenClaw existing node tool to route commands.
- Use node permissions and pairing flow for security.

Acceptance criteria:
- OpenClaw nodes tool can invoke Autopilot commands.
- Autopilot node reports capabilities and status.

### Phase 6: Artifact ingestion + replay UX

Goal: OpenClaw renders Verified Patch Bundles and replays.

Tasks:
- Add OpenClaw UI rendering for `PR_SUMMARY.md` plus receipt metadata.
- Provide links to `REPLAY.jsonl` or a small replay viewer.
- Optionally store artifact hashes in OpenClaw session metadata for audit.

Acceptance criteria:
- OpenClaw displays patch summary and verification results.
- Replay artifacts are discoverable and linked to the OpenClaw session.

### Phase 7: Hardening, security, and policy

Goal: Safe by default, auditable, and bounded.

Tasks:
- Explicit allowlists for `autopilot.run` and any OpenClaw -> Autopilot tool.
- Configurable repo access policies (read-only vs full access).
- Ensure tool schema validation is enforced at execution runtime.
- Emit receipts + hashes for all tool calls.

Acceptance criteria:
- Integrations respect tool policy, sandbox mode, and allowlists.
- Any unsafe action requires explicit user approval.

## Proposed integration interfaces (concrete)

### `autopilot.run` tool (OpenClaw plugin)

Input (JSON):
- `task` (string, required)
- `repo_path` (string, required)
- `access` (enum: read_only | full)
- `timeout_seconds` (number)
- `model` (string)
- `lane` (string)
- `openclaw_session_id` (string)

Output (JSON):
- `session_id` (string)
- `status` (enum: success | failed | cancelled | timeout)
- `summary` (string)
- `artifacts` (object)
  - `pr_summary_path`
  - `receipt_path`
  - `replay_path`
- `verification` (object)
  - `commands`
  - `exit_codes`
  - `verification_delta`
- `diff_stats` (object)
  - `files_changed`
  - `lines_added`
  - `lines_removed`

Schemas:
- Params: `docs/openclaw/schemas/autopilot.run.params.json`
- Response: `docs/openclaw/schemas/autopilot.run.response.json`

Plugin example:
- `docs/openclaw/plugin-examples/autopilot-tools.md`

### `autopilot.progress` event (Autopilot -> OpenClaw)

Payload:
- `run_id`
- `session_id`
- `thread_id`
- `turn_id`
- `phase` (guidance/status/step/response/tool/verify)
- `signature`
- `summary`
- `confidence`
- `timestamp`

Schema:
- `docs/openclaw/schemas/autopilot.progress.metadata.json`

### `autopilot.approval` tool (OpenClaw plugin)

Input (JSON):
- `session_id` (string, required)
- `run_id` (string, required)
- `request_id` (string, required)
- `method` (string, required)
- `params` (object, required) — original app-server request params

Output (JSON):
- Approval decision:
  - `decision` (enum: accept | acceptForSession | decline | cancel)
  - Optional `acceptSettings` if `acceptForSession`
- Or tool input:
  - `answers` object mirroring Codex tool input shape

Schemas:
- Params: `docs/openclaw/schemas/autopilot.approval.params.json`
- Response: `docs/openclaw/schemas/autopilot.approval.response.json`

## Configuration and feature flags

Recommended env/config toggles:

- `OPENCLAW_GATEWAY_URL`
- `OPENCLAW_API_TOKEN`
- `OPENCLAW_SESSION_ID`
- `OPENCLAW_PROGRESS_MODE` (ws | http | off)
- `OPENCLAW_PROGRESS_URL` (override POST endpoint for progress; default `${OPENCLAW_GATEWAY_URL}/tools/invoke`)
- `OPENCLAW_PROGRESS_TOOL` (default `sessions_send`)
- `OPENCLAW_PROGRESS_DEBOUNCE_MS` (default 1200)
- `OPENCLAW_PROGRESS_INCLUDE_METADATA` (default off)
- `OPENCLAW_APPROVALS_MODE` (on | off)
- `OPENCLAW_APPROVALS_URL` (default `${OPENCLAW_GATEWAY_URL}/tools/invoke`)
- `OPENCLAW_APPROVALS_TOOL` (default `autopilot.approval`)
- `OPENCLAW_APPROVALS_TIMEOUT_MS` (default 30000)

## Test plan (minimum)

OpenClaw repo:
- Plugin tool schema tests.
- Tool execution failure modes (timeout, missing artifacts).
- Progress event rendering tests.

OpenAgents repo:
- Unit tests for progress event mapper.
- Replay/test harness verifying no panics when progress is enabled.
- Integration test that runs a short Autopilot job and forwards one progress event.

## Open questions

1) Should OpenClaw be the authoritative store for job metadata, or should it only reference Autopilot session ids?
2) Do we want a single OpenClaw session per Autopilot run, or a single OpenClaw session that aggregates multiple Autopilot runs?
3) How should OpenClaw approvals map onto Autopilot approval types (exec, patch, tool input)?
4) Should Autopilot be allowed to call OpenClaw tools directly, or only via a restricted bridge?
5) What is the default trust boundary for repo paths and workspace access?

## Suggested first milestone (1-week slice)

- Implement OpenClaw plugin tool `autopilot.run` (Phase 1).
- Add Autopilot progress bridge for `guidance/status` + `guidance/step` (Phase 2).
- Publish a short skill in OpenClaw workspace that teaches when to use `autopilot.run`.

Acceptance:
- User can request a code task in OpenClaw and see Autopilot progress and final artifacts.
- Verified Patch Bundle is surfaced in OpenClaw with a link to PR_SUMMARY.md.
