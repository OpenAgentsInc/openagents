# os-june Reference Audit for OpenAgents

Date: 2026-06-30
Status: research audit / adaptation guide, not a product promise
Source studied: `projects/repos/os-june` at
`5feb9ae9b0c33cef75f554dadf04a14a2cf2d6bf`
Target codebase: OpenAgents monorepo

This is a public-safe source study of Open Software's June repository. The
reference repo was treated as read-only. The goal is not to vendor June code,
copy its brand, or move OpenAgents back toward a Rust/Tauri monolith. The goal
is to extract patterns that can improve OpenAgents product trust, Khala/Pylon
UX, metered inference boundaries, desktop overlays, and release evidence.

## Executive summary

June is a compact but unusually complete example of a private desktop AI app
connected to a metered, privacy-sensitive backend. Its strongest ideas are not
individual components. They are contracts:

- A desktop client is treated as a public client and never owns upstream
  provider keys, app billing keys, or charge authority.
- The backend is split into domain traits, services, providers, config, and API
  handlers, with typed pricing and explicit authorize -> work -> charge flow.
- The trust story is user-facing: the README, `/verify` page, attestation
  walkthrough, and reproducible-build docs all say what is proven and what is
  not.
- Onboarding is shaped as one ask per screen, with just-in-time permissions,
  live status feedback, and an explicit distinction between inference privacy
  and agent action risk.
- Ambient desktop state is handled with tiny, non-disruptive HUD surfaces that
  preserve keyboard focus and encode status without forcing a dashboard open.
- Live previews are not allowed to become authority. Saved audio is the retry
  source of truth; preview failure does not fail the final recording.
- Full integration QA is framed as an evidence contract first, then promoted to
  deterministic automation once it proves stable.

For OpenAgents, the most valuable adaptations are:

1. A Khala verification/proof surface that mirrors June's "source, image,
   attestation, limitations" structure while using OpenAgents-owned proof
   primitives and avoiding claims that only a TEE can support.
2. A tighter Khala metering error taxonomy: distinguish insufficient credits,
   transient capacity/authorization denial, metering dependency failure,
   upstream provider failure, timeout, and invalid model at the API boundary.
3. A first-run Khala Code / Pylon connect flow that uses June's onboarding
   mechanics: one ask per screen, permission previews, live checks, and a
   mandatory honesty step for owner-local agent authority.
4. A Pylon/Codex fleet HUD pattern: compact status pill, aggregate state,
   needs-user attention state, bounded list, stable dimensions, and no focus
   theft.
5. A release and compatibility discipline for bundled/connected agent runtimes:
   pin note, version agreement check, fixture replay, live smoke, and known-gap
   matrix.

Do not adapt June's GitHub Actions substrate, light/warm visual identity, icon
set, Tauri/Rust workspace shape, or any TEE language for OpenAgents surfaces
that do not actually run in a remotely attested confidential VM.

## Source scan

Primary source files and areas read:

- Product overview and privacy contract:
  `projects/repos/os-june/README.md`, `CONTEXT.md`.
- Onboarding and permission strategy:
  `docs/onboarding-design.md`,
  `src/components/onboarding/OnboardingFlow.tsx`,
  `src/components/onboarding/StepChrome.tsx`,
  `src/components/onboarding/steps/SignInStep.tsx`,
  `src/components/onboarding/steps/PermissionSteps.tsx`,
  `src/components/onboarding/steps/PracticeStep.tsx`,
  `onboarding-preview.html`.
- Account and funding gates:
  `src/components/account/AccountGate.tsx`,
  `src/components/account/FundingGate.tsx`.
- App shell and UI tokens:
  `src/styles/tokens.css`, `src/styles/app.css`,
  `src/app/App.tsx`.
- Agent and recording overlays:
  `agent-hud.html`, `meeting-hud.html`,
  `src/styles/agent-hud.css`, `src/styles/meeting-hud.css`,
  `src-tauri/src/agent_hud.rs`, `src/lib/agent-events.ts`.
