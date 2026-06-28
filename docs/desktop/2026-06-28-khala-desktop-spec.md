# Khala Desktop - Native macOS SwiftUI App Spec

> **Honest-scope header.** This is a direction/spec document, not a product
> promise. It describes the intended native macOS Khala Desktop surface and the
> contracts it should target. It does not guarantee a shipped feature, ship date,
> paid offering, or broad Apple Foundation Models capability. Live behavior is
> only what the running app, Pylon, and `openagents.com` APIs actually do. Route
> user-facing claims through `docs/promises/` before broadening copy.

- **Date:** 2026-06-28
- **Status:** spec-first desktop target
- **Owning future surface:** `clients/desktop/Khala/` or equivalent native
  macOS SwiftUI app target, to be created.
- **Sibling surface:** `clients/khala-ios/Khala/` native SwiftUI iOS app.
- **Core thesis:** every admitted Apple Silicon Mac can become a self-contained
  Khala node: local chat UI, local Pylon, local Apple Foundation Models backend,
  OpenAuth/agent identity, fleet visibility, and optional provider/earning mode.

## 1. Source Grounding

This spec is grounded in the current repository material below:

- `clients/khala-ios/Khala/README.md`, `clients/khala-ios/Khala/project.yml`, and
  `clients/khala-ios/Khala/Khala/KhalaApp.swift` show the current native SwiftUI
  Khala app shape: a ChatGPT-style client, local conversation storage, Keychain
  API key storage, streaming Khala API calls, markdown/code rendering, and a
  retained push-to-talk voice affordance.
- `docs/mobile/2026-06-26-khala-chatgpt-style-app-spec.md` is the active mobile
  app target. It supersedes the first voice-only slice while preserving voice as
  a runtime reference.
- `docs/mobile/2026-06-26-khala-voice-app-spec.md` defines the native Apple
  voice loop: `AVAudioEngine`, `SFSpeechRecognizer`, push-to-talk state, local
  Keychain storage, native Xcode builds, and the no-Expo/no-EAS policy.
- `apps/autopilot-desktop/src/shared/apple-fm-packaging.ts`,
  `apps/autopilot-desktop/scripts/apple-fm-live-smoke.ts`, and
  `apps/autopilot-desktop/scripts/verify-packaged-apple-fm-bridge.ts` define
  the existing packaged Apple FM bridge contract, live smoke shape, and pre-
  notarization packaging gate for the `foundation-bridge` helper.
- `apps/pylon/docs/probe-port/2026-06-07-apple-fm-first-backend-audit.md`,
  `apps/pylon/docs/probe-port/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`,
  and `apps/pylon/docs/probe-port/apple-fm-admitted-mac-acceptance.md` define
  the Apple FM backend posture: attach/readiness first, typed availability,
  `apple-foundation-model`, `apple_fm_bridge`, exact-vs-estimated usage truth,
  fake-bridge CI, admitted-Mac live acceptance, and no global Codex parity
  claim.
- `docs/transcripts/201.md` frames "Fracking Apple Silicon": idle Apple Silicon
  as stranded compute that becomes useful market supply when paired with
  routing, receipts, settlement, and trust. `docs/transcripts/README.md` places
  this in the compute-market arc.
- `docs/transcripts/194.md` and `docs/transcripts/195.md` contain the "trillion
  dollar question" framing around Apple Silicon carrying a meaningful fraction
  of future inference and developer adoption of Apple Foundation Models plus
  MLX as one watched signal.
- `docs/transcripts/214.md` shows the compute-market launch framing: a Mac user
  clicks "Go Online", advertises Apple FM compute, and can earn Bitcoin for
  useful work while preserving honest unsupported/unavailable states.

## 2. Product Intent

Khala Desktop is the Mac sibling of the mobile Khala app, but it is not just a
larger chat window. It is the "boots everything" app for a user's local Khala
node:

1. Launch the native macOS app.
2. Discover or start the user's local Pylon.
3. Check Apple FM availability through the local bridge.
4. Register Apple FM as a Pylon/Khala inference backend when admitted.
5. Authenticate the user/agent identity.
6. Bring the Pylon online with truthful capability refs.
7. Let the user chat, inspect fleet/agent status, and optionally earn as a
   provider node.

