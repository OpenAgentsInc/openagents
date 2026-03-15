# Apple Foundation Models Bridge: Considerations in Full Detail

This document describes the **FM bridge**—the Swift HTTP sidecar that exposes Apple’s Foundation Models to the Rust desktop app—and all considerations for building, running, testing, shipping, and integrating it. It is the single reference for bridge behavior, discovery, packaging, and user-facing requirements.

## 1. What the bridge is and why it exists

- **Apple’s Foundation Models** are only callable from **Swift** (and from the Python SDK, which wraps system APIs). The OpenAgents desktop app is Rust; it cannot call the Foundation Models framework directly.
- The **foundation-bridge** is a small **Swift HTTP server** that:
  - Runs as a **localhost sidecar** (default `http://127.0.0.1:11435`).
  - Exposes an **OpenAI-compatible** API so the Rust client can use a simple HTTP contract.
  - Calls Apple’s Foundation Models framework under the hood for health, model listing, chat completions, sessions, streaming, and structured generation.
- **Rust ownership**: The reusable **contract, client, and types** live in **`crates/psionic/psionic-apple-fm`**. The **process supervision, binary discovery, auto-build, and UI integration** live in **`apps/autopilot-desktop`** (e.g. `apple_fm_bridge.rs`). The **Swift implementation** lives in **`swift/foundation-bridge/`**.

So: Swift talks to Apple; Rust talks to the bridge over HTTP; the desktop app owns “where is the binary, is it running, what does the user see.”

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  apps/autopilot-desktop                                          │
│  - Finds or builds foundation-bridge binary                      │
│  - Spawns / supervises bridge process                            │
│  - Mission Control UI, Go Online preflight, workbench            │
│  - Uses psionic_apple_fm::AppleFmBridgeClient (HTTP)             │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP (127.0.0.1:11435)
┌────────────────────────────▼────────────────────────────────────┐
│  swift/foundation-bridge (executable)                            │
│  - GET /health, GET /v1/models, POST /v1/chat/completions, etc.  │
│  - Calls Apple Foundation Models framework                      │
└────────────────────────────┬────────────────────────────────────┘
                             │ Swift / system
┌────────────────────────────▼────────────────────────────────────┐
│  Apple Foundation Models (macOS 26+, Apple Silicon,              │
│  Apple Intelligence enabled)                                    │
└─────────────────────────────────────────────────────────────────┘
```

- **`psionic-apple-fm`**: transport-neutral types and HTTP client. No process management, no UI.
- **`apple_fm_bridge.rs`**: discovers binary, optionally runs `build.sh`, spawns bridge, polls health, pushes snapshots to the app, drives Mission Control “Start / Refresh Apple FM” and “Test Local FM.”

## 3. Contract (URL, endpoints, readiness)

- **Default base URL**: `http://127.0.0.1:11435`. Override with **`OPENAGENTS_APPLE_FM_BASE_URL`**.
- **Key endpoints** (see `psionic_apple_fm::contract` and `swift/foundation-bridge/README.md`):
  - **`GET /health`** — Returns system model availability, supported use cases, guardrails. Used to decide “is the bridge up and is the system model ready?”
  - **`GET /v1/models`** — List of model IDs (e.g. `apple-foundation-model`).
  - **`POST /v1/chat/completions`** — OpenAI-style chat completion.
  - Session, stream, and structured-generation endpoints are also part of the
    live bridge contract.
  - Adapter-management surfaces are now implemented in the retained Swift
    bridge:
    - **`GET /v1/adapters`**
    - **`POST /v1/adapters/load`**
    - **`DELETE /v1/adapters/{adapter_id}`**
    - **`POST /v1/sessions/{session_id}/adapter`**
    - **`DELETE /v1/sessions/{session_id}/adapter`**
  - Session create, session response, structured response, and one-shot chat
    completion requests can also carry optional adapter selections. Session
    attach is durable; per-request adapter overrides are temporary and do not
    mutate the session’s default binding.

- **Readiness** (from the app’s point of view): the bridge is **ready** when:
  - **Reachable**: `GET /health` succeeds.
  - **Model available**: the health response indicates the system model is available (not disabled or unavailable).
  - **Ready model**: we have a non-empty model ID (from health or list_models) to use for requests.
  - **Adapter state**: health now also reports adapter-inventory support,
    adapter attach/detach support, and the currently loaded adapter inventory.

If the bridge process is running but Apple Intelligence is off (or the system model is unavailable for another reason), the app sees “reachable but not ready” and should tell the user to enable Apple Intelligence (see user requirements below).

## 4. Binary discovery (where the app looks for the bridge)

The desktop app looks for the **`foundation-bridge`** binary in this order:

1. **`OPENAGENTS_APPLE_FM_BRIDGE_BIN`** — If set, use this path (must exist).
2. **CWD-relative** (when run from repo root):
   `bin/foundation-bridge`,
   `swift/foundation-bridge/.build/release/foundation-bridge`,
   `swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge`.