- Backend architecture and metering:
  `docs/june-api-prd.md`, `docs/os-accounts-backend.md`,
  `june-api/Cargo.toml`, `june-api/config.toml`,
  `june-api/crates/domain/src/lib.rs`,
  `june-api/crates/api/src/lib.rs`,
  `june-api/crates/api/src/envelope.rs`,
  `june-api/crates/api/src/error.rs`,
  `june-api/crates/api/src/auth.rs`,
  `june-api/crates/api/src/state.rs`,
  `june-api/crates/api/src/handlers/models.rs`,
  `june-api/crates/api/src/handlers/verify.rs`,
  `june-api/crates/services/src/charge_flow.rs`,
  `june-api/crates/services/src/note_transcribe.rs`,
  `june-api/crates/services/src/pricing.rs`.
- Live transcript and source-of-truth discipline:
  `docs/adr/0002-live-transcript-preview-strategy.md`,
  `specs/003-conversation-turns/plan.md`.
- QA, release, and security discipline:
  `docs/qa/agent-driven-integration.md`,
  `docs/reproducible-builds.md`,
  `docs/release-macos.md`,
  `docs/github-security-readiness.md`,
  `docs/hermes-upgrade-checklist.md`.

## What June is

June is a Tauri desktop app for meeting notes, dictation, and local agent work.
The app stores state, recordings, transcripts, sessions, and agent memory on
the user's machine by default. Model calls go through June API, a backend that
holds provider keys, verifies OS Accounts tokens, meters credits, and routes
model calls through configured upstream providers.

The repo is both desktop client and backend:

```text
src/        React app, onboarding, account gates, settings, agent UI
src-tauri/  Tauri v2 Rust core, audio capture, OS permissions, native HUDs
june-api/   Rust backend, pricing, metering, provider adapters, verify page
docs/       product, release, security, backend, and QA notes
specs/      feature specs with plans, contracts, tasks, validation notes
```

The terminology is disciplined. `CONTEXT.md` distinguishes June, June API, OS
Accounts, upstream provider, dictation, note transcription, note generation, and
credit price. That is not cosmetic. It keeps product copy, code boundaries, and
billing surfaces aligned.

## Adaptation opportunities

### 1. Make trust verification a product surface

June's `/verify` handler is small but strategically important. It renders an
unauthenticated human-readable page from inside the backend. The page lists the
running version, source commit, source repo, image tag, and attestation report,
then explains how a verifier walks the chain. It also has a section for what
the chain does not cover.

OpenAgents should adapt the pattern, not the exact TEE claim:

- Add or harden a Khala proof page that reads like a verifier's checklist, not
  a marketing block.
- Structure it as "this deployment", "why this matters", "check it yourself",
  and "what this does not cover".
- Link proof refs to existing OpenAgents receipts, public token rows, model
  route disclosures, product-promise records, and Worker version/deploy refs.
- Keep non-TEE routes honest. If a surface is a Cloudflare Worker and not a
  TEE-backed service, do not reuse confidential-VM wording.
- For any future TEE or enclave-backed Khala route, copy June's separation
  between code attestation and upstream model-provider behavior.

OpenAgents already has receipt-backed public proof instincts. June's lesson is
presentation: make the proof legible to a skeptical developer and to an agent
parsing the page.

### 2. Split metered inference errors by actual failure domain

June API has a crisp API error taxonomy:

- `401` invalid or missing access token.
- `402` insufficient credits.
- `422` invalid model or unpriced model.
- `429` transient authorization denial, such as a concurrency cap, with
  `Retry-After`.
- `502` upstream provider failure.
- `503` metering provider failure.
- `504` timeout.

The key detail is that a funded user who hits a concurrency cap does not see
"add funds", and a billing dependency outage does not look like an LLM provider
outage. This is a product-support multiplier.

OpenAgents should adapt this for Khala and Pylon:

- Normalize Khala pay-loop and OpenAI-compatible API errors into a typed
  envelope with stable error codes, even when the HTTP shape stays compatible.
- Keep route capacity, pylon readiness, payment/funding, upstream, timeout, and
  model-not-supported failures distinct from the first user-visible response.
- Preserve machine-readable retry hints for agent callers.
- Add tests for the embarrassing cases: capacity denial must not render as
  depleted credits; metering failure must not render as provider failure;
  unknown model must fail before spending.