The first-run success state is simple: the app opens to a usable Khala chat
surface, shows local node readiness, and makes it obvious whether Apple FM and
provider mode are ready, unavailable, or owner-action-required.

## 3. Current vs Future

Current, grounded facts:

- The mobile app already exists as native SwiftUI under `clients/khala-ios/Khala/`.
- The Khala API is OpenAI-compatible at `https://openagents.com/api/v1`, with
  the single public model `openagents/khala`.
- The repo has Apple FM bridge packaging and smoke material in
  `apps/autopilot-desktop/`.
- Pylon already has Apple FM backend audits, status/smoke runbooks, and typed
  acceptance posture for admitted Macs.

Future work proposed here:

- A new native macOS SwiftUI Khala Desktop target.
- A desktop app supervisor that embeds or connects to a local Pylon.
- An app-level Apple FM bridge lifecycle that reuses the existing bridge
  contract instead of inventing a second local model path.
- A unified desktop UX for chat, fleet/agent status, local backend readiness,
  and provider/earning mode.

This document should not be read as a live product promise. It is an
implementation target and boundary record.

## 4. Architecture

### macOS App Shell

Khala Desktop should be a native SwiftUI macOS app, not Electron, React Native,
Expo, or a web wrapper. The app should use:

- SwiftUI `NavigationSplitView` for the larger desktop layout.
- Keychain for local bearer/API material.
- `URLSession` for Khala API and local Pylon control calls.
- `AVFoundation` and Apple `Speech` only where voice input is supported on
  macOS.
- Local app storage for UI preferences and non-secret state.

The default layout:

- **Left sidebar:** conversations, local Pylon status, connected accounts, and
  provider mode entry.
- **Main pane:** ChatGPT-style Khala conversation view with markdown/code-block
  rendering and streaming responses.
- **Right inspector:** node readiness, Apple FM backend status, fleet capacity,
  recent assignments, receipts, and provider earnings/status.
- **Toolbar:** model pill fixed to `Khala`, new chat, voice input, node online
  toggle, and settings.

The mobile app's `Conversation`, `ConversationStore`, `KhalaClient`,
`KeychainStore`, markdown/code rendering, and settings concepts should be ported
or shared where possible. The desktop shell should not fake model variants:
Khala remains one model, `openagents/khala`.

### Pylon Supervisor

Khala Desktop should support two Pylon modes:

1. **Connect to existing Pylon:** detect a running local Pylon control endpoint,
   authenticate via local control/session token, and show current identity,
   accounts, capacity, and assignments.
2. **Boot bundled Pylon:** if no suitable local Pylon exists, launch the bundled
   Pylon binary/runtime with an app-managed `PYLON_HOME` under the user's
   Application Support directory.

The bundled mode must never touch default Codex homes or unrelated local
credentials. It should follow the same isolation posture described in
`AGENTS.md`: per-account homes, no printing tokens, and no destructive login
"checks".

The Pylon supervisor is responsible for:

- ensuring one active node identity for the app-managed Pylon home;
- starting/stopping the local control server;
- publishing heartbeat/capacity when the user goes online;
- exposing assignment and proof status to the UI;
- providing local-only diagnostics without leaking prompts, auth paths, wallet
  material, raw command output, or private repo data into public views.

### Apple FM Backend

Apple FM should be registered as a first-party local backend only when the
bridge reports truthful readiness. The backend identity should preserve the
existing audit vocabulary:

- backend family: `apple_fm_bridge`
- profile id: `apple-fm-local`
- default model id: `apple-foundation-model`
- default base URL: `http://127.0.0.1:11435`
- readiness gate: `GET /health`
- override order for lower-level runtime: `PROBE_APPLE_FM_BASE_URL`, then
  `OPENAGENTS_APPLE_FM_BASE_URL`, then default loopback
- stream semantics: Apple FM snapshots, not fake token deltas
- usage truth: exact, estimated, or unknown; never fake exact tokens

Desktop should reuse the packaged helper contract from
`apps/autopilot-desktop/src/shared/apple-fm-packaging.ts`: the helper belongs at
`Contents/Resources/app/apple-fm-bridge/foundation-bridge` inside the signed
`.app`, with the same non-empty/executable verification before notarization.

