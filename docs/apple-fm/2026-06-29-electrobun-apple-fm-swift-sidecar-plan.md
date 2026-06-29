# Electrobun Apple-FM Swift Sidecar Plan

Date: 2026-06-29

Status: audit and implementation plan for issue #6947. This document does not
implement the sidecar. The implementation issue is tracked separately and is
blocked on #6932: #6973.

## Summary

The repo already has most of the Apple Foundation Models runtime contract that
the future Electrobun desktop app should reuse. The missing piece is the
Electrobun Apple build integration: bundle, sign, launch, supervise, and smoke a
Swift Foundation Models bridge as a sidecar process, then route all local Apple
FM turns through Pylon/Khala so token accounting, traces, receipts, and public
counter projections stay inside the existing authority boundaries.

The recommended architecture is:

1. The Electrobun Apple target bundles the existing Swift `foundation-bridge`
   helper as a macOS app resource.
2. The Electrobun Bun host launches or adopts the sidecar on supported Apple
   Silicon machines only.
3. Pylon remains the Apple FM runtime authority. It checks sidecar health over
   loopback, owns tool callbacks, records receipts, and emits safe token usage
   events.
4. Khala routes local Apple FM demand as caller-owned local capacity, not as
   hosted OpenAgents compute and not as third-party marketplace capacity until a
   separate settlement policy is proven.
5. The webview receives only public-safe readiness, blocker refs, and run
   summaries. It never calls the sidecar directly.

## Existing Material

### Swift Bridge And Runtime

Already present:

- `apps/pylon/swift/foundation-bridge/Package.swift`
- `apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift`
- `apps/pylon/swift/foundation-bridge/build.sh`
- `apps/pylon/bin/foundation-bridge`
- `apps/pylon/packages/runtime/src/backends/apple-fm/contract.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/client.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/tools.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/blueprint-tools.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/program-run-evidence.ts`
- `apps/pylon/packages/runtime/src/backends/apple-fm/receipts.ts`
- `apps/pylon/packages/runtime/src/fleet/backend-capability.ts`

The retained bridge contract exposes:

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`
- `POST /v1/sessions`
- `POST /v1/sessions/{id}/responses/stream`

The runtime backend identity is stable enough to reuse:

- backend kind: `apple_fm_bridge`
- profile id: `apple-fm-local`
- model id: `apple-foundation-model`
- default base URL: `http://127.0.0.1:11435`
- attach mode: `attach_existing`
- auth mode: `none`
- capability ref: `probe.backend.apple_fm_bridge`

What exists is intentionally conservative. It supports typed health,
plain-text completion, snapshot streaming, bounded session/tool callbacks, safe
Blueprint tool projection, and public-safe receipts. It should be reused instead
of creating a second Apple FM bridge contract in the Electrobun app.

### Autopilot Desktop Packaging And Smokes

Existing Autopilot Desktop files that should inform the Electrobun sidecar:

- `apps/autopilot-desktop/src/shared/apple-fm-packaging.ts`
- `apps/autopilot-desktop/scripts/verify-packaged-apple-fm-bridge.ts`
- `apps/autopilot-desktop/scripts/apple-fm-live-smoke.ts`
- `apps/autopilot-desktop/tests/apple-fm-packaging.test.ts`
- `apps/autopilot-desktop/tests/apple-fm-loopback-integration.test.ts`

The packaging contract already defines the important invariant:

- helper basename: `foundation-bridge`
- Electrobun copy destination:
  `apple-fm-bridge/foundation-bridge`
- app Resources subpath:
  `app/apple-fm-bridge/foundation-bridge`

`verify-packaged-apple-fm-bridge.ts` is the pre-notarization gate. It verifies
that a built macOS `.app` contains a non-empty executable helper inside the app
bundle, so deep code signing and notarization cover it. The new Electrobun Apple
target should either consume this same helper module or move the shared
packaging constants into a shared package before reuse.

### Apple FM Audits And Acceptance

Grounding docs already in the repo:

- `docs/apple-fm/2026-06-15-current-apple-fm-electrobun-desktop-audit.md`
- `docs/apple-fm/2026-06-15-apple-fm-integration-audit.md`
- `docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-runbook.md`
- `docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-smoke-evidence.md`
- `apps/pylon/docs/probe-port/2026-06-07-apple-fm-first-backend-audit.md`
- `apps/pylon/docs/probe-port/apple-fm-admitted-mac-acceptance.md`
- `apps/pylon/docs/probe-port/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`

These docs establish the current posture:

- Apple FM is a useful bounded local backend, not a Codex replacement.
- Live readiness must come from bridge health, not hardware inventory alone.
- Apple FM tools must be selected before session creation.
- Tool callbacks, callback URLs, and callback tokens belong inside Pylon.
- Public evidence may contain only safe refs and summaries.
- Modeled local Apple FM power/kWh is denominator evidence only, not measured
  telemetry and not accepted-outcomes-per-kWh by itself.

### Khala macOS Context