### 3. Treat pricing and model privacy as catalog data

June's model catalog is not just a list of model names. `june-api/config.toml`
stores provider, model type, pricing unit, display name, privacy class,
context tokens, traits, and capabilities. The `/v1/models` handler returns only
priced models and includes a human-readable price description.

OpenAgents should align Khala model discovery with this:

- Every public model row should carry provider/route, model type, privacy
  posture, priced unit, capabilities, and a stable price explanation.
- The UI should not let a user select a model/route that the server cannot
  price, meter, or explain.
- Capabilities should be typed: tool use, reasoning, web search, response
  schema, local/remote execution, streaming, and receipt support should be
  explicit fields instead of inferred from display names.
- Public copy should distinguish "private", "anonymized", "zero retention",
  "local", and "hosted" rather than flattening them into one trust adjective.

This fits OpenAgents' existing product-promise and receipt vocabulary. The new
work is making the catalog visible, strict, and route-aware.

### 4. Use onboarding as a consent contract

June's onboarding design doc is worth adapting even where its final
implementation is shorter. The durable mechanics:

- One ask per screen.
- Show or preview the scary system prompt before asking for it.
- Explain every permission in one sentence.
- Verify hardware/permission state before the first practice moment.
- Teach by doing, not by a tour.
- Put an explicit honesty screen in front of agent authority.
- Separate inference privacy from action risk.

OpenAgents should apply this to Khala Code, Pylon connect, and caller-owned
coding delegation:

```text
Sign in
-> connect local runtime or Codex account
-> verify capacity/readiness
-> run a safe fixture task
-> explain owner-local authority and approval boundaries
-> enter real work
```

Potential screens:

- "Connect your coding account" with a device-login or local account check.
- "What Khala can do locally" with exact permission rows: read workspace, edit
  approved files, run commands, use network, use browser, spend credits.
- "Verify your Pylon" with live readiness rows for token, heartbeat, capacity,
  account readiness, and closeout path.
- "Run one fixture" where success is a real local proof, not just a completed
  animation.
- "Before you give it real work" that says owner-local full access is powerful,
  approvals/receipts matter, and private inference does not make actions on the
  internet private.

OpenAgents already has strong runbooks for Khala -> Pylon -> Codex. June's
onboarding pattern can turn those runbooks into an ergonomic first-run flow.

### 5. Centralize gates instead of scattering blockers

`AccountGate` and `FundingGate` are simple but effective: full-screen gate
surfaces handle sign-in, upgrade, billing recovery, polling, "check again",
reopen portal, and sign-out. The rest of the app does not need to rediscover
those states.

OpenAgents should use the same pattern for:

- Khala API key/account readiness.
- Pylon registration and heartbeat readiness.
- Caller-owned coding delegation readiness.
- Wallet/funding/credit gates.
- Model route unavailable or policy-blocked gates.

These should be command surfaces, not marketing cards, to match OpenAgents'
dark operational UI. But the structural pattern is right: one gate owns one
class of readiness and recovery.

### 6. Borrow the ambient HUD concept for Pylon and fleet state

June's agent HUD is a top-right passive status surface. It is intentionally
small, always on top, visible on all workspaces, skip-taskbar, and macOS
non-activating so it does not steal focus while the user is typing. The status
model is compact: received, starting, running, waiting for user, completed,
failed, cancelled. The collapsed pill aggregates counts; expanded state shows
up to a bounded set of sessions; clicking opens the real app.

OpenAgents should adapt this for the desktop/operator side of Pylon and Khala:

- Collapsed: "2 running", "1 needs input", "idle", or "offline" with one mark
  and one count.
- Expanded: bounded session/assignment rows with title, latest status, and
  user-needed attention.
- No focus theft on background updates.
- Stable native/window dimensions with tests. June tests that the HUD width is
  constant and that expansion grows downward.
- Status color and shimmer should map to OpenAgents tokens, not June's warm
  terracotta palette.

This is especially relevant for a Codex fleet. Users need ambient assurance
that work is moving and a clear nudge when human input is needed. They do not
need a dashboard to interrupt every state change.

