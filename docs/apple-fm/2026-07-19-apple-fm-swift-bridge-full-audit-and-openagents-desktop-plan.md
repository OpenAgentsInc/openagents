# Apple Foundation Models Swift Bridge — Full History Audit And OpenAgents Desktop (Electron) Plan

Date: 2026-07-19

Status: consolidated archaeology of every Apple Intelligence / Apple
Foundation Models (FM) integration in this repository history, plus a concrete
plan to ship the surviving Swift bridge inside the current OpenAgents Desktop
Electron app at `apps/openagents-desktop`.

Supersedes the practical target of the earlier plans, which named the now
deleted Electrobun/`autopilot-desktop` and Khala desktop clients:

- `docs/apple-fm/2026-06-15-apple-fm-integration-audit.md`
- `docs/apple-fm/2026-06-15-current-apple-fm-electrobun-desktop-audit.md`
- `docs/apple-fm/2026-06-29-electrobun-apple-fm-swift-sidecar-plan.md`

Those documents remain correct history. This document adds the full dated
timeline, the current-code truth (which now includes a bridge launcher and
supervisor set that did not exist in June), and retargets the desktop
integration at the Electron app that superseded Electrobun.

---

## 1. Executive Summary

Apple Foundation Models has been integrated six distinct times across three
runtime shapes. Only one lane survives on `main`:

1. **Native Swift** (Nov 2025). Direct `FoundationModels` calls inside the old
   `ios/OpenAgentsCore` Swift package. Deleted 2025-12-01.
2. **Effect service + first Swift HTTP sidecar** (Dec 2025). A Bun/Effect
   client and the first `swift/foundation-bridge` localhost server on
   `127.0.0.1:11435`, born in one commit. Sidecar survives, TS layer replaced.
3. **Rust `fm-bridge` crate + GPUI Commander + FRLM conductor** (Dec 2025 to
   Jan 2026). A Rust HTTP client and `fm` CLI to the bridge, inside a Zed-based
   GPUI shell. Removed 2026-01-12.
4. **`foundation-bridge` sidecar matured** (Mar 2026, "psionic" era). The
   sidecar became a full sessioned API: health, models, chat, adapters,
   sessions, snapshot streaming, transcript export/restore, structured
   generation, tool callbacks, typed errors. **This lane survives.**
5. **Khala desktop bundle/supervise/gate** (Jun to Jul 2026). Electrobun/Bun
   desktop clients bundled, signed, and supervised the sidecar, then
   **disabled it for launch** on 2026-07-01. The clients were later deleted by
   owner supersession.
6. **iOS / mobile clients** (Jun to Jul 2026). Attach-only readiness-first
   Swift and Expo clients to the local bridge. Removed 2026-07-14 with the
   retired `clients/` tree.

The durable engineering is real: a buildable Swift bridge helper at
`apps/pylon/swift/foundation-bridge`, a mature TypeScript/Effect runtime at
`apps/pylon/packages/runtime/src/backends/apple-fm/*`, live Pylon presence,
capability, control, token-usage, and receipt wiring, plus a bridge
launcher/supervisor set behind an opt-in flag.

The recurring failure mode is not the runtime. It is the **signed installer
integration**: each desktop attempt was disabled at launch because a packaged,
code-signed, notarized, supervised, from-install smoke was never green in time.

OpenAgents Desktop removes the hard part. The Electron app already bundles,
signs, notarizes, verifies, and supervises a native helper today: the Rust
`oa-desktop-audio` voice helper. The Swift bridge should ride that exact,
already-shipping spine. Section 8 is that plan.

---

## 2. Full Historical Timeline

The requested phases are not strictly time ordered. The native Swift era came
first. The Effect service and the first Swift sidecar were born together. The
Rust crate/GPUI era came days after that. All SHAs are verified against
`git log --all`; dates are commit dates.

### Phase 1 — Native Swift `FoundationModels` (2025-11-03 to 2025-12-01)

Host: Swift, inside the `ios/OpenAgentsCore` package. Direct use of
`SystemLanguageModel.default`, `LanguageModelSession`, and `@Generable`. No
HTTP bridge.

- `8eb0be0cac` (2025-11-03) first FM use: `FoundationModelSummarizer.swift`
  generated 3 to 5 word conversation titles, env-gated, availability-gated,
  low temperature, guardrail failures treated as fallback.
- `9bec6f155f` / `ca13dbcf8b` (2025-11-06) `FMTools.swift` projected OpenAgents
  operations as FM tools: `session.list`, `session.search`, `session.read`,
  `session.analyze`, `content.get_span`, `code.grep`, `fs.list_dir`.
- `d458e64270` (2025-11-07) `FMOrchestrator.swift` and `FMAnalysis.swift`: an
  FM `LanguageModelSession` with those tools explored a workspace, analyzed
  recent sessions, inspected code, and summarized findings.
- `aa9e4e92c1` (2025-11-10) switched tool calling to `@Generable Output`
  instead of `String`; `652966e6c4` typed tool names with a `ToolName` enum.
- `84fe61b232` (2025-11-10) "Enable concurrent delegations from Foundation
  Models to Codex/Claude Code" — FM as an on-device router fanning work out to
  stronger cloud coding agents.

Fate: deleted 2025-12-01 in the twin "Nuke" commits `571ad7a180` /
`d6b8ce1bec` when the project pivoted off the native iOS core. This is the
earliest concrete "Apple FM does agentic codebase search and delegates to
Codex" path in the repo.

### Phase 2 — Effect FM service + first Swift HTTP sidecar (2025-12-05 to 2025-12-07)