The now-closed native macOS direction is represented by #6790, #6812, #6873,
#6874, #6884, and #6885. Those issues scoped a native SwiftUI desktop app that
would boot Pylon, connect Apple Foundation Models, package and verify the Swift
helper, publish truthful local capacity, and ship through the signed/notarized
Apple lane. That direction is paused in favor of the Electrobun desktop app
tracked by #6932, but the product requirement survives: on Apple targets, the
desktop app should make local Apple FM available through the same Pylon/Khala
authority path.

The change is implementation shape, not authority shape. The SwiftUI app shell
is no longer the first integration target; the Swift bridge remains the sidecar.

## Target Architecture

```text
Electrobun webview
  |
  | public-safe readiness, mode, blocker refs, run summaries
  v
Electrobun Bun host
  |
  | launch/adopt/stop sidecar, own local process lifecycle
  | token-authenticated loopback control to Pylon
  v
Local Pylon node
  |
  | Apple FM capability report, Blueprint tool preflight,
  | callback tokens, receipts, token usage telemetry, traces
  v
Swift foundation-bridge sidecar
  |
  | Apple Foundation Models API
  v
On-device Apple Intelligence models
```

The webview is not part of the trusted local runtime boundary. It must not know
the raw bridge path, callback URL, callback token, control token, prompt text,
file contents, local workspace paths, or model transcript. It can render:

- `ready`, `unavailable`, `unsupported`, `malformed`, or `unreachable`
- bounded blocker refs
- selected local/hosted mode
- safe run refs and public proof refs
- aggregate token/counting status when projected by Khala

## Sidecar Lifecycle

The Electrobun Bun host should own only process lifecycle and packaged-resource
resolution. Pylon should remain responsible for backend truth.

Lifecycle policy:

- Launch only for macOS Apple Silicon builds.
- Do not launch on unsupported platforms.
- Fail closed when the helper is missing, not executable, unsigned, not
  notarized, or outside the app bundle.
- Autostart in packaged Apple builds when local Apple FM mode is enabled or
  when readiness is being checked for first-run setup.
- In source/dev, prefer explicit configuration:
  `OPENAGENTS_APPLE_FM_BRIDGE_PATH`,
  `OPENAGENTS_APPLE_FM_BASE_URL`, or `PROBE_APPLE_FM_BASE_URL`.
- Keep the sidecar on loopback only.
- Stop the sidecar on app shutdown when the app launched it.
- If an operator already runs a trusted bridge on the configured loopback URL,
  allow attach/adopt mode and record that as an adopted sidecar state.

Sidecar states should be explicit:

- `not_supported`
- `candidate`
- `helper_missing`
- `launching`
- `adopted`
- `running`
- `unavailable`
- `ready`
- `failed`
- `stopped`

Readiness is true only after Pylon calls the bridge health endpoint through
`reportAppleFmBackendCapability` or the equivalent control projection and sees
live Apple FM readiness plus safe Blueprint support.

## Khala Routing And Counting

Local Apple FM turns should count only through canonical token usage ingestion,
not by UI-side estimates or counter deltas.

Required route:

1. Khala or desktop creates a typed local Apple FM request with a public-safe
   objective, selected local mode, owner/caller scope, and bounded tool menu.
2. Pylon performs Blueprint preflight and creates the Apple FM session with only
   the selected tools.
3. Pylon runs the turn through the local sidecar and records safe receipts.
4. Pylon emits an `openagents.token_usage_event.v1` style event or the current
   canonical Khala/Pylon token-ingest equivalent.
5. The ledger row preserves provider/model/backend identity, demand kind/source,
   task/session refs, and `usage_truth`.
6. Public `khala-tokens-served` projections read from the ledger and expose only
   aggregate counters.

Apple FM usage truth must be honest:

- If the bridge reports token counts from Apple FM, mark them `exact`.
- If the runtime can only approximate from bounded text, mark them `estimated`.
- If counts are unavailable, mark them `unknown` and do not synthesize a public
  exact-token claim.

Recommended ledger identity for the first local Apple FM own-capacity lane:

- provider: `pylon-apple-fm-own-capacity`
- model: `apple-foundation-model`
- backend profile: `apple-fm-local`
- backend kind: `apple_fm_bridge`
- demand kind: `own_capacity`
- demand source: `khala_apple_fm_delegation`

The exact strings should be implemented once in the Worker/Pylon contracts, not
duplicated in UI code.

Trace rules:

- Store raw prompts, tool args, file snippets, local paths, and full model
  events only in owner-private raw-event storage if that storage exists for the
  lane.
- Store public-safe ATIF summaries with bounded messages, refs, token counts,
  backend labels, tool labels, and redaction booleans.
- Never copy raw sidecar output, local file contents, control tokens, callback
  tokens, API keys, wallet material, or local auth paths into public traces,
  product-promise evidence, issue comments, or counter projections.

Settlement rules:

- The first Electrobun Apple FM sidecar path should be no-spend owner-local
  capacity.