### 7. Keep live previews separate from authoritative records

The live transcript ADR has a valuable architecture rule: live preview helps
confidence, but finalized saved audio remains the retry source of truth and the
source for final transcripts/notes. Preview events are ephemeral UI state,
stored separately, visually distinguished, and discarded or reconciled when
final processing completes. Preview failure never fails recording.

OpenAgents has analogous surfaces:

- Streaming Khala chat deltas versus final completion receipt.
- Pylon live progress versus accepted outcome.
- Agent activity HUD rows versus closeout evidence.
- Public stats projections versus settlement/payment truth.

Recommendation:

- Label live states as preview/current progress, not final proof.
- Render final receipts, accepted outcomes, and settlement refs differently
  from transient streaming text.
- Do not let preview telemetry become product-promise authority.
- In tests, prove preview failure does not corrupt final closeout or receipt
  generation.

### 8. Use agent-driven QA as a promotion lane, not a replacement for tests

June's agent QA doc makes a useful distinction:

1. Deterministic tests for contracts and logic.
2. Agent-driven live QA for native/product workflows, with video/log evidence
   and explicit gaps.
3. Promotion of stable live walkthroughs into deterministic automation later.

OpenAgents should adapt the evidence contract:

- command used;
- surface tested;
- data mode;
- pass/fail checks;
- screenshots, logs, or video when available;
- explicit gaps for native hardware, permission prompts, real accounts, live
  spend, or production-only paths.

This fits the existing OpenAgents QA-runner and product-promise discipline.
Important constraint: do not copy June's GitHub Actions approach. OpenAgents'
root invariant bans GitHub-hosted CI/cloud actions. Run this through
OpenAgents-owned runners or manual/agent-triggered evidence paths.

### 9. Add compatibility matrices for bundled or connected runtimes

June's Hermes upgrade checklist is a strong release pattern:

- one pinned runtime version;
- one compatibility matrix constant;
- one pin note for the upstream version;
- fixture replay for wire/event behavior;
- live smoke against the runtime;
- known gaps listed honestly as planned or unsupported;
- release-note copy limited to supported UI behavior.

OpenAgents should adapt this for:

- Pylon CLI/runtime releases;
- Khala Code desktop runtime;
- Codex/Claude account integration surfaces;
- browser-control and local-tool presets;
- model-provider adapters that claim structured/tool/streaming support.

The pattern matters because OpenAgents surfaces are already multi-runtime. A
dependency can add a feature, change a wire event, or break an approval flow
without breaking typecheck. A matrix plus replay fixtures catches that drift.

### 10. Preserve spec discipline for large product changes

June's `specs/003-conversation-turns` folder carries a feature plan, data
model, contracts, quickstart, tasks, and validation notes. It also names
constraints clearly: no realtime captions, no speaker diarization, saved audio
remains source of truth.

OpenAgents should keep doing this for high-risk changes, but bias it into the
existing docs layout:

- use `docs/research/` for external studies and synthesis;
- use `docs/blueprint/`, `docs/khala/`, `docs/ops/`, or app-local docs for
  implementation specs;
- include contracts and validation notes when a change crosses runtime,
  payment, public claim, or authority boundaries;
- convert meaningful counterexamples into tests or invariant updates.

## What not to copy

- **Do not copy GitHub Actions workflows.** June uses GitHub Actions for
  release, image build, and CI. OpenAgents explicitly forbids GitHub-hosted CI
  and scheduled automation. Adapt the evidence model to owned runners.
- **Do not copy the Rust/Tauri monorepo shape wholesale.** OpenAgents'
  implementation baseline is Bun, Effect, Effect Schema, Cloudflare Workers,
  Foldkit, and shared packages. The transferable idea is layering, not crate
  names.
- **Do not copy June's visual identity.** June's warm neutral, terracotta,
  serif-friendly desktop app language is not OpenAgents' dark operational
  surface. Translate mechanics into black/blue/mono OpenAgents tokens.
- **Do not copy icon dependencies.** June deliberately uses `central-icons`.
  OpenAgents should follow its own UI/package conventions.
