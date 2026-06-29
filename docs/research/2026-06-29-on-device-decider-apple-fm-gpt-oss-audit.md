# On-Device Decider Audit — Apple FM + GPT-OSS (2026-06-29)

Audit of the optional **on-device decider** in `clients/khala-code-desktop`: a
small, local model the desktop app can use to make fast, private
routing/triage/yes-no decisions without a network round trip. It selects a
platform-appropriate backend behind one uniform contract — **Apple Foundation
Models** on Apple-Silicon macOS (and, later, iOS), **self-hosted GPT-OSS**
everywhere else — and is exercised end-to-end on real Apple FM hardware below.

Status: **working and tested.** Apple FM path verified live on this Mac; GPT-OSS
path verified by contract + mocked transport; selection/fallback verified by
unit tests; the desktop app exposes it over RPC.

## Why this exists

Khala Code needs a cheap, private "decider" for small choices (route this turn
locally vs. to the cloud, classify intent, quick yes/no) that should not cost a
cloud call or leak the prompt off-device. Apple Silicon has a genuinely
on-device model (Apple FM); other hosts don't, so the self-hosted GPT-OSS
deployment is the drop-in. The decider unifies both behind one contract and is
**optional + fails soft** — if nothing is available it reports so and callers
fall back to their normal path. It is never a hard dependency.

## Architecture

```
                    OnDeviceDecider (shared/on-device-decider.ts)
                    ├─ preferredOnDeviceBackend(platform)  ← the selection policy
                    ├─ select()  → probe backends in order, pick first available
                    └─ decide()  → route inference to the selected backend
                          │
          ┌───────────────┴────────────────┐
   apple_fm backend                   gpt_oss backend
 (bun/apple-fm-decider-backend)   (bun/gpt-oss-decider-backend)
   └─ foundation-bridge HTTP        └─ self-hosted OpenAI-compatible
      (Apple FoundationModels)         endpoint (vLLM-style)
```

- **Contract** — `src/shared/on-device-decider.ts` (pure, transport-agnostic):
  the `OnDeviceDeciderBackend` interface (`probe` + `complete`), the message /
  usage / readiness / result types, the `OnDeviceDeciderUnavailable` error, and
  the selection policy.
- **Selection policy** — `preferredOnDeviceBackend({platform, arch})`:
  Apple-Silicon macOS (`darwin`+`arm64`) and `ios` prefer `apple_fm`; everything
  else prefers `gpt_oss`. `backendSelectionOrder` puts the preferred backend
  first and the other as fallback. Pure function → identical on host + view,
  unit-tested.
- **Apple FM backend** — `src/bun/apple-fm-decider-backend.ts`: HTTP client for
  the `foundation-bridge` helper's loopback OpenAI contract (`GET /health`,
  `POST /v1/chat/completions`). Optionally nudges the sidecar to launch the
  helper, but trusts the direct `/health` as its readiness source of truth.
- **GPT-OSS backend** — `src/bun/gpt-oss-decider-backend.ts`: HTTP client for the
  self-hosted GPT-OSS OpenAI-compatible endpoint (`GET /v1/models`,
  `POST /v1/chat/completions`), configured by `KHALA_GPT_OSS_BASE_URL` /
  `KHALA_GPT_OSS_API_KEY` / `KHALA_GPT_OSS_MODEL`. Unconfigured → unavailable
  (not an error).
- **Host wiring** — `src/bun/on-device-decider-host.ts` assembles both backends
  with the real platform and the shared sidecar; `src/bun/index.ts` exposes
  `onDeviceDeciderStatus()` over RPC (host handler + webview caller in
  `src/ui/main.ts`).

## The Apple FM bridge (the underlying tool)