- Do not make Apple FM marketplace/provider-settlement eligible until Pylon
  capability advertising, assignment admission, receipts, payout, and
  settlement evidence are separately green.

## Packaging And Notarization

The Apple build must package the Swift helper as a sidecar resource, not as an
untracked external binary.

Build requirements:

- Build the Swift helper on the macOS release host.
- Copy the helper into the Electrobun app bundle Resources path:
  `Contents/Resources/app/apple-fm-bridge/foundation-bridge`.
- Ensure the helper is non-empty and owner-executable.
- Deep sign the app bundle so the helper is signed under Apple Team
  `HQWSG26L43`.
- Notarize the app before distribution.
- Run the packaged-helper verifier before notarization.
- Run an admitted-Mac from-install smoke after notarization.

The release runbook remains `docs/DEPLOYMENT.md`, with signing details in
`apps/oa-updates/docs/release-signing-runbook.md`. The relevant Apple team is
`HQWSG26L43`.

Failure handling:

- Missing helper: show an Apple FM local-mode blocker, not a global app failure.
- Unsupported hardware: show unsupported/candidate truth.
- Apple Intelligence disabled: show typed disabled readiness.
- Notarization/signing failure: block release.
- Health timeout: keep local Apple FM unavailable and leave hosted routes
  unaffected.

## Reuse Plan

Reuse directly:

- Swift `foundation-bridge` package and build script.
- Pylon Apple FM runtime contract/client/tool projection/receipts.
- Pylon `apple_fm.status` and `apple_fm.session.start` control shape where it
  still fits the new app.
- Autopilot Desktop packaged-helper verifier and path constants.
- Existing fake-bridge tests and admitted-Mac smoke criteria.

Move or generalize before reuse if needed:

- Move Apple FM packaging constants from `apps/autopilot-desktop/src/shared` to
  a shared package if the new Electrobun app is not `apps/autopilot-desktop`.
- Move sidecar lifecycle helpers into the new desktop app's Bun host layer, not
  into the webview.
- Keep product-promise and public counter wording in `docs/promises/` before
  broadening user-facing claims.

Do not reuse:

- The paused native SwiftUI shell as the primary app implementation.
- Historical Rust/Cargo/Tauri Apple FM desktop code paths.
- Hardware inventory alone as readiness truth.

## Implementation Backlog

Blocked on #6932:

1. Add an Electrobun Apple app resource copy for
   `foundation-bridge`.
2. Add a Bun-side sidecar lifecycle module for packaged helper discovery,
   launch/adopt/stop, loopback port selection, timeout, and cleanup.
3. Wire Pylon control readiness into the new desktop app's local backend mode.
4. Wire local Apple FM turn submission through Pylon, not directly from the
   webview to the sidecar.
5. Add token usage ingestion for local Apple FM own-capacity turns if the
   current Probe telemetry route is not sufficient for Khala counters.
6. Add owner-private trace/raw-event storage or explicitly document a narrower
   first release if raw archives are unavailable.
7. Add fake-bridge unit/integration tests for helper missing, unsupported,
   disabled, ready, not-ready refusal, bounded tool success, and redaction.
8. Add the packaged-helper verifier to the Apple release pipeline before
   notarization.
9. Run an admitted-Mac from-install smoke on the signed/notarized build.

## Acceptance For The Follow-Up Implementation

The implementation issue should be considered done only when:

- #6932 has landed enough Electrobun Apple build structure to attach packaging
  work.
- A signed/notarized Apple build contains the helper at the expected Resources
  path.
- The app launches or adopts the helper on admitted Apple Silicon.
- Pylon reports Apple FM ready from live bridge health.
- A local Apple FM turn runs through Pylon with bounded tools.
- Token usage rows are recorded with honest usage truth and reconcile to the
  public Khala tokens-served projection when truth is exact or estimated.
- Owner-private traces/raw events follow the same redaction boundary as other
  Khala/Pylon local-capacity lanes.
- Unsupported, disabled, missing-helper, and malformed-health states fail
  closed with typed blockers.

## Non-Goals

- Do not implement the sidecar in issue #6947.
- Do not make Apple FM the default hosted-compute replacement.
- Do not claim Codex parity.
- Do not expose the sidecar directly to the webview.
- Do not create paid marketplace Apple FM supply in the first sidecar slice.
- Do not publish exact token claims if the bridge/runtime only has estimates or
  unknown usage.
- Do not broaden product promises until the signed/notarized from-install proof
  exists.

## Open Risks

- Apple Foundation Models availability is admitted-hardware and OS dependent.
- The Swift bridge may need entitlement or hardened-runtime adjustments once it
  is launched as a packaged helper rather than from source.
- Token usage truth may remain estimated or unknown unless the bridge exposes
  reliable model usage.
- Sidecar lifecycle needs careful port/adoption behavior to avoid colliding with
  an operator's existing bridge.
- A public "free tokens on every Mac" story can outrun proof. Keep public copy
  scoped to local owner-capacity until settlement and provider evidence exists.