These two threads started in the **same commit**.

- `1beb1235f3` (2025-12-05) "feat(fm): Add Apple Foundation Models integration"
  added, together: the TypeScript/Effect client `src/llm/foundation-models.ts`
  (Bun/Effect host, default port `11435`, binary discovery, macOS gating,
  `/health` checks, optional `Bun.spawn` autostart, OpenAI-shaped conversion),
  and the first Swift sidecar `swift/foundation-bridge/`
  (`main.swift`, `Server.swift`, `ChatHandler.swift`, `Types.swift`,
  `Package.swift`, `build.sh`) exposing an OpenAI-compatible API on `11435`.
- `c9d6a5fcec` (2025-12-05) "feat(fm): Add Effect-native FM service with retry,
  metrics, logging" — `src/fm/service.ts` wrapping the bridge with retry
  schedules, metrics, and structured logging.
- `5ef650cc12` (2025-12-06) "FM Terminal-Bench loop with learning layers"
  (MechaCoder): FM as the coding brain routing to subagents.
- `5ba9c1f7a7` (2025-12-07) connected a UI to the MechaCoder loop with live
  streaming.

Fate: the TypeScript layer was superseded by repeated repo resets over Dec
2025 to Feb 2026 and re-homed into the Pylon runtime. The Swift sidecar
persisted and became the long-lived component.

### Phase 3 — Rust `fm-bridge` crate + GPUI Commander + FRLM (2025-12-09 to 2026-01-12)

