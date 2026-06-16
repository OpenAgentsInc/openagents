# Current Apple FM Runtime And Electrobun Desktop Integration Audit

Date: 2026-06-15

Status: current-code audit focused on the Apple Foundation Models runtime that
exists now and the work required to connect it to Autopilot Desktop, the
Electrobun app in `apps/autopilot-desktop`.

## Bottom Line

The current codebase already has the Apple FM backend contract in Pylon runtime.
It now also has a token-authenticated Pylon control projection,
`apple_fm.status`, that reports live Apple FM readiness from the existing
runtime health/capability code, plus a buildable in-tree Swift Foundation
Models bridge helper at `apps/pylon/swift/foundation-bridge`. Autopilot
Desktop now consumes the Pylon readiness projection through Bun-owned RPC and
renders hosted OpenAgents compute separately from local Apple FM mode. It now
has a Desktop-originated bounded local Apple FM chat/tool session path through
Pylon with fake-bridge tests, public-safe event summaries, and admitted-Mac
smoke evidence. The retained Apple FM session proof now also includes a modeled
local-session power/kWh estimate for issue #5074. It does not yet have
bridge-helper launch/supervision in the signed installer path, so broad
installer copy still must stay yellow.

The integration path should be:

1. Autopilot Desktop launches or adopts the local Pylon node.
2. Pylon runtime checks a local Apple FM bridge at
   `http://127.0.0.1:11435` through `apple_fm_bridge`.
3. Pylon publishes a public-safe backend capability/readiness projection.
4. The Electrobun Bun host reads that projection over loopback and exposes only
   public-safe readiness to the Foldkit webview.
5. The webview shows Apple FM as a local backend option only when live health
   is ready, not merely because the machine is Apple Silicon.

Do not wire the webview directly to the Foundation Models bridge. The existing
desktop boundary is intentional: secrets, loopback tokens, helper process
ownership, and backend health checks belong in Bun/Pylon, while the webview
renders safe projections.

## Current Desktop Host

Autopilot Desktop lives at `apps/autopilot-desktop`. Its local contract is:

- `apps/autopilot-desktop/electrobun.config.ts` defines the app, webview entry,
  Bun entry, auto-update feed, and resource copy list.
- `apps/autopilot-desktop/src/bun/index.ts` is the Electrobun Bun main process.
  It creates the `BrowserWindow`, defines the typed RPC handlers, launches or
  adopts the Pylon node, polls node state, and owns private environment
  material.
- `apps/autopilot-desktop/src/bun/node-launcher.ts` resolves a dev Pylon entry
  or packaged Pylon bundle and launches it into a managed `.pylon-local` home.
- `apps/autopilot-desktop/src/bun/pylon-control.ts` talks to Pylon over
  loopback using the control token and exposes read-only projections plus
  scoped command verbs.
- `apps/autopilot-desktop/src/shared/rpc.ts` defines the webview to Bun RPC
  schema.
- `apps/autopilot-desktop/src/shared/install-readiness.ts` projects first-run
  health from node lifecycle, built-in-agent readiness, and auto-update state.
- `apps/autopilot-desktop/src/ui/bridge.ts`,
  `apps/autopilot-desktop/src/ui/commands.ts`, and
  `apps/autopilot-desktop/src/ui/view.ts` connect the Foldkit webview to the
  typed RPC surface.

The desktop has the right seam for Apple FM because the Bun host owns all
privileged local work. #5071 now adds Apple FM as another public-safe
readiness/capability projection beside the existing local-node and built-in
hosted-agent projections.

## Current Desktop Behavior

The current app does these things well:

- It launches or adopts Pylon and keeps the launched node supervised.
- It uses a managed Pylon home for packaged installs:
  `~/.openagents/autopilot-desktop/.pylon-local`.
- It surfaces local-node lifecycle honestly with statuses like `launching`,
  `online`, `adopted`, `failed`, and `unavailable`.
- It exposes `installReadiness` so normal first-run failures appear in
  Settings.
- It exposes a no-user-key built-in agent path through hosted OpenAgents
  compute.
- It calls Pylon control `apple_fm.status` through Bun-owned RPC and exposes
  `appleFmReadiness` to the webview.
- It includes local Apple FM as an optional first-run readiness item without
  making optional local mode block hosted compute.
