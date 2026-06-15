# Current Apple FM Runtime And Electrobun Desktop Integration Audit

Date: 2026-06-15

Status: current-code audit focused on the Apple Foundation Models runtime that
exists now and the work required to connect it to Autopilot Desktop, the
Electrobun app in `apps/autopilot-desktop`.

## Bottom Line

The current codebase already has the Apple FM backend contract in Pylon runtime.
It does not yet have an in-tree Swift Foundation bridge binary, a packaged
bridge helper, or a desktop UI/control projection that lets Autopilot Desktop
truthfully say "local Apple FM is ready."

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

The desktop already has a good seam for Apple FM because the Bun host owns all
privileged local work. The missing part is adding Apple FM as another
public-safe readiness/capability projection beside the existing local-node and
built-in hosted-agent projections.

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
- It keeps hosted compute credentials and Pylon control tokens out of the
  webview.

The current app does not yet do these Apple FM-specific things:

- It does not bundle, launch, or supervise a Foundation Models bridge helper.
- It does not call Pylon runtime `apple-fm status`.
- It does not expose `appleFmReadiness` or `localBackendReadiness` in
  `DesktopRPCSchema`.
- It does not include Apple FM in `InstallReadinessResponse.items`.
- It does not show Apple FM health in the Agent pane.
- It does not advertise local Apple FM as the backend for "Go online."
- It does not distinguish "Apple Silicon present" from "Foundation Models
  bridge live and Apple Intelligence enabled."

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

The runtime client supports:

- `health()`
- `requireReady()`
- `completePlainText(messages)`
- `streamPlainTextSnapshots(messages)`
- `streamSessionWithTools(input)`
- `smoke(prompt)`

The CLI exposes direct Pylon aliases through `apps/pylon/src/index.ts`:

- `pylon apple-fm status [--base-url URL] [--profile apple-fm-local]`
- `pylon apple-fm smoke [--base-url URL] [--profile apple-fm-local]
  [--prompt TEXT]`
- `pylon apple-fm tool-stream-demo [--base-url URL] [--path FILE]
  [--prompt TEXT]`

Those commands are routed into the runtime CLI through `runProbeCli`. The
current CLI is useful for development and acceptance, but the desktop should not
shell out for its normal live projection if Pylon can expose the same status
through the control API.

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

## Current Missing Swift Bridge

The current tree retains the runtime client and tests, but not the old in-tree
Swift bridge implementation. The expected bridge contract still exists in the
TypeScript runtime:

- `GET /health`
- `POST /v1/chat/completions`
- snapshot streaming for `streamMode: "snapshot"`
- `POST /v1/sessions`
- `POST /v1/sessions/{id}/responses/stream`
- loopback tool callback support

To ship desktop Apple FM, one of these must become true:

1. A Foundation Models bridge helper is restored into the repo and packaged
   with the app.
2. A signed external helper is installed separately and the desktop only
   discovers it.
3. Pylon owns bridge launch/discovery as part of its node runtime, and desktop
   merely displays Pylon's projection.

The recommended route is 1 plus 3: restore/package the helper, but let Pylon
own the backend health and capability truth.

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

## Pylon Control API Work Needed

Autopilot Desktop should not need to run `pylon apple-fm status` as a subprocess
on every refresh. Add a Pylon control command that returns the same public-safe
readiness shape.

Suggested command:

- `apple_fm.status`

Suggested response fields:

- `ok`
- `fetchedAt`
- `sourceUrl`
- `backendKind`
- `profileId`
- `model`
- `capabilityRef`
- `available`
- `status`
- `baseUrl`
- `baseUrlRedacted`
- `platform`
- `version`
- `unavailableReason`
- `message`
- `advertisedCapabilities`
- `blockerRefs`
- `contentRedacted`

This command should be implemented in Pylon using
`reportAppleFmBackendCapability` or `makeAppleFmClient().health()`, not by
duplicating bridge parsing in desktop.

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

This matches the current Apple FM receipt model and the desktop AGENTS rule that
secrets stay in the Bun main process.

## Test Plan

CI-safe tests:

- Pylon runtime fake bridge tests for ready, unsupported, malformed, and
  unreachable status.
- Pylon control command tests for `apple_fm.status`.
- Desktop `pylon-control.ts` tests with a fake control server.
- Desktop `DesktopRPCSchema` and bridge tests for `appleFmReadiness`.
- `install-readiness.ts` tests for Apple FM ready, unsupported, bridge missing,
  and Apple Intelligence disabled states.
- Foldkit view tests that the Agent pane and Settings pane show local Apple FM
  status without exposing tokens or local secret paths.
- Node launcher/helper tests if desktop owns bridge process launch.

Live admitted-Mac tests:

- Start or install the Foundation Models bridge.
- Run `pylon apple-fm status`.
- Run `pylon apple-fm smoke`.
- Run `pylon apple-fm tool-stream-demo --path README.md`.
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

1. Add Pylon `apple_fm.status` control command backed by current runtime
   capability/health code.
2. Add desktop Bun fetch/RPC support for Apple FM readiness.
3. Extend desktop install readiness and Agent pane with Apple FM as an optional
   local backend.
4. Restore or package the Swift Foundation bridge helper and add desktop/Pylon
   discovery.
5. Add provider go-online capability declaration for
   `probe.backend.apple_fm_bridge`, gated by live health.
6. Add admitted-Mac acceptance docs and live smoke evidence.

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
- Pylon CLI aliases.

What is not connected yet:

- In-tree Swift bridge helper.
- Packaged helper resource.
- Pylon control command for Apple FM readiness.
- Desktop Apple FM RPC.
- Desktop first-run health item.
- Desktop Agent pane local Apple FM route.
- Provider go-online capability declaration.
- Current admitted-Mac desktop smoke evidence.

The next honest milestone is not "Apple FM works in desktop." It is:

`pylon apple-fm status` and desktop `appleFmReadiness` agree on the same
public-safe readiness state for a local bridge, and the desktop shows that state
without exposing tokens or making Apple Silicon detection look like live model
availability.