3. **Exe-relative repo root**: walk up from the current executable path until a directory contains `swift/foundation-bridge` or `bin/foundation-bridge`; then check `bin/foundation-bridge` and the same `.build/` paths under that root. This lets the app find the binary when run from `target/debug` or elsewhere under the repo.
4. **Bundled with the app** (for shipped .app):
   - **Next to the executable**: `YourApp.app/Contents/MacOS/foundation-bridge`.
   - **In Resources**: `YourApp.app/Contents/Resources/foundation-bridge`.

If none of these yield an existing binary, the app may try to **auto-build** once (see below). If the app is **running from an .app bundle** (path contains `.app/Contents/`), it does **not** tell the user to build; it tells them the app was not packaged with the bridge and to reinstall or get a complete build.

## 5. Build and run (developers and CI)

- **Build** (from repo root):
  `cd swift/foundation-bridge && ./build.sh`
  Produces **`bin/foundation-bridge`** and ad-hoc signs the copied binary so
  macOS will launch the rebuilt sidecar locally. The bridge is written in
  **Swift**, so the build requires the **Swift compiler** (Xcode from the App
  Store, or **`xcode-select --install`** for Command Line Tools only—no full
  Xcode needed).
- **Run**:
  `./bin/foundation-bridge`
  Default port **11435**. Optional: `./bin/foundation-bridge 8080` for a different port (and set `OPENAGENTS_APPLE_FM_BASE_URL` accordingly).
- **Test**:
  `curl -s http://127.0.0.1:11435/health`
  You should get a JSON object with system model availability. Before working on autopilot or Mission Control, **test the bridge first** (build → run → curl health), then start the desktop app. See **AGENTS.md** and the main **README.md** “Agent Install Instructions” for the canonical “test bridge first” workflow.

## 6. Shipping the app (no build on user machines)

To ship so **users never need to build the bridge or install Xcode**:

1. **Build the bridge once** (on your machine or in CI):
   `cd swift/foundation-bridge && ./build.sh`
   → produces `bin/foundation-bridge`.
2. **Include that binary in your app bundle** when you create the .app:
   - Put **`foundation-bridge`** in **`YourApp.app/Contents/MacOS/`** (next to your main executable), or in **`YourApp.app/Contents/Resources/`**. The app checks both.
   - For other layouts, set **`OPENAGENTS_APPLE_FM_BRIDGE_BIN`** to the full path of the binary in your package.
3. Users then only need: **macOS 26+**, **Apple Silicon**, and **Apple Intelligence enabled**. No Xcode or build step.

If you do **not** bundle the binary:
- **From source** (e.g. `cargo run`): the app may run **`./build.sh`** once; if that fails (e.g. no Swift), the user sees instructions to install Xcode or run `xcode-select --install` and restart the app.
- **From a shipped .app**: the app detects it is in a bundle and shows a **“bridge missing from this app”** message instead of “run build.sh,” so users don’t get incorrect build instructions.

## 7. User requirements (what “not ready” can mean)

For the **system model** to be available (and thus for “Go Online” to unblock on the Apple FM lane):

- **macOS 26+** on **Apple Silicon**.
- **Apple Intelligence enabled**:
  **System Settings → Apple Intelligence** (in the sidebar) → turn on Apple Intelligence.

If the bridge is **reachable** but the system model is **not available**, the framework typically returns an availability reason (e.g. Apple Intelligence disabled). The app surfaces this and should direct the user to enable Apple Intelligence at the path above.

Other “not ready” cases:
- **Binary missing** → build instructions (from source) or “app not packaged with bridge” (from .app).
- **Bridge not running** → the app can auto-start it (when binary is found) or the user can use “Start Apple FM” in Mission Control.
- **Health check failed / timeout** → bridge may have crashed or not be listening; check logs and binary.

## 8. Agent and developer workflow (test bridge first)

When working on autopilot, Mission Control, or Apple FM:

1. **Build the bridge**: `cd swift/foundation-bridge && ./build.sh`.
2. **Run the bridge**: `./bin/foundation-bridge` (leave running or in another terminal).
3. **Verify**: `curl -s http://127.0.0.1:11435/health` — confirm JSON response.
4. **Then** run or test the desktop app (`cargo autopilot` or equivalent).

This is codified in **AGENTS.md** (“Apple FM bridge (agent)”) and in the main **README.md** “Agent Install Instructions” paste prompt. The bridge is the dependency; testing it first avoids confusing “health check failed” or “not ready” errors that are really “bridge wasn’t running.”

## 9. References

- **Swift bridge implementation and user-facing build/ship steps**: [swift/foundation-bridge/README.md](../../../swift/foundation-bridge/README.md).
- **Rust contract and client**: `crates/psionic/psionic-apple-fm` (contract, client, health, sessions, streaming, tools, transcript).
- **Desktop supervision and UI**: `apps/autopilot-desktop/src/apple_fm_bridge.rs` (discovery, auto-build, spawn, refresh, Mission Control).
- **Apple adapter reference specs and fixtures**:
  - `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`
  - `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
  - `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md`
  - `crates/psionic/fixtures/apple_adapter/`
- **Roadmap and API coverage**: [ROADMAP_FM.md](ROADMAP_FM.md), [FM_API_COVERAGE_MATRIX.md](FM_API_COVERAGE_MATRIX.md).
- **Audit of current bridge vs contract**: `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`.
- **Agent rules and test-bridge-first**: repo root **AGENTS.md** and **README.md**.