- It shows hosted OpenAgents compute and local Apple FM as distinct Agent pane
  modes, with local blocker refs visible.
- It can start a bounded local Apple FM session through Bun-owned Pylon control
  when live Apple FM readiness is ready.
- It keeps hosted compute credentials and Pylon control tokens out of the
  webview.
- It has an admitted-Mac smoke runbook and retained public-safe evidence for a
  local `read_file` chat/tool session with no hosted model prompt path.
- It retains modeled Apple FM local-session energy evidence
  (`energyEstimate.evidenceState: "modeled"`, default `modeledPowerKw: 0.02`)
  in Pylon control-session proof artifacts.

The current app does not yet do these Apple FM-specific things:

- It does not bundle, launch, or supervise the Foundation Models bridge helper.
- It does not advertise local Apple FM as the default backend for "Go online"
  yet.

## Current Pylon Runtime Apple FM Implementation

The current Apple FM backend implementation is in Pylon runtime:

- `apps/pylon/packages/runtime/src/backends/apple-fm/contract.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/client.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/tools.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/blueprint-tools.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/program-run-evidence.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/receipts.ts`
- `apps/pylon/packages/runtime/src/fleet/backend-capability.ts`
- `apps/pylon/packages/runtime/src/cli.ts`

The backend identity is:

- backend kind: `apple_fm_bridge`
- profile id: `apple-fm-local`
- model id: `apple-foundation-model`
- default base URL: `http://127.0.0.1:11435`
- attach mode: `attach_existing`
- auth mode: `none`
- readiness path: `/health`
- stream mode: `snapshot`
- capability ref: `probe.backend.apple_fm_bridge`

The profile resolver accepts:

1. explicit `--base-url` or runtime override
2. `PROBE_APPLE_FM_BASE_URL`
3. `OPENAGENTS_APPLE_FM_BASE_URL`
4. the default loopback URL

The local Swift bridge helper is now retained under:

- `apps/pylon/swift/foundation-bridge/Package.swift`
- `apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift`
- `apps/pylon/swift/foundation-bridge/build.sh`
- `apps/pylon/bin/foundation-bridge`

It exposes `/health`, `/v1/models`, `/v1/chat/completions`, `/v1/sessions`,
and `/v1/sessions/{id}/responses/stream` on loopback. The first restored
version is intentionally modest: it creates fresh Foundation Models sessions,
returns typed unavailable reasons for disabled or unsupported Apple FM states,
can invoke the projected read-only `read_file` callback for Pylon's local
tool/chat MVP, and emits single-turn snapshot/completed SSE events. It logs
startup/listener state only, not prompts, message bodies, local files, or
secrets.

Pylon source/package discovery for the helper now lives in:

- `apps/pylon/src/node/apple-fm-bridge-helper.ts`
- `apps/pylon/tests/apple-fm-bridge-helper.test.ts`

The runtime client supports:

- `health()`
- `requireReady()`
- `completePlainText(messages)`
- `streamPlainTextSnapshots(messages)`
- `streamSessionWithTools(input)`
- `smoke(prompt)`

The current Pylon app exposes Apple FM readiness through the token-authenticated
control command rather than a user-facing `pylon apple-fm status` shell alias.
Development smokes can call the bridge directly or use the runtime capability
reporter, but the desktop should not shell out for its normal live projection
when Pylon can expose the same status through the control API.

## Current Runtime Semantics

The runtime implementation is attach-only. It assumes some local process is
already serving the Foundation Models bridge.

`makeAppleFmClient` checks live health by calling the profile readiness path.
The health result is typed as:

- `ready`
- `unavailable`
- `unsupported`
- `malformed`
- `unreachable`

Unavailable reasons include:

- `bridge_unreachable`
- `apple_intelligence_disabled`
- `unsupported_hardware`
- `model_unavailable`
- `permission_denied`
- `malformed_response`
- `not_ready`
- `unknown`

The runtime already carries the right honesty model:

- A status command does not run inference.
- A smoke command runs inference only after `requireReady()`.
- Receipts redact backend URLs and content where needed.
- Usage truth is explicit: `exact`, `estimated`, or `unknown`.
- Capability reporting advertises `probe.backend.apple_fm_bridge` only when
  live health is ready and Blueprint projection is safe.