`apps/pylon/swift/foundation-bridge` is a Swift 6.2+ executable using Apple's
`FoundationModels` framework (`SystemLanguageModel.default`,
`LanguageModelSession.respond`). It serves a loopback OpenAI-compatible API
(default `127.0.0.1:11435`): `/health`, `/v1/models`, `/v1/chat/completions`,
and minimal session endpoints. `clients/khala-code-desktop/scripts/prepare-apple-fm-bridge.sh`
builds it and copies the binary into the app's `resources/apple-fm-bridge/`;
`verify-packaged-apple-fm-bridge.ts` asserts the helper is bundled into a built
`.app` before signing.

## Live verification (this Mac)

Host: macOS 26.4, arm64 (Apple Silicon); Swift 6.3.3.

1. **Build** — `prepare:apple-fm-bridge` → `swift build -c release` clean;
   helper installed to `resources/apple-fm-bridge/foundation-bridge`.
2. **Availability** — `GET /health` → `{"ready":true, "message":"Apple
   Foundation Models is available.", "model":"apple-foundation-model"}`.
3. **Direct inference** — `POST /v1/chat/completions` "Reply with exactly: PONG"
   → `"PONG"`; "what is the Khala fleet?" → honest *"I don't have any
   information…"* (correct for an on-device model with no OpenAgents knowledge).
4. **Full decider, end-to-end** — the real `createOnDeviceDeciderHost()` on this
   Mac:
   - `select()` → `selected: "apple_fm"`, `preferred: "apple_fm"`, reason
     *"preferred backend apple_fm is available"*; `gpt_oss` readiness
     `available:false, "no GPT-OSS endpoint configured"` (correct here).
   - `decide("is 17 prime? Answer only YES or NO")` → backend `apple_fm`,
     content **`YES`** (correct), usage `{prompt:12, completion:1, total:13,
     truth:"estimated"}`.

## Tests

- `tests/on-device-decider.test.ts` (14 cases): the selection contract
  (platform → preferred), evaluation order, preferred-available, fallback when
  preferred is down, non-Mac selects gpt_oss, none-available → null + `decide`
  throws `OnDeviceDeciderUnavailable`, a throwing probe is non-fatal, `decide`
  routes to the winner, unconfigured backend is unavailable.
- `tests/on-device-decider-backends.test.ts` (6 cases): Apple FM probe
  ready/unreachable + completion mapping + sidecar nudge non-fatal; GPT-OSS
  unconfigured/served probe + OpenAI usage mapping + env-driven config & auth.
- Whole `khala-code-desktop` suite: **41/41 pass**, typecheck clean.

## What is and isn't gated

- **Decider readiness ≠ product-promise readiness.** The decider uses the
  helper's direct `/health`. The separate `apple-fm-readiness.ts` `state:"ready"`
  / `available:true` (product-promise evidence) is deliberately stricter: it only
  flips ready via the **Pylon control-plane** path (`POST /command
  {type:"apple_fm.status"}` with a control token), so production readiness is
  supervised, not just "a local port answered." This audit does **not** change
  that contract; the decider is an additive consumer of the same helper.
- **GPT-OSS endpoint is config-driven.** No bundled non-Mac model; the drop-in
  points at the self-hosted GPT-OSS deployment via env. Unconfigured is a clean
  "unavailable," never a crash.
- **Privacy.** Readiness `detail` strings are short and public-safe; the GPT-OSS
  bearer token is read from env and never logged. The bridge itself logs only
  startup/listener failures, never prompts or bodies.

## Follow-ups (not blocking)

1. Expose a `decide(messages)` RPC (currently only `onDeviceDeciderStatus()` is
   wired; `decide` is exercised via the host) once a concrete in-app caller
   exists, with bounded max-tokens.
2. A dev affordance to surface the decider selection in the desktop UI.
3. Optional: let the Apple FM backend reuse the sidecar's adopted port instead
   of the default when the sidecar launches the helper itself.

## Verdict

The on-device decider is real, selects the right backend per platform, and runs
genuine on-device inference on Apple Silicon today, with the GPT-OSS drop-in
ready for non-Mac hosts by configuration. Optional, fail-soft, and tested.