Host: Rust (Cargo workspace), GPUI (Zed's UI framework). This era came after
the Effect/Swift bridge already existed.

- `65ce6a9896` (2025-12-09) "Add Rust workspace with Commander GPUI app and FM
  bridge": `crates/commander/` (a minimal GPUI desktop shell) and
  `crates/fm-bridge/` (a type-safe Rust HTTP client to the Swift bridge, not
  FFI: chat completions, model listing, health, guided generation via the
  bridge, approximate token tracking, a `fm` CLI, Tokio, typed errors). It did
  not yet support multi-turn sessions, function calling, or embeddings.
- `ea95f2fc2c` (2025-12-09) expanded the Swift bridge with streaming, sessions,
  tools, and `AdapterRegistry.swift`.
- `33b3ce4850` (2025-12-10) `subagent_router.rs` routed between Claude Code,
  FM, and a minimal backend; Commander then absorbed large parts of Zed.
- `1402745754` (2026-01-04) "Implement RLM as Apple FM tools via FRLM
  conductor": `crates/compute/src/backends/apple_fm.rs`,
  `frlm_tool_handler.rs`, `crates/frlm/`, and Swift `GuidedTypes.swift` /
  `ToolHandler.swift`. FM became a conductor invoking RLM behaviors as tools.

Fate: `cfd3da20e2` (2026-01-12) "Remove fm-bridge crate from workspace" as part
of the autopilot MVP prune. The Rust codebase was deprecated, restored, and
pruned again to "wgpui + MVP doc only" `d7f53fccc0` (2026-02-25). The GPUI
Commander/`fm-bridge` lane did not survive.

### Phase 4 — `foundation-bridge` sidecar matured (2026-03-07 to 2026-03-16, "psionic" era)

Host: Swift HTTP sidecar, driven by a Rust host in the then-current
`apps/autopilot-desktop`. This is the **surviving** design.

- `a0f4ea285c` (2026-03-07) "compute: revive apple foundation models backend"
  re-added `swift/foundation-bridge/` alongside a Rust host
  `apps/autopilot-desktop/src/apple_fm_bridge.rs`.
- The psionic build-out turned the sidecar into a full sessioned API, one
  capability per commit:
  - `8ed1965768` session bridge (`POST /v1/sessions`)
  - `139bdab3bd` SSE session streaming
    (`POST /v1/sessions/{id}/responses/stream`) with snapshot events plus
    terminal completion payloads
  - `b08f2fc547` transcript export/restore
    (`GET /v1/sessions/{id}/transcript`); historical tool mentions in a
    restored transcript do not re-enable tools
  - `72cf435a92` structured generation
    (`POST /v1/sessions/{id}/responses/structured`), schema-guided decode via
    `@Generable` / `GeneratedContent`, mapped Rust-side to
    `schemars::JsonSchema`
  - `2fa2b742c2` tool calling (`ToolCallbackConfiguration`, callback URL plus
    `session_token`, `RemoteTool` implementing Apple's `Tool` protocol)
  - `bd41600f75` typed Foundation Models error mapping
- Bundled-helper packaging began here: `399749f38e` "Launch Apple FM bridge as
  bundled helper", `948356e85d` "Fix packaged Apple bridge helper bundling".

Relocation: after the Apr to Jun 2026 rebuild as a Bun/Effect workspace
(`f5919c7669`) and the Pylon scaffold, the root `swift/` tree was deprecated
(`2f1ba3abd8`, 2026-06-08) and the sidecar was restored under Pylon at
`apps/pylon/swift/foundation-bridge/` by `001f75b919` (2026-06-15).

Fate: survives on `main`. The current single-file `main.swift` is a
deliberately conservative subset of the psionic API (Section 4).

### Phase 5 — Khala desktop bundle/supervise/gate, then disabled (2026-06-20 to 2026-07-01)

Host: Electrobun/Bun desktop clients (`khala-desktop`, `khala-macos`,
`khala-code-desktop`) supervising the Swift sidecar as a child process.

- 2026-06-20: supervision policy (`24521b5a1f`), public-safe status projection
  (`feac9ce3fd`), installer recut gate on the bundled helper (`4d15c4ee82`),
  stateful supervisor driver (`8ed9df38c6`), live launcher gluing supervisor to
  a real child (`c8afd52a6a`), host-assembly factory (`aad236c09e`).
- 2026-06-28/29: bundle the sidecar in app builds (`2d6f3e5224`,
  `2cc89a63ed`), readiness UI (`e8f033b44a`), supervise the sidecar
  (`b89972dc77`), accept a bridge port flag (`9f100b02aa`), relocate the
  authoritative sidecar and delete `khala-desktop` (`c35bb70b61`), and an
  on-device decider choosing between Apple FM and a GPT-OSS backend
  (`e4968c23f5`). Supporting fixes bundled, verified, fail-closed on readiness,
  passed a port flag, restart-supervised, stopped on shutdown, and gated the
  installer on a smoke/evidence check.

Fate: `1bbefc253c` (2026-07-01) "Disable Apple FM bridge for launch" flipped
the client off the bundled bridge; `a855203bb5` deleted the prepare script.
The Khala Code desktop client was later superseded by OpenAgents Desktop and
deleted (owner supersession, mid-Jul 2026). It was disabled because the
bundled/signed/supervised on-device path was not launch-ready, and
Pylon-hosted capacity was the supported path.

### Phase 6 — iOS / mobile clients + push-to-talk STT (2026-06-28 to 2026-07-14)

- `clients/khala-ios/.../Net/AppleFMClient.swift` (2026-06-28): an attach-only,
  readiness-first Swift client. `defaultBaseURL = http://127.0.0.1:11435`,
  model `apple-foundation-model`. Always checks `GET /health` before
  inference; models unsupported machines as an honest `Availability`
  (`ready` / `unavailable` / `unsupported`) with `blockerRefs` and a
  `UsageTruth` (`exact` / `estimated` / `unknown`). Never masks an unsupported
  device as a transport error.
- `clients/khala-mobile/modules/khala-apple-foundation-models/` (2026-07-04):
  an Expo native module reading `OPENAGENTS_APPLE_FM_BASE_URL` /
  `PROBE_APPLE_FM_BASE_URL` (default `127.0.0.1:11435`), returning `blocked`
  by default (`blocker.khala_mobile.apple_fm_bridge_health_unproven`) until the
  local helper proves health.
- `afb316491b` (2026-07-05) wired push-to-talk STT and the FM readiness signal
  into the mobile routed screens.

Fate: removed 2026-07-14 by `9e99de2ab2` "refactor: retire deprecated clients"
with the whole `clients/` tree, in favor of Pylon, OpenAgents Desktop, and
OpenAgents mobile.

### Timeline table

| Phase | Dates | Host / language | Key SHAs | Fate |
|---|---|---|---|---|
| 1. Native Swift FM | 2025-11-03 to 12-01 | Swift, `FoundationModels` direct | `8eb0be0cac`, `aa9e4e92c1`, `84fe61b232` | Deleted 2025-12-01 (`571ad7a180`) |
| 2. Effect svc + first sidecar | 2025-12-05 to 12-07 | TS/Effect on Bun + Swift sidecar | `1beb1235f3`, `c9d6a5fcec`, `5ef650cc12` | TS replaced; sidecar persisted |
| 3. Rust `fm-bridge` + GPUI + FRLM | 2025-12-09 to 2026-01-12 | Rust, GPUI | `65ce6a9896`, `33b3ce4850`, `1402745754` | Removed 2026-01-12 (`cfd3da20e2`) |
| 4. Sidecar matured | 2026-03-07 to 03-16 | Swift sidecar | `a0f4ea285c`, `139bdab3bd`, `72cf435a92`, `2fa2b742c2`, `bd41600f75` | **Survives** in Pylon |
| 5. Khala desktop bundle/gate | 2026-06-20 to 07-01 | Electrobun/Bun + supervised child | `b89972dc77`, `c35bb70b61`, `1bbefc253c` | Disabled 2026-07-01; client deleted |
| 6. iOS / mobile clients | 2026-06-28 to 07-14 | Swift iOS + Expo module | `e6bb20a648`, `2f15f14899`, `afb316491b` | Removed 2026-07-14 (`9e99de2ab2`) |

Recovery: the deleted lanes are recoverable only via Git history. The native
`OpenAgentsCore` FM code, the Rust `fm-bridge`/Commander/FRLM code, and all
`clients/` FM clients are gone from the tree.

---

## 3. The Product Thesis (Episodes 194 and 201)

The thesis never was "Apple FM replaces Codex." From the transcripts:

- Episode 194: Apple FM is a useful local agent primitive for codebase search
  and title/summary work; run on-device on Apple Silicon, stream to a device,
  and supplement Codex/QuadCode rather than replace them. The first expected
  offload was small (about 5 percent of cloud share), growing over time.
- Episode 201 ("Fracking Apple Silicon"): stranded Apple Silicon becomes
  sellable only when Pylon supplies discovery, job packaging, trust,
  settlement, observability, replay, receipts, and routing. Apple Silicon is
  the first target because no model download is needed when Apple Intelligence
  is already on the machine. "Go online and earn Bitcoin" is a market claim,
  not a runtime claim, and is honest only after the market plumbing is live.

That maps exactly to the surviving code: FM as a bounded local coordinator and
delegator around stronger coding agents, routed through Pylon as one sellable
local compute envelope once receipts and settlement are green.

---

## 4. The Surviving Artifact: The Current Swift Bridge

Location on `main`: `apps/pylon/swift/foundation-bridge/`
(`Package.swift`, `Sources/foundation-bridge/main.swift` at 799 lines,
`build.sh`, `README.md`). The built wrapper is `apps/pylon/bin/foundation-bridge`.

### Build and platform

- `Package.swift`: `swift-tools-version:6.2`, `platforms: [.macOS(.v26)]`,
  single executable target `foundation-bridge`, compiled `-parse-as-library`.
- `build.sh`: `swift build -c release`, then installs the
  `apps/pylon/bin/foundation-bridge` wrapper.
- README requirements: Apple Silicon Mac, macOS 26 or newer, Swift 6.2 or
  newer toolchain, Apple Intelligence and Foundation Models available for the
  logged-in user.

### Identity constants (`main.swift`)

- bridge version `0.1.1`
- default port `11435`
- model id `apple-foundation-model`
- max request body 1 MiB

### FoundationModels APIs actually called

- `import FoundationModels`.
- Availability: `SystemLanguageModel.default.availability`, switched over
  `.available` and `.unavailable(reason)`. Reasons map to typed strings:
  `unsupported_hardware`, `apple_intelligence_disabled`, `permission_denied`,
  `model_unavailable`, `unknown`.
- Generation: a fresh `LanguageModelSession()` per completion, then
  `try await session.respond(to: prompt)` using `response.content`. Messages
  are flattened into one text prompt with `System:` / `User:` / `Assistant:` /
  `Tool Result:` prefixes.
- Usage is **estimated** from character counts (`count / 4`),
  `truth: "estimated"`. The runtime never labels this as exact.

The current single file is a deliberately conservative subset of the mature
psionic API. It does **not** currently implement `@Generable` structured
generation, native token-by-token streaming, transcript export/restore, or
adapter loading. The SSE session stream emits exactly two frames (`snapshot`
then `completed`) around one bounded turn.

### HTTP contract (routing table in `main.swift`)

- `GET /health` — availability probe (`ready`, `model`, `unavailableReason`,
  `platform`, `version`).
- `GET /v1/models` and `GET /models` — the single `apple-foundation-model`.
- `POST /v1/chat/completions` and `POST /chat/completions` — one-shot
  completion.
- `POST /v1/sessions` — create a session (`apple_fm_session_<uuid>`), records
  instructions, projected tools, and an optional `tool_callback`.
- `POST /v1/sessions/{id}/responses/stream` — bounded callback-tool session
  stream, SSE `snapshot` plus `completed`.
- `OPTIONS` — 204 with permissive CORS.

The server is hand-rolled on `Network.framework` `NWListener` over loopback
TCP, with manual HTTP/1.1 parse and `Connection: close`. It logs only startup
and listener failures. It never logs prompts, bodies, files, secrets, or
provider payloads.

### Tool callback (bridge side)

Only `read_file` is honored, heuristically: when the projected tools contain
`read_file` and the prompt mentions `read_file` / `read file` / `readme`, the
bridge POSTs `{ session_token, tool_name, arguments: { generation_id, content:
{ path }, is_complete } }` to the loopback callback URL, folds the returned
text into the prompt, and instructs the model not to mention callback URLs or
tokens. This is minimal demo-grade callback, not general tool use. The general
tool authority lives in Pylon, not the bridge.

---

## 5. The Current Pylon Runtime (What Exists Now)

The Apple FM runtime is structurally complete and, apart from process
supervision, live and wired into every Pylon heartbeat. There are two
near-identical copies: the **live** one Pylon imports at
`apps/pylon/packages/runtime/**` (package `@openagentsinc/pylon-runtime`,
ESM `.js` specifiers), and a migration-source mirror at
`packages/probe/packages/runtime/**` (package `probe`). References below are
the live copy.

### Backend contract (`backends/apple-fm/contract.ts`)

- backend kind `apple_fm_bridge`
- profile id `apple-fm-local`
- model id `apple-foundation-model`
- default base URL `http://127.0.0.1:11435`
- readiness path `/health`, attach mode `attach_existing`, auth `none`,
  stream mode `snapshot`
- capability ref `probe.backend.apple_fm_bridge`
- base URL resolution: explicit override, then `PROBE_APPLE_FM_BASE_URL`, then
  `OPENAGENTS_APPLE_FM_BASE_URL`, then the default loopback URL.

### Live pieces (all wired)

- `backends/apple-fm/client.ts` — Effect HTTP client: `health()`,
  `requireReady()`, `completePlainText()`, `streamPlainTextSnapshots()`,
  `streamSessionWithTools()` (spins a loopback callback server, creates a
  session, parses SSE), and `smoke()`. Typed `AppleFmBackendError` with
  redacted receipts.
- `backends/apple-fm/tools.ts` — callback-tool session engine and loopback
  server; accepts both the Swift-bridge payload shape and a native shape;
  policy gate `allow` / `approval_required` / `deny`, round-trip limit, token
  check, redacted transcript. Recognized tool names: `read_file`,
  `list_files`, `code_search`, `shell`, `apply_patch`, `consult_oracle`,
  `analyze_repository`, `propose_action_submission`.
- `backends/apple-fm/receipts.ts` — availability / failure / transcript
  receipts, all `contentRedacted: true`, base URLs redacted.
- `backends/apple-fm/blueprint-tools.ts`, `acceptance.ts`,
  `program-run-evidence.ts` — Blueprint-tool preflight projection, retained
  acceptance cases, offline program-run evidence.
- `fleet/backend-capability.ts` — `reportAppleFmBackendCapability(...)`
  produces the capability report; advertises `probe.backend.apple_fm_bridge`
  only when live health plus safe Blueprint support are green.
- `src/node/apple-fm-status.ts` — attach-only status projection
  (`openagents.pylon.apple_fm.status.v0.1`) with typed blocker refs. This is
  the `apple_fm.status` control action's data source.
- `src/node/apple-fm-local-session.ts` — the real local coding session:
  read-only workspace tools (`read_file`, `list_files`, `code_search`) bounded
  to the worktree, `sandboxMode: "read-only"`, `networkAccessEnabled: false`,
  `maxModelRoundTrips: 8`, digest refs only.
- `src/node/control-server.ts` + `control-sessions.ts` — control actions
  `apple_fm.status` and `apple_fm.session.start`; the session action gates on
  `available && ready && capability advertised && no blockers`.
- `packages/pylon-core/src/presence/apple-fm-status.ts` +
  `presence/presence.ts` — capacity/health/load refs merged into every
  heartbeat when the local bridge reports ready:
  `capacity.inference.apple_fm_bridge.ready=1`,
  `capacity.inference.apple_fm_bridge.available=1`, plus health, model,
  profile, and load refs (extracted in `5676fe92c6`, #8578 PY-1).
- `fleet/token-usage.ts` — `makeAppleFmProbeTokenUsageEvent(...)`,
  `provider: "apple_fm"`, `usageTruth = usage.truth`, run through the same
  public-projection redaction and unsafe-material validators as all Pylon
  token events.
- `src/node/apple-fm-energy-estimate.ts` — modeled local-session power/kWh
  estimate (denominator evidence for #5074), default `0.02` kW.

### Dormant / opt-in

Bridge **process supervision** (auto-launch of the helper) is the one inert
piece, behind `PYLON_APPLE_FM_SUPERVISE=1`:

- `src/node/apple-fm-bridge-helper.ts` — helper discovery. Honors
  `OPENAGENTS_APPLE_FM_BRIDGE_PATH`, then walks ancestors for
  `bin/foundation-bridge` or `swift/foundation-bridge/.build/release/foundation-bridge`,
  then a packaged Electron resource at
  `<resources>/app/apple-fm-bridge/foundation-bridge`.
- `apple-fm-bridge-launcher.ts` / `-launcher-host.ts` — spawn and adopt logic.
- `apple-fm-bridge-supervisor.ts` / `-driver.ts` / `-status.ts` — a pure
  restart/backoff reducer plus supervised status blockers.
- `apple-fm-supervised-launch.ts` / `apple-fm-supervised-status.ts` —
  composition. Constructed only when `PYLON_APPLE_FM_SUPERVISE=1` and a helper
  is discovered; otherwise `apple_fm.status` returns the plain attach-only
  projection.

This launcher/supervisor set is newer than the June 2026 audits, which
described the runtime as attach-only. The building blocks for a supervised
sidecar now exist in Pylon; they are simply not enabled by default because the
signed-installer recut and admitted-Mac from-install smoke are still open.

### Tests

CI-safe fake-server tests cover env override order, a fake bridge returning
ready health plus a completion, receipt redaction, snapshot streaming,
callback-tool policy gating and redaction, Blueprint tool projection, offline
program-run evidence, capability reporting, helper discovery, the restart
supervisor reducer, supervised-launch composition, and the read-only local
control session. None require a real Apple Silicon FoundationModels device.

---

## 6. Product-Promise Status

From `docs/promises/registry.md`:

- `edge.apple_silicon_local_orchestration.v1` — **yellow**.
- `autopilot.local_apple_fm_tool_chat.v1` — **yellow**. Open blockers:
  `blocker.product_promises.local_apple_fm_helper_supervision_missing` and
  `blocker.product_promises.local_apple_fm_signed_from_install_supervised_smoke_missing`.
- `pylon.open_compute_market_with_wallet.v1` — **yellow**.
- `pylon.apple_silicon_button_money.v1` — **red**.

The single blocker between yellow and green for the local tool/chat promise is
a signed, notarized installer that launches and supervises the helper, proven
by an admitted-Mac from-install smoke. Section 8 targets exactly that gap.

---

## 7. Why It Keeps Getting Disabled At Launch

Three desktop attempts (autopilot-desktop, khala-desktop, khala-code-desktop)
all reached "bundle and supervise the sidecar" and all stopped short of a
green signed release. The recurring blocker is never the runtime. It is:

1. A packaged, deep-code-signed, notarized `.app` that actually contains the
   helper at a stable Resources path.
2. Hardened-runtime and entitlement adjustments once the helper runs as a
   packaged binary rather than from source.
3. A from-install smoke on an admitted Apple Silicon Mac that proves launch,
   supervision, ready health, one bounded turn, honest token accounting, and
   redaction.

Every disable-for-launch decision (`1bbefc253c` and predecessors) traces to
one of those three not being green in time. OpenAgents Desktop already solves
1 and 2 for another native helper today, so the remaining work is far smaller
than it was for any prior client.

---

## 8. Bringing It To OpenAgents Desktop (Electron)

Target: `apps/openagents-desktop` (`@openagentsinc/openagents-desktop`,
Electron `^43.1.0`, Electron Forge `7.11.2`, Node 24, Vite renderer, Effect
and Effect Native throughout). The app has zero Apple FM references today, so
this is greenfield inside the app but reuses the entire Pylon runtime.

### 8.1 Why this app is the right home

OpenAgents Desktop already ships a native helper end to end: the Rust
`oa-desktop-audio` voice helper. It is built into `dist/native/<arch>/`, hashed
into a `manifest.json`, bundled with Forge `extraResource`, added to the
code-signable basename allowlist, deep-signed and notarized, verified at
runtime (SHA-256 digest plus `codesign --verify --strict`), spawned with a
hardened stripped environment, and supervised by a typed host state machine.
The Swift bridge should ride this exact spine. Files to model on:
`src/voice-native-helper.ts`, `src/voice-host.ts`, `scripts/build.ts`,
`scripts/stage-target.ts`, `forge.config.ts`.

### 8.2 Target architecture

```text
Renderer (React / Effect Native, sandboxed, CSP connect-src 'none')
  |  typed Effect Schema IPC only (window.openagentsDesktop.appleFm.*)
  v
Electron main process (src/main.ts)
  |  owns: packaged-helper resolve + verify (digest + codesign),
  |        spawn/adopt/stop, hardened env, typed supervisor state
  |  imports @openagentsinc/pylon-runtime in-process for FM runtime authority
  v
Swift foundation-bridge sidecar  (loopback 127.0.0.1:11435)
  |  Apple FoundationModels API
  v
On-device Apple Intelligence models
```

The renderer is never part of the trusted boundary. It never learns the raw
bridge path, callback URL, callback token, prompt text, file contents, local
workspace paths, or transcript. It renders only: readiness state
(`ready` / `unavailable` / `unsupported` / `malformed` / `unreachable`),
bounded blocker refs, selected local/hosted mode, and safe run/proof refs.

### 8.3 Runtime-authority decision: in-process Pylon runtime, not a Pylon daemon

The desktop app already imports `@openagentsinc/pylon-core` in-process for
Codex account custody; it does not spawn a Pylon binary. Follow the same
shape: the Electron **main process owns only the sidecar process lifecycle and
packaged-resource resolution**, and imports the Pylon **runtime library**
(`@openagentsinc/pylon-runtime` FM backend: `makeAppleFmClient`,
`reportAppleFmBackendCapability`, `runAppleFmLocalControlSession`,
`makeAppleFmProbeTokenUsageEvent`) in-process to check health, gate readiness,
run bounded read-only sessions, produce receipts, and emit honest token-usage
events. This keeps Pylon as the runtime authority without a second daemon, and
reuses every existing test and redaction boundary.

Do not re-implement the bridge client, capability report, or receipt logic in
the desktop app. Import them.

### 8.4 What to ship — build, stage, bundle

1. **Dev build** (`scripts/build.ts`): add a macOS-arm64-only step, gated on
   `process.platform === "darwin"` and arch, that runs
   `apps/pylon/swift/foundation-bridge/build.sh` (or `swift build -c release`)
   and copies the binary to `dist/native/<arch>/foundation-bridge`, writing a
   `manifest.json` line (protocolVersion, helperVersion, architecture,
   sha256). Skip cleanly on non-darwin and non-arm64 hosts. The voice helper's
   `cargo build -p oa-desktop-audio` step at `scripts/build.ts` is the pattern;
   substitute `swift build`.
2. **Release staging** (`scripts/stage-target.ts`): extend `buildNativeHelper`
   to also build the Swift helper with the target definition, land it at
   `native/<arch>/foundation-bridge`, and record its digest in `ledger.json`.
   Only build it for `darwin-arm64`; every other target must omit it.
3. **Bundle** (`forge.config.ts`): `extraResource: ["dist/native"]` already
   ships anything under `native/<arch>/`, so the helper lands at
   `Contents/Resources/native/<arch>/foundation-bridge`. Match Pylon's
   packaged-resource discovery, which also probes
   `<resources>/app/apple-fm-bridge/foundation-bridge`; pick one canonical path
   and make the Pylon helper discovery and the desktop bundling agree on it.

### 8.5 Mandatory allowlist changes (packaging refuses otherwise)

The staging pipeline is deny-by-default and hard-pinned to `oa-desktop-audio`.
All three must change or the build fails:

1. `forge.config.ts` `macCodeSignableBasenames` — add `"foundation-bridge"`,
   or signing skips it and the app is Gatekeeper-dead.
2. `scripts/stage-target.ts` native-path allowlist regex (currently
   `/^native\/(?:arm64|x64)\/oa-desktop-audio(?:\.exe)?$/`) — extend to admit
   `native/arm64/foundation-bridge`.
3. `scripts/stage-target.ts` closure/ledger logic — include the Swift helper as
   a first-class native component so the pre-copy, post-copy, and post-package
   ASAR/closure oracles verify its bytes.

### 8.6 Entitlements

The current `build/entitlements.mac.plist` is minimal (`allow-jit`,
`allow-unsigned-executable-memory`). The Swift helper is a separate Mach-O
signed under the same Developer ID and hardened runtime. Evaluate and, if
required, add:

- `com.apple.security.network.server` and `com.apple.security.network.client`
  — the helper listens on loopback and makes loopback callback requests.
- Any FoundationModels-framework requirement surfaced when the helper runs
  under hardened runtime rather than from source (a known open risk from every
  prior attempt).

Per-file entitlements are applied at sign time via `optionsForFile`, so a
helper-specific entitlement set is possible without loosening the app.

### 8.7 Runtime supervision and IPC

1. `src/apple-fm-native-helper.ts` — mirror `voice-native-helper.ts`: resolve
   the path from `process.resourcesPath` (packaged) or `dist` (dev); verify
   manifest `protocolVersion`, `architecture === process.arch`, executable
   bit, SHA-256 digest, and `codesign --verify --strict`; spawn with the
   hardened env (`LANG=C`, `LC_ALL=C`, `HOME=/var/empty`, `PATH=""`,
   `windowsHide: true`, no detach), passing `--port` for a chosen loopback
   port. Transport differs from the voice helper: the bridge speaks loopback
   HTTP, so after spawn the supervisor polls `GET /health` through the Pylon
   client rather than reading stdout JSONL. Keep stdout/stderr for lifecycle
   and crash detection only.
2. `src/apple-fm-host.ts` — mirror `voice-host.ts`: a pure state machine with a
   single owned session, a generation counter, typed states
   (`not_supported`, `candidate`, `helper_missing`, `launching`, `adopted`,
   `running`, `unavailable`, `ready`, `failed`, `stopped`), crash to typed
   failure, and `dispose()` on app shutdown. Adopt an operator-run bridge if
   one already listens on the configured loopback URL. Readiness is true only
   after the Pylon client sees live health plus safe Blueprint support.
3. IPC: add `src/apple-fm-contract.ts` (Effect Schema channels plus decoders),
   expose a typed `appleFm` method group on the `openagentsDesktop` bridge in
   `preload.cts`, and `ipcMain.handle(...)` in `main.ts`. Never expose a raw
   channel. Decode requests before they reach main and responses before they
   reach the renderer, exactly like `desktop-preferences-contract.ts`.

### 8.8 Lifecycle policy

- Launch only for macOS Apple Silicon builds. On any other platform report
  `not_supported` and leave hosted routes untouched.
- Fail closed when the helper is missing, non-executable, digest-mismatched,
  or unsigned. Show an Apple FM local-mode blocker, never a global app failure.
- Autostart in packaged Apple builds when local Apple FM mode is enabled or a
  first-run readiness check runs. In dev, prefer explicit config
  (`OPENAGENTS_APPLE_FM_BRIDGE_PATH`, `OPENAGENTS_APPLE_FM_BASE_URL`,
  `PROBE_APPLE_FM_BASE_URL`).
- Keep the sidecar on loopback only. Stop it on app shutdown when the app
  launched it. Never stop an adopted operator bridge.

### 8.9 Token accounting, receipts, redaction

- Route local FM turns only through the Pylon runtime; count only through
  canonical token-usage ingestion, never UI estimates or counter deltas.
- Usage truth stays honest: mark exact only if the bridge reports exact model
  usage. The current bridge reports character-estimated usage, so the first
  desktop slice must mark `estimated`, never synthesize an exact public claim.
- First lane is no-spend owner-local capacity. Recommended ledger identity:
  provider `pylon-apple-fm-own-capacity`, model `apple-foundation-model`,
  backend profile `apple-fm-local`, backend kind `apple_fm_bridge`, demand
  kind `own_capacity`, demand source `khala_apple_fm_delegation`. Implement the
  exact strings once in the Pylon/Worker contracts, not in desktop UI.
- Redaction: never copy raw prompts, tool args, file snippets, local paths,
  control tokens, callback tokens, API keys, wallet material, or model
  transcripts into public traces, product-promise evidence, issue comments, or
  counter projections. Reuse the existing Apple FM receipt and token-event
  redaction; the desktop app adds no new public surface.

### 8.10 Signing, notarization, release

Reuse the existing pipeline unchanged in shape:

- `preMake` deep-signs the verified app (helper included via the basename
  allowlist) under `OA_DEVELOPER_ID_APPLICATION` with hardened runtime.
- `postMake` notarizes and staples the `.app` and `.dmg` and runs
  `assertGatekeeperGreen`. There is no unsigned release fallback except
  `OA_ALLOW_UNSIGNED_DEV=1`, which renames artifacts `-UNSIGNED-DEV`.
- Credentials: `ASC_API_PRIVATE_KEY_PATH`, `ASC_API_KEY_ID`,
  `ASC_API_ISSUER_ID`; Apple Team `HQWSG26L43`. Release hub is
  `docs/DEPLOYMENT.md`; signing detail is
  `apps/oa-updates/docs/release-signing-runbook.md`.
- Add a from-install smoke after notarization that adopts or launches the
  helper, proves ready health through the Pylon client, runs one bounded
  read-only session, and asserts honest token accounting plus redaction.

### 8.11 Test plan (CI-safe plus admitted-Mac)

CI (fake bridge, no device): helper-missing, unsupported-platform,
unsupported-hardware, apple-intelligence-disabled, ready, not-ready refusal,
bounded read-only tool success, digest mismatch, signature invalid, adopt an
existing loopback bridge, and redaction of callback URL/token and base-URL
credentials. Reuse the Pylon fake-server tests through the in-process runtime
import.

Admitted Apple Silicon Mac (from a signed, notarized install): launch or adopt
the helper, Pylon reports ready from live health, one local FM turn runs with
bounded tools, token rows record honest usage truth and reconcile to the public
counter when exact or estimated, and unsupported/disabled/missing-helper states
fail closed with typed blockers.

### 8.12 Acceptance gates

Done only when:

- A signed, notarized `darwin-arm64` OpenAgents Desktop build contains the
  helper at the stable Resources path and passes the staged-ledger and ASAR
  oracles.
- The app launches or adopts the helper on admitted Apple Silicon; Pylon
  reports ready from live bridge health.
- A local FM turn runs through the in-process Pylon runtime with bounded
  read-only tools; token rows carry honest usage truth and redaction.
- Unsupported, disabled, missing-helper, and malformed-health states fail
  closed with typed blockers and never affect hosted routes.
- `pnpm run check` is green, and the from-install smoke passes.

When those hold, `autopilot.local_apple_fm_tool_chat.v1` can move from yellow
to green, and the two supervision/from-install blockers clear.

### 8.13 Non-goals and boundaries

- Not a Codex replacement and not the default hosted-compute path.
- No paid marketplace Apple FM supply in the first slice; no settlement,
  provider eligibility, or "instant sats" copy.
- No exact-token public claims while the bridge only estimates.
- The renderer never talks to the sidecar directly.
- No new Rust/Tauri/WGPUI shell and no revival of the deleted GPUI Commander,
  Electrobun, or `clients/` lanes.
- Apple FM tools must be selected before session creation (Blueprint
  preflight); do not expose all tools by default.
- Do not broaden product promises until the signed-notarized from-install
  proof exists.

---

## 8b. Implementation Status (landed 2026-07-19)

The backlog in §9 was implemented and merged to `main` under epic #9069:

- **AFM-1 (#9070)** — one-shot bridge launcher/adopt/stop for the CLI
  (`apps/pylon/packages/runtime/src/backends/apple-fm/bridge-process.ts`).
- **AFM-2 (#9071)** — frozen, versioned wire contract
  (`.../apple-fm/wire.ts`, `openagents.apple_fm.bridge.wire.v0.2`) with fake-
  and opt-in real-bridge conformance tests.
- **AFM-3 (#9072)** — runnable `apple-fm` CLI (`health`, `infer`/`chat`,
  `session`, `tool`) with `--json` and `--auto-launch`.
- **AFM-4 (#9073)** — bounded read-only workspace tool loop
  (`read_file`/`list_files`/`code_search`, escape/symlink refusal + caps) with
  a generalized Swift bridge dispatcher.
- **AFM-5 (#9074)** — real progressive snapshot streaming (bridge v0.1.3,
  FoundationModels `streamResponse(to:)`) and honest usage truth; CLI
  `--stream`.
- **AFM-6 (#9075)** — OpenAgents Desktop Effect Schema IPC + native helper +
  host supervisor consuming the Pylon FM runtime in-process (renderer surface
  deferred).
- **AFM-7 (#9076)** — Desktop build/stage/bundle/sign wiring + entitlements +
  allowlists (signed/notarized from-install smoke deferred pending Apple
  credentials).

All were proven end-to-end on an admitted Apple Silicon Mac (macOS 26.4, Swift
6.3.3): real inference, a real bounded read-only tool turn, and progressive
streaming through a bridge the launcher started and stopped. The Pylon runtime
now carries `bridge-process`, `wire`, and `workspace-tools`; the CLI reference
is `docs/apple-fm/apple-fm-cli.md`. Remaining to reach a green from-install
product promise: the signed/notarized installer smoke (AFM-7 deferred item) and
a renderer readiness surface (AFM-6 deferred item).

## 9. Implementation Backlog

1. Add the macOS-arm64 Swift build step to `scripts/build.ts` (dev) and
   `scripts/stage-target.ts` (`buildNativeHelper`, release) with a
   `manifest.json` digest.
2. Extend the three `stage-target.ts`/`forge.config.ts` allowlists
   (Section 8.5).
3. Evaluate and add helper entitlements (Section 8.6).
4. Add `src/apple-fm-native-helper.ts` (resolve/verify/spawn/adopt/stop) and
   `src/apple-fm-host.ts` (typed supervisor), importing the Pylon FM runtime
   in-process.
5. Add `src/apple-fm-contract.ts`, the preload `appleFm` method group, and the
   `ipcMain.handle` wiring; add a minimal renderer readiness/mode surface.
6. Wire local FM turns through the Pylon runtime with honest token-usage
   ingestion and receipts; add no new public surface.
7. Add CI fake-bridge tests (Section 8.11).
8. Add the packaged-helper verifier to the release pipeline before
   notarization; add the from-install smoke after.
9. Run the admitted-Mac from-install smoke on a signed, notarized build and
   record public-safe evidence under `docs/apple-fm/`.

---

## 10. Reference And Restoration Notes

- The surviving bridge is `apps/pylon/swift/foundation-bridge`. Reuse it; do
  not invent a second Apple FM bridge contract.
- The mature psionic API (structured generation, transcript export/restore,
  adapters) is recoverable from `bd41600f75:swift/foundation-bridge` and the
  intervening psionic commits if the desktop app later needs more than the
  current conservative subset. Restore into the existing Pylon TypeScript
  contract rather than renaming endpoints.
- The desktop native-helper spine to copy is `oa-desktop-audio`:
  `src/voice-native-helper.ts`, `src/voice-host.ts`, `scripts/build.ts` native
  step, `scripts/stage-target.ts` `buildNativeHelper` plus allowlist,
  `forge.config.ts` `macCodeSignableBasenames` and `extraResource`, and
  `src/main.ts` helper wiring (`createPackagedVoiceNativeMedia`).
- Durable invariants to preserve: named backend kind `apple_fm_bridge` (not a
  generic local-inference alias); loopback contract on `127.0.0.1:11435`;
  attach-or-adopt readiness posture; Apple Silicon plus Apple Intelligence as
  explicit requirements; snapshot-stream semantics; usage truth typed as
  exact/estimated/unknown; Pylon transcript and tool-execution authority
  outside the model; Blueprint preflight for tool menus; Action Submission as
  the boundary for direct effects; admitted-Mac live tests separate from
  default CI; unsupported/unavailable distinct from failed.