## Tool Callback Implementation

`apps/pylon/packages/runtime/src/backends/apple-fm/tools.ts` implements a
session-local callback server for Apple FM tool use.

Retained tool names are:

- `read_file`
- `list_files`
- `code_search`
- `shell`
- `apply_patch`
- `consult_oracle`
- `analyze_repository`
- `propose_action_submission`

Tool policy is one of:

- `allow`
- `approval_required`
- `deny`

The callback layer validates tokens, enforces round-trip limits, records
transcript entries, emits redacted callback receipts, and supports the Swift
bridge callback payload shape with `session_token`, `tool_name`, and
`arguments`.

For desktop integration, this means Apple FM can eventually be a local
tool-backed backend, but the desktop must keep tool callback URLs and callback
tokens inside Pylon/Bun. The webview should receive only a redacted capability
or run summary.

## Blueprint Projection

`apps/pylon/packages/runtime/src/backends/apple-fm/blueprint-tools.ts` maps
Probe/Pylon Blueprint tool menus into Apple FM tool definitions.

Current important constraints:

- Tool schemas must have `additionalProperties: false`.
- Root object schema metadata is normalized for Apple FM.
- Supported projected Probe tools are currently `read_file`, `code_search`, and
  `propose_action_submission`.
- `record_evidence` is intentionally not projected to Apple FM.
- Approval policy is preserved.

For desktop, this means "Apple FM local backend" should not be presented as a
generic unrestricted agent. It is a constrained local model route that becomes
eligible only for tool menus Pylon can safely project.

## Capability Reporting

`apps/pylon/packages/runtime/src/fleet/backend-capability.ts` is the most
important current file for Autopilot Desktop.

`reportAppleFmBackendCapability`:

- builds an Apple FM client,
- checks live bridge health,
- checks Blueprint backend support,
- emits a redacted capability report,
- advertises `probe.backend.apple_fm_bridge` only if live health and safe
  projection are both true.

This is the right truth source for desktop readiness.

The desktop should eventually consume a Pylon control projection derived from
this report, not from static platform checks.

## Current Pylon Inventory Caveat

`apps/pylon/src/inventory.ts` already has a `backend.apple_fm` row. However,
`discoverHostInventory` currently sets `appleFmReady` from platform and arch:
Darwin arm64 becomes ready without checking the live Foundation Models bridge.

That is not strong enough for the desktop green path.

Use inventory for coarse platform context:

- Darwin arm64 can mean "candidate machine."
- Non-Darwin or non-arm64 can mean "unsupported."

Do not use inventory alone to say Apple FM is ready. The green state must come
from live bridge health through the runtime client or capability report.

## Current Swift Bridge Helper

The current tree retains the runtime client and tests, and now also has a
restored, buildable in-tree Swift bridge helper:

- `apps/pylon/swift/foundation-bridge/Package.swift`
- `apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift`
- `apps/pylon/swift/foundation-bridge/build.sh`
- `apps/pylon/bin/foundation-bridge`
- `apps/pylon/src/node/apple-fm-bridge-helper.ts`
- `apps/pylon/tests/apple-fm-bridge-helper.test.ts`