- **Do not claim TEE attestation where none exists.** June can make a Phala/TDX
  claim for June API. OpenAgents Worker surfaces should use receipt/projection
  proof unless a future route actually runs in an attested enclave.
- **Do not make preview text authoritative.** June is disciplined here. We
  should copy the discipline, not accidentally weaken it.
- **Do not hide private evidence in public docs.** June's docs are careful
  about provider keys and app keys. OpenAgents docs must keep tokens, prompts,
  wallet material, raw private repo content, and private customer data out of
  public records.

## Already aligned in OpenAgents

OpenAgents already has several patterns that line up with June:

- `packages/khala-tools` already separates bounded model output, structured UI
  payloads, private local artifacts, and public-safe summaries.
- OpenAgents product-promise docs already separate shipped claims from roadmap.
- Khala/Pylon delegation docs already distinguish public chat, owner-local
  Pylon routing, and admin/operator assignment authority.
- Receipts, token rows, public projections, and payout/settlement boundaries
  already exist as first-class concepts.
- Root invariants already require typed boundaries and reject ad hoc routing.

The audit therefore recommends focused UX/proof hardening, not a wholesale
architecture transplant.

## Concrete next steps

1. **Khala proof page.** Add a human/agent-readable verification page for the
   current Khala endpoint. Include route, deploy/version ref, model route
   disclosure, receipt links, product-promise status, and limitations.
2. **Khala error envelope audit.** Compare current Khala OpenAI-compatible and
   pay-loop errors against June's taxonomy. Add tests for insufficient credits,
   transient capacity, metering failure, upstream failure, timeout, and
   unpriced/unsupported model.
3. **Model catalog hardening.** Ensure Khala public model discovery exposes
   provider/route, model type, price unit, privacy posture, capabilities, and
   receipt support. Hide or block unpriced routes before spend.
4. **Pylon connect onboarding spec.** Write a first-run flow for connecting a
   local Pylon/Codex account: sign in, verify account, heartbeat/capacity check,
   fixture task, owner-local authority honesty screen.
5. **Fleet HUD design spec.** Design a compact OpenAgents-status HUD for Pylon
   and Codex-fleet activity. Borrow June's collapsed/expanded state model and
   stable geometry tests, translated to OpenAgents tokens.
6. **Live-vs-final receipt UI rule.** Add a design/implementation note that
   streaming chat, assignment progress, and HUD state are preview/progress only;
   final receipts and accepted outcomes remain the authority.
7. **Runtime compatibility matrix.** Create a matrix/checklist for Pylon and
   Khala Code runtime dependencies, modeled after June's Hermes checklist but
   run through OpenAgents-owned test infrastructure.
8. **Agent QA evidence contract.** Extend the existing OpenAgents QA runner
   docs with June-style live QA fields: command, surface, data mode, evidence
   artifacts, pass/fail checks, and explicit gaps.

## Product mapping table

| June pattern | OpenAgents adaptation | Priority |
| --- | --- | --- |
| `/verify` attestation walkthrough | Khala proof/check page with route, receipts, deploy refs, and limits | High |
| `authorize -> upstream -> charge` services | Khala pay-loop and Pylon assignment error taxonomy | High |
| priced model catalog | Khala model discovery with price/privacy/capability metadata | High |
| one-ask onboarding | Pylon/Codex connect first-run flow | High |
| Account/Funding gates | Khala/Pylon readiness and credit gates | Medium |
| agent HUD and meeting HUD | Ambient Pylon/fleet activity surface | Medium |
| preview not source of truth | Streaming chat/progress vs final receipt rule | High |
| agent-driven integration QA | OpenAgents-owned runner evidence contract | Medium |
| Hermes compatibility matrix | Pylon/Khala Code runtime matrix and fixture replay | Medium |
| GitHub Actions release flow | Do not adapt; replace with owned infra gates | N/A |

## Bottom line

June's best contribution to OpenAgents is a posture: privacy and billing are not
copy blocks added after the product; they are typed contracts, recovery flows,
proof pages, and UI gates. OpenAgents already has richer market, receipt, and
agent-runtime primitives. The work now is to make those primitives feel as
coherent to a first-time user and as checkable to a skeptical agent as June's
desktop/backend loop does.