MVP should be attach/readiness plus plain text and bounded tool-use where the
existing Pylon/Probe contracts already support it. It should not claim Apple FM
is a full Codex replacement. Unsupported machines are `unsupported` or
`unavailable`, not `failed`.

### Khala Chat

The desktop chat surface should mirror the mobile app's current direction:

- single model label: `Khala`;
- streaming OpenAI-compatible chat completions;
- local conversation history;
- markdown, bullets, headings, and fenced code blocks with copy actions;
- typed prompts and optional push-to-talk;
- clear 402/free-quota and network error handling;
- first-use disclosure for free-tier data sharing when minting or using a free
  key.

When local Apple FM is available, the app may show it as a local backend/source
status for node/provider work. It should not silently replace the public Khala
model label with "Apple FM"; the user should be able to see which route handled
local provider assignments and which remote Khala route served chat.

### Fleet, Agent, and Provider Surfaces

Desktop has room for the whole Khala node surface:

- connected OpenAgents identity and local agent identity;
- local Pylon readiness and heartbeat freshness;
- connected provider accounts such as Codex refs, when present;
- Apple FM backend readiness and blockers;
- advertised capacity refs, including counted refs such as
  `capacity.coding.codex.available=N` where applicable;
- current load, queued assignments, recent closeouts, and proof refs;
- provider mode toggle: offline, online, online-with-Apple-FM, online-with-
  coding-capacity;
- earnings/receipt summary with unsettled, rejected, credited, and settled
  states clearly distinguished.

Provider mode should default to truthful readiness rather than optimistic
availability. If Apple Intelligence is disabled, the bridge is missing, the Mac
is not admitted, or the bundled helper is not executable, the provider surface
must show a blocker and avoid advertising that capacity.

### Auth and Identity

Desktop needs two identity layers:

- **User/API auth:** OpenAgents agent bearer key or OpenAuth-backed token used
  for Khala API calls and account-linked operations. Store secrets in Keychain.
- **Local node identity:** Pylon identity, wallet/agent refs, and local
  capability state stored under the app-managed Pylon home.

The app should support:

- mint/paste an `oa_agent_...` key for chat;
- OpenAuth login/linking for richer account/fleet/provider functions;
- safe display of account email/refs when returned by Pylon/OpenAgents;
- no raw token display after capture;
- public-safe logs by default, with owner-only raw traces behind explicit local
  diagnostics.

## 5. Boots-Everything UX

The first launch should feel like one operation, not a setup checklist:

1. **Welcome / connect:** user signs in or enters/mints an agent key.
2. **Node home:** app creates or selects the app-managed Pylon home.
3. **Pylon start:** local Pylon boots or the app attaches to an existing one.
4. **Apple FM check:** bridge readiness is checked; if missing, the app explains
   that Apple FM needs an admitted Apple Silicon/Apple Intelligence environment
   and the packaged helper.
5. **Khala connected:** chat surface is usable even when Apple FM is
   unavailable.
6. **Go online:** optional provider mode publishes only verified capabilities.
7. **Earn/serve:** provider rows show accepted assignments, closeouts, token
   accounting, and receipt state.

The key UX invariant: chat should work before provider mode is perfect, but
provider claims must be honest. A user on an unsupported Mac can still use Khala
chat; they simply cannot advertise Apple FM capacity.

## 6. Reuse vs Build

### Reuse

Reuse or port from `clients/khala-ios/Khala/`:

- Khala API client shape, including streaming SSE and 402 handling.
- Keychain storage pattern.
- conversation/message model and local persistence concepts.
- markdown/code-block rendering behavior.
- settings disclosure pattern for free keys.
- voice input state machine where macOS APIs support it.

Reuse from `apps/autopilot-desktop/`:

- Apple FM packaged helper path and verification contract.
- Apple FM live smoke expectations and redaction posture.
- local control/readiness command shapes where they already talk to Pylon.
- installed app packaging lessons for signed/notarized helpers.

Reuse from Pylon/Probe docs and code:

- Apple FM backend vocabulary and profile ids.
- readiness-first attach semantics.
- usage truth and backend receipt posture.
- admitted-Mac acceptance boundary.
- provider online/heartbeat/capacity concepts.

### Build New

Build new for Khala Desktop:

- native macOS SwiftUI `NavigationSplitView` app.
- Pylon supervisor layer for bundled-vs-existing local Pylon.
- app-managed Pylon home and setup flow.
- desktop fleet/provider dashboard.
- Apple FM bridge lifecycle UI and install/readiness repair guidance.
- notarized DMG/TestFlight distribution lane for this app.
- local owner-only diagnostics viewer with public-safe redaction by default.

## 7. Build, Distribution, and Release

Khala Desktop should follow the repo's native Apple build policy:

- **No Expo/EAS.** Expo applies to the retired React Native mobile app only if a
  future Expo app is explicitly reintroduced.
- **Build locally with Xcode/xcodebuild.** Use a native `.xcodeproj` or
  XcodeGen-generated project committed with enough structure to open in Xcode.
- **Signing team:** Apple Team `HQWSG26L43`.
- **Distribution targets:**
  - development builds through Xcode;
  - TestFlight for Mac app beta if the team chooses App Store Connect
    distribution;
  - signed and notarized Developer ID `.app`/`.dmg` for direct desktop
    distribution.
- **Notarization:** codesign with hardened runtime, notarize, staple the `.app`,
  create the DMG from the stapled app, then sign/notarize/staple the DMG.
- **Apple FM helper gate:** run a Khala Desktop equivalent of
  `verify-packaged-apple-fm-bridge.ts` before notarization so the shipped app
  cannot claim Apple FM support while missing the helper.

The first desktop release lane should have its own runbook under `docs/desktop/`
or the future app directory, then be linked from `docs/DEPLOYMENT.md` before any
public release.

## 8. MVP Cut

MVP should ship only the pieces needed to prove the desktop node loop honestly:

1. Native macOS SwiftUI app opens to a usable Khala chat surface.
2. Keychain-backed auth and local chat history.
3. Local Pylon attach/start with visible readiness.
4. Apple FM readiness through the existing bridge contract.
5. Provider mode can go online only with truthful advertised capability refs.
6. Fleet/provider dashboard shows local status, recent assignments, blockers,
   and closeout/proof refs.
7. Signed/notarized build bundles the Apple FM helper or clearly marks Apple FM
   unavailable.

Non-goals for MVP:

- no fake model picker;
- no global Apple FM coding parity claim;
- no marketplace settlement broadening beyond current Pylon receipt states;
- no pooled third-party capacity routing by default;
- no public traces containing raw prompts, raw shell output, local paths, auth
  material, wallet material, or private repo data;
- no OTA path.

## 9. Risks and Open Questions

- **Apple FM availability:** only admitted Apple Silicon/Apple Intelligence
  environments should pass live readiness. The UX must make unsupported normal,
  not broken.
- **Bridge packaging:** a signed app without the helper is a false green. The
  packaging gate must be part of release.
- **Pylon lifecycle:** supervising Pylon inside a GUI app needs careful
  ownership of ports, homes, logs, and crash recovery.
- **Identity confusion:** user auth, Pylon identity, agent refs, account refs,
  and provider capacity refs need crisp labels.
- **Provider claims:** "online" must mean the exact capabilities advertised by
  heartbeat are fresh and available; stale or blocked capability must not appear
  as earnable supply.
- **Usage accounting:** Apple FM usage may be estimated or unknown. Public
  counters and proof claims should not synthesize exact token rows from local
  estimates.
- **Distribution split:** TestFlight and notarized DMG may have different
  sandbox/entitlement and helper-launch constraints; release docs must cover
  both.
- **Privacy:** local raw Pylon/Codex/Apple FM diagnostics may contain sensitive
  material. The desktop app should default to public-safe summaries and keep raw
  detail local/owner-only.

## 10. Done-When for First Implementation

The first implementation should be considered complete when:

- the macOS app builds locally through Xcode/xcodebuild;
- chat against `openagents/khala` works with a Keychain-stored key;
- app start either attaches to an existing Pylon or boots an app-managed Pylon;
- Apple FM readiness shows ready/unavailable/unsupported with typed blockers;
- provider mode publishes only verified capabilities;
- a local no-spend/fixture provider assignment can be accepted and closed out
  when the required capability is available;
- packaging verification proves the Apple FM helper is inside the signed app
  bundle when Apple FM support is advertised;
- release docs state whether the build is TestFlight, notarized DMG, or both;
- `bun run --cwd apps/openagents.com check:deploy` remains green before the PR
  lands.