The helper implements the expected TypeScript runtime contract:

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`
- `POST /v1/sessions`
- `POST /v1/sessions/{id}/responses/stream`

The remaining desktop packaging choices are:

1. Package the restored helper with the app and let desktop/Pylon launch it.
2. Support a signed external helper that desktop/Pylon only discovers.
3. Let Pylon own bridge launch/discovery as part of its node runtime, and desktop
   merely displays Pylon's projection.

The recommended route remains 1 plus 3: package the helper, but let Pylon own
the backend health and capability truth.

## Electrobun Packaging Work Needed

`apps/autopilot-desktop/electrobun.config.ts` currently copies:

- webview HTML
- generated CSS
- a built Pylon node entry at `resources/pylon-node/index.js`

There is no Apple FM helper resource. If the bridge is bundled, add a resource
copy such as:

- `resources/apple-fm-bridge/FoundationBridge` to
  `apple-fm-bridge/FoundationBridge`

Exact names should follow the actual restored Swift package and release
artifact.

Packaged launch must respect:

- macOS-only support.
- arm64 Apple Silicon requirement.
- hardened runtime and notarization.
- no bridge launch on unsupported platforms.
- no false ready state if the helper is missing.
- graceful status if Apple Intelligence is disabled.

The bridge launch policy should be explicit:

- Autostart default for packaged macOS arm64 builds if helper is bundled.
- No autostart in source/dev unless configured or helper path is found.
- Override path through an env var such as `OPENAGENTS_APPLE_FM_BRIDGE_PATH`.
- Backend URL override through existing `OPENAGENTS_APPLE_FM_BASE_URL` or
  `PROBE_APPLE_FM_BASE_URL`.

## Pylon Control API Now Available

Autopilot Desktop should not run a Pylon Apple FM status subprocess on every
refresh. #5070 added a Pylon control command that returns the same public-safe
readiness shape through the existing token-authenticated `POST /command`
loopback path.

Command:

- `apple_fm.status`

Current response fields:

- `schema`: `openagents.pylon.apple_fm.status.v0.1`
- `kind`: `pylon_apple_fm_status`
- `runnerId`
- `runnerKind`
- `backendKind`
- `profileId`
- `model`
- `capability`
- `advertisedCapabilities`
- `available`
- `status`
- `baseUrl`
- `platform`
- `version`
- `unavailableReason`
- `message`
- `requirements`
- `support`
- `blueprintSupport`
- `receipt`
- `blockerRefs`
- `observedAt`
- `contentRedacted`

The implementation lives in:

- `apps/pylon/src/node/apple-fm-status.ts`
- `apps/pylon/src/node/control-server.ts`
- `apps/pylon/tests/control-protocol.test.ts`

It is backed by `reportAppleFmBackendCapability`, which itself calls live
`/health`. Tests cover ready, unsupported, unreachable, and malformed health.
The command does not run inference and does not advertise
`probe.backend.apple_fm_bridge` unless live health and safe projection support
are ready.

If command names need to align with existing command style, use the same
pattern as `accounts.list`, `wallet.status`, `assignments.poll`, and
`coordinator.status`.

## Desktop Bun Work Needed

Add Apple FM projection support in the Bun main process:

- Add a `fetchAppleFmReadiness` helper in
  `apps/autopilot-desktop/src/bun/pylon-control.ts`.
- Add an `AppleFmReadinessResponse` type in
  `apps/autopilot-desktop/src/shared/rpc.ts`.
- Add an `appleFmReadiness` RPC request to `DesktopRPCSchema`.
- In `apps/autopilot-desktop/src/bun/index.ts`, implement the RPC handler by
  reading the Pylon control token and forwarding the Pylon control command.
- Fail closed when the local Pylon node or control token is unavailable.
- Redact base URL and helper paths before crossing into the webview.

If the desktop owns helper launch, add a new Bun module instead of mixing it
into `index.ts`, for example:

- `apps/autopilot-desktop/src/bun/apple-fm-helper.ts`

That module should handle:

- bridge binary discovery,
- platform gating,
- launch and stop,
- port/base URL selection,
- readiness timeout,
- status transitions,
- packaged resource path resolution,
- process cleanup on app close.

Keep bridge helper lifecycle distinct from Pylon node lifecycle. Pylon node
online is necessary but not sufficient for Apple FM ready.

## Desktop Shared Readiness Work Needed

Extend first-run health:

- Add Apple FM readiness as an optional item in
  `apps/autopilot-desktop/src/shared/install-readiness.ts`.
- Add blocker refs such as:
  - `blocker.autopilot.apple_fm.unsupported_platform`
  - `blocker.autopilot.apple_fm.bridge_missing`
  - `blocker.autopilot.apple_fm.bridge_unreachable`
  - `blocker.autopilot.apple_fm.apple_intelligence_disabled`
  - `blocker.autopilot.apple_fm.model_unavailable`
  - `blocker.autopilot.apple_fm.health_unproven`
- Do not make Apple FM required for the whole app to be healthy unless the user
  explicitly selects a local Apple FM mode.
- If Apple FM is a candidate but not ready, show it as attention or blocked for
  local-backend readiness, not as a global install failure.

The current `highestRoiAction` centers hosted compute. It should become mode
aware:

- If hosted compute is configured, keep "Go online."
- If local Apple FM is ready and local mode is selected, "Go online locally."
- If Apple FM is the selected local backend but not ready, point to the highest
  blocker: start bridge, enable Apple Intelligence, or use hosted compute.

## Desktop Webview Work Needed

The webview must stay Foldkit and shared UI first, per
`apps/autopilot-desktop/AGENTS.md`.

Required UI changes:

- Add the new RPC method to `apps/autopilot-desktop/src/ui/bridge.ts`.
- Add a Foldkit command in `apps/autopilot-desktop/src/ui/commands.ts`.
- Add model/message fields in `apps/autopilot-desktop/src/ui/model.ts` and
  `apps/autopilot-desktop/src/ui/message.ts`.
- Update the Agent pane in `apps/autopilot-desktop/src/ui/view.ts` to show:
  - hosted OpenAgents compute readiness,
  - local Apple FM readiness,
  - selected lane/mode,
  - blocker refs.
- Update Settings first-run health to include Apple FM when applicable.

Avoid a separate hand-rolled Apple FM panel. If a reusable readiness component
is needed, add it to `@openagentsinc/autopilot-ui` so the web surface can reuse
it later.

## Go Online Semantics

Today, "Go online" in desktop means starting a bounded hosted Codex session on
OpenAgents compute through the local Pylon node. It is not a local Apple FM
session.

Apple FM should add a separate local route, not silently change the hosted
route.

Recommended model:

- `hosted`: current no-user-key hosted compute path.
- `local-apple-fm`: local Foundation Models bridge through Pylon runtime.
- `auto`: prefer hosted unless local Apple FM is explicitly ready and selected,
  or use Pylon's own lane policy if that policy becomes authoritative.

The UI copy should stay honest:

- "OpenAgents hosted" for hosted compute.
- "Local Apple FM" for local Apple Foundation Models.
- "Apple Silicon candidate" for hardware-only detection.
- "Ready" only after live bridge health passes.

## Assignment And Market Readiness

Apple FM becomes market-eligible only after Pylon runtime state carries the
right capability refs.

Current assignment admission checks `state.runtime.capabilityRefs` and rejects
leases with missing required capabilities or unsupported backend refs. Pylon
provider `go-online` currently adds local Codex, local Claude Agent, Tassadar,
NIP-90, labor, and workspace materializer capabilities. It does not add Apple
FM capability refs.

Needed Pylon work:

- On provider go-online, run Apple FM capability reporting.
- If live health and Blueprint projection are safe, add
  `probe.backend.apple_fm_bridge` to runtime capability refs.
- If not ready, keep it out of advertised capabilities and add public-safe
  blocker refs.
- Ensure presence heartbeat publishes only capability-true Apple FM refs.
- Add assignment admission tests for Apple FM-required leases.

Do not advertise Apple FM capability based on `discoverHostInventory` alone.

## Security And Privacy Boundaries

Keep these boundaries:

- Webview receives public-safe readiness only.
- Pylon control token stays in Bun.
- Bridge callback tokens stay in Pylon runtime.
- Raw callback URLs stay redacted in descriptors, receipts, and webview state.
- Bridge helper paths should not leak local usernames when projected.
- Program Run evidence remains content-redacted unless a future explicit
  release policy says otherwise.
- Apple FM local runs remain no-spend by default unless routed through a
  separate assignment/settlement policy.
- Apple FM local-session kWh is denominator evidence only; it is not measured
  telemetry and not AO/kWh unless joined to a verified accepted-outcome receipt.

This matches the current Apple FM receipt model and the desktop AGENTS rule that
secrets stay in the Bun main process.

## Test Plan

CI-safe tests:

- Pylon runtime fake bridge tests for ready, unsupported, malformed, and
  unreachable status.
- Pylon control command tests for `apple_fm.status`.
- Desktop `pylon-control.ts` tests with a fake control server.
- Desktop `DesktopRPCSchema` and bridge tests for `appleFmReadiness`.
- Pylon/Desktop fake-bridge tests for `apple_fm.session.start`, read-only tool
  success, not-ready refusal, unsupported-tool refusal, and event redaction.
- Pylon tests for modeled Apple FM session kWh, unavailable energy estimate,
  and "not measured" proof semantics.
- `install-readiness.ts` tests for Apple FM ready, unsupported, bridge missing,
  and Apple Intelligence disabled states.
- Foldkit view tests that the Agent pane and Settings pane show local Apple FM
  status without exposing tokens or local secret paths.
- Node launcher/helper tests if desktop owns bridge process launch.

Live admitted-Mac tests:

- Start or install the Foundation Models bridge.
- Run Pylon `apple_fm.status` over the loopback control API.
- Run a direct bridge `/v1/chat/completions` smoke.
- Run the Pylon runtime local chat/tool session runner.
- Start Autopilot Desktop with the same base URL.
- Verify Settings shows Apple FM ready.
- Verify the Agent pane can select local Apple FM only when ready.
- Verify unsupported or disabled Apple Intelligence appears as unavailable, not
  as a generic failure.

Packaging tests:

- `bun run --cwd apps/autopilot-desktop build:canary`
- Confirm Pylon node bundle is copied.
- Confirm Apple FM bridge helper is copied if shipping local Apple FM.
- Confirm notarized macOS build can launch helper under hardened runtime.
- Confirm missing helper yields an honest blocker, not a crash or false ready.

## Minimal Implementation Sequence

1. Done: add Pylon `apple_fm.status` control command backed by current runtime
   capability/health code.
2. Done: restore a buildable Swift Foundation Models bridge helper and Pylon
   helper discovery rules.
3. Done: add desktop Bun fetch/RPC support for Apple FM readiness.
4. Done: extend desktop install readiness and Agent pane with Apple FM as an
   optional local backend.
5. Done: add a Desktop-originated bounded local Apple FM chat/tool session
   runner through Pylon control with safe read-only workspace tools.
6. Done: add fake-bridge desktop loopback coverage and admitted-Mac local
   chat/tool smoke evidence.
7. Done: add modeled Apple FM local-session power/kWh estimate to retained
   proof artifacts and admitted-Mac smoke evidence.
8. Add provider go-online capability declaration for
   `probe.backend.apple_fm_bridge`, gated by live health.

This order avoids presenting a UI promise before the control and runtime truth
sources exist.

## Non-Goals

- Do not revive the old Swift iOS operator app as the desktop integration path.
- Do not let the webview call `127.0.0.1:11435` directly.
- Do not mark Apple FM ready from hardware detection alone.
- Do not claim Apple FM replaces Codex or hosted OpenAgents compute.
- Do not claim "earn Bitcoin with Apple FM" until Pylon capability advertising,
  assignment acceptance, receipts, payout path, and settlement evidence are all
  green.

## Current State Classification

Current implementation: yellow.

What is real:

- Pylon runtime Apple FM client.
- Typed health/readiness.
- Plain-text smoke.
- Snapshot streaming.
- Tool callbacks.
- Blueprint tool projection.
- Program Run evidence.
- Capability report gated on live health.
- Pylon `apple_fm.status` loopback control command with fake-bridge tests.
- Buildable Swift Foundation Models bridge helper.
- Pylon source/package bridge-helper discovery tests.
- Desktop `appleFmReadiness` RPC through Bun-owned Pylon control.
- Desktop first-run health item for optional local Apple FM.
- Desktop Agent pane hosted/local mode cards with blocker refs.
- Pylon `apple_fm.session.start` local session path using read-only workspace
  tools and public-safe summaries.
- Desktop `startAppleFmSession` RPC/control command path.
- Fake-bridge coverage for ready session success, not-ready refusal,
  unsupported-tool refusal, and redaction.
- Desktop fake-bridge loopback integration coverage.
- Admitted-Mac source smoke evidence for bridge health, Pylon/Desktop
  readiness, one local `read_file` chat/tool session, no cloud runner, no
  resource usage receipt, read-only sandbox, disabled handling, and redaction.
- Modeled Apple FM local-session power/kWh proof metadata and admitted-Mac
  smoke summary: 0.02 kW default model over retained wall-clock, explicitly not
  measured telemetry and not AO/kWh by itself.

What is not connected yet:

- Packaged helper resource.
- Provider go-online capability declaration.
- Signed/notarized installer helper launch/supervision and from-install smoke.

The next honest milestone is not "source Apple FM works in desktop." It is:

a signed/notarized Autopilot Desktop recut that bundles or supervises the
helper, launches from a normal install, repeats the admitted-Mac local
chat/tool smoke, and keeps the same public-safe evidence boundary.
