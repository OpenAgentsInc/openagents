# Native Desktop Helpers Specification

Status: Design  
Scope: `apps/openagents-desktop/`, `apps/openagents-desktop/helpers/`, `coax/lib/agent/desktop/`, `apps/openagents-desktop/linux/`, `apps/openagents-desktop/windows/`

## 1. Purpose

OpenAgents Desktop (`apps/openagents-desktop/`) currently gives agents shell access, file editing, and iOS Simulator control (`oasim`, see `docs/desktop/ios-simulator-spec.md`). To support full computer-use workflows, sandboxed scripting, and hybrid passkey/BLE flows on par with Antigravity's native helper suite (`agy-desktop-helper`, `agy-desktop-script`, `cable_ble_scanner`), OpenAgents Desktop needs three small, out-of-process native helper binaries:

1. **`oa-desktop-helper`** — Screen/window capture, Accessibility (AX) tree inspection/hit-testing, and HID mouse/keyboard actuation.
2. **`oa-desktop-script`** — Isolated out-of-process JavaScript evaluator (and optional opt-in OS automation bridge).
3. **`oa-cable-ble-scanner`** — Bluetooth Low Energy (BLE) advertisement scanner for FIDO2 / caBLE v2 hybrid authentication.

This spec defines the Coax-first protocol boundary, macOS-only initial implementation and bundling in `OpenAgents.app`, strict platform guards so Linux and Windows builds remain unaffected, and the cross-platform roadmap for Windows and Linux equivalents.

---

## 2. Architectural Principles

1. **Coax owns policy, wire protocol, and grants; native helpers are thin OS adapters.**
   - Request/response framing, capability negotiation, permission state, and per-chat approval rules live in Coax (mirroring `oasim` and `coax/lib/agent/device/sim-wire.coax`).
   - Native helpers contain only OS framework calls (e.g. `ScreenCaptureKit`, `AXUIElement`, `CGEvent`, `JavaScriptCore`, `CoreBluetooth`) and speak a versioned NDJSON protocol over `AF_UNIX` (for long-lived `serve` daemons) or `stdin`/`stdout` (for one-shot/streaming helpers).
2. **Strict macOS-only loading and compilation.**
   - Helper compilation, bundling, and codesigning happen **only** in the macOS build (`apps/openagents-desktop/build.sh` and `sign.sh`).
   - Runtime discovery and spawning in the desktop host are gated behind `#if os(macOS)` (Swift) / `#if defined(__APPLE__)` (C) and an explicit capability probe.
   - `apps/openagents-desktop/linux/` and `apps/openagents-desktop/windows/` never reference, link, or expect macOS helper binaries. On those platforms, the shared capability layer returns a structured `unsupported_platform` response until native Windows/Linux backends are implemented.
3. **Out-of-process isolation.**
   - Running capture, AX traversal, JS evaluation, and BLE scanning in separate helper processes prevents framework crashes, `ScreenCaptureKit` stalls, or runaway scripts from taking down `OpenAgents.app`.
4. **Explicit user consent and visibility.**
   - Arbitrary desktop control (`oa-desktop-helper`) is distinct from driving OpenAgents' own windows (`docs/desktop/macos-drive-and-screenshots.md`). Any agent action that captures the user's screen or injects HID input requires an explicit per-chat grant from the user (same model as `oasim` returning `no_grant` when unapproved) and displays an active-control indicator in the window with a one-click/hotkey stop.

---

## 3. Helper Specifications (macOS First)

### 3.1 `oa-desktop-helper` (Screen Capture, Accessibility, & Input)

- **macOS Frameworks:** `ScreenCaptureKit`, `VideoToolbox`, `CoreMedia`, `CoreVideo`, `CoreGraphics`, `ApplicationServices` (`AXUIElement`), `AppKit`, `Foundation`.
- **Minimum OS:** macOS 14.0 (matches `LSMinimumSystemVersion` in `apps/openagents-desktop/Info.plist`).
- **CLI Modes:**
  - `oa-desktop-helper probe` — Prints a single JSON object reporting OS version, TCC permission status (`screen_recording`, `accessibility`), available displays, and helper version, then exits `0`.
  - `oa-desktop-helper serve [--socket PATH]` — Listens on a `0600` `AF_UNIX` socket (default `~/.openagents/run/desktop-helper.sock`) and serves NDJSON RPC requests.
- **Capabilities (RPC methods):**
  - **Discovery:** `displays` (list screens, bounds, backing scale), `windows` (list on-screen windows with `windowId`, owner PID/bundle ID, title, bounds, off-display flag).
  - **Capture:** `screenshot` (full display or region via `SCScreenshotManager` / `SCStream`), `window_screenshot` (single `CGWindowID` via `SCContentFilter`), encoded as PNG or JPEG/HEVC frame.
  - **Accessibility:** `ax_tree` (bounded `AXUIElement` walk returning role, title, value, bounds, enabled/focused state, secure-field redaction for `AXSecureTextField`), `ax_hit` (element at `(x, y)`), `ax_action` (`AXPress`, `AXScrollToVisible`, `AXIncrement`, `AXDecrement`, etc.).
  - **Input Actuation:** `click`, `double_click`, `mouse_move`, `drag`, `scroll`, `type`, `key` via `CGEvent` (`CGEventCreateMouseEvent`, `CGEventCreateKeyboardEvent`, `CGEventPost`).
- **Reliability:**
  - Watchdog timer on `ScreenCaptureKit` frame callbacks (e.g. 5s). If capture stalls, the helper exits non-zero with a structured `capture_stalled` error so the host supervisor restarts it cleanly.
  - Never log or return contents of `AXSecureTextField` nodes.

### 3.2 `oa-desktop-script` (Sandboxed JavaScript Evaluator)

- **macOS Frameworks:** `JavaScriptCore`, `Foundation`.
- **CLI Modes:**
  - `oa-desktop-script eval [--timeout-ms N] [--no-jit]` — Reads a JSON request `{ "script": "...", "args": ... }` from `stdin`, evaluates it in a fresh `JSContext`, and writes `{ "ok": true, "result": ... }` or `{ "ok": false, "error": "..." }` to `stdout`.
- **Safety:**
  - Runs with strict wall-clock timeout (process killed on expiry) and optional `--no-jit` (`JSC_useJIT=0`) for untrusted snippets.
  - Pure `JSContext` has no filesystem or network bindings by default. Any optional macOS app automation (Apple Events / JXA) must be a separate opt-in flag gated by `NSAppleEventsUsageDescription` and user approval.

### 3.3 `oa-cable-ble-scanner` (caBLE v2 / FIDO2 BLE Scanner)

- **macOS Frameworks:** `CoreBluetooth`, `Foundation`.
- **CLI Modes:**
  - `oa-cable-ble-scanner [--uuids FFF9,FD6E] [--timeout-ms N]` — Initializes `CBCentralManager`, prints `READY` once Bluetooth is powered on, scans for matching FIDO/caBLE service UUIDs, and streams discovered advertisement packets (service data, RSSI, timestamp) as NDJSON lines on `stdout` until stopped or timed out.
- **Use Case:**
  - Enables hybrid passkey / WebAuthn caBLE v2 flows (e.g. phone-to-desktop authentication without manual codes) alongside `KitPasskey.swift`.

---

## 4. Integration with `apps/openagents-desktop/` (macOS)

### 4.1 Repository Layout

```text
apps/openagents-desktop/
  helpers/
    macos/
      desktop-helper/        # Swift sources -> oa-desktop-helper
      desktop-script/        # Swift sources -> oa-desktop-script
      cable-ble-scanner/     # Swift sources -> oa-cable-ble-scanner
    windows/                 # Future: Windows native helpers (stub/README)
    linux/                   # Future: Linux native helpers (stub/README)
```

Shared wire types, capability probing, and `#authority` grant checks live in `coax/lib/agent/desktop/helpers-wire.coax` and `coax/lib/agent/desktop/helpers-policy.coax`.

### 4.2 Build & Bundling (`apps/openagents-desktop/build.sh`)

- `apps/openagents-desktop/build.sh` (which is macOS-only) compiles the three Swift helpers for `arm64-apple-macos14.0` and copies them into `OpenAgents.app/Contents/Resources/` (or `Contents/Helpers/`), alongside `oasim`, `mcworker`, and `coder-host`.
- Add a build flag (e.g. `OA_SKIP_MAC_HELPERS=1`) for fast UI-only iteration if needed, defaulting to enabled on macOS.
- `apps/openagents-desktop/linux/build.sh` and `apps/openagents-desktop/windows/build.sh` are **not** modified to build or bundle these macOS binaries.

### 4.3 Signing, Entitlements & `Info.plist`

- **`apps/openagents-desktop/Info.plist`** (macOS only) adds usage descriptions required by macOS TCC:
  - `NSScreenCaptureUsageDescription` — explains why OpenAgents captures the screen when the user grants an agent desktop access.
  - `NSBluetoothAlwaysUsageDescription` — explains why OpenAgents scans for nearby passkey/caBLE devices.
  - `NSAppleEventsUsageDescription` — only if/when opt-in Apple Events automation is enabled.
- **`apps/openagents-desktop/sign.sh`:**
  - Inside-out signing: sign each bundled helper binary with `--force --options runtime` (and `--timestamp` for Developer ID / notarization) **before** signing `OpenAgents.app`.
  - Verify with `codesign --verify --deep --strict` and `spctl -a -vv --type exec` during notarization (`sign_notarize`).

### 4.4 Runtime Discovery, Platform Guarding & Supervision

- **macOS (`apps/openagents-desktop/DesktopHelpers.swift`, compiled only by macOS `build.sh`):**
  - Mirror `SimHostClient` in `Window+Simulator.swift`: resolve helper paths from `Bundle.main.resourcePath` (with `OA_DESKTOP_HELPER`, `OA_DESKTOP_SCRIPT`, `OA_CABLE_BLE_SCANNER` env overrides for worktree testing).
  - Guard all helper loading with `#if os(macOS)` and `FileManager.default.isExecutableFile(atPath:)`.
  - Lazy startup: do not spawn `oa-desktop-helper serve` or `oa-cable-ble-scanner` at app launch (avoiding premature macOS TCC permission prompts). Spawn only when a user-approved feature or agent tool invokes it.
  - Clean teardown: terminate child helper processes in `AppDelegate.applicationWillTerminate`.
- **Worker `PATH` (`Worker.swift`):**
  - `Contents/Resources` is already appended to worker `PATH` (for `oasim`). Agents invoke a thin Coax CLI wrapper (e.g. `oadesk`) rather than calling raw helper binaries directly, so every call passes through the `#authority` grant check.
- **Linux & Windows (`apps/openagents-desktop/linux/main.c`, `apps/openagents-desktop/windows/main.c`, `host-core.coax`):**
  - Expose a platform capability bitmask from the host (`OA_CAP_DESKTOP_HELPER`, `OA_CAP_DESKTOP_SCRIPT`, `OA_CAP_CABLE_BLE`), initialized to `0` on Linux and Windows until their native backends exist.
  - Any request for an unavailable capability returns `{ "ok": false, "code": "unsupported_platform", "platform": "linux|windows" }` without attempting to spawn a macOS binary.

---

## 5. Cross-Platform Roadmap (Windows & Linux)

Because the wire protocol and Coax policy layer are OS-agnostic, Windows and Linux support can be added incrementally by implementing the same CLI/NDJSON contracts using native OS APIs:

| Capability | macOS (Phase 1) | Windows (Future) | Linux (Future) |
| --- | --- | --- | --- |
| **Screen & Window Capture** | `ScreenCaptureKit` (`SCScreenshotManager` / `SCStream`), `VideoToolbox` | `Windows.Graphics.Capture` (WGC) or DXGI Desktop Duplication (`IDXGIOutputDuplication`) | Wayland: `xdg-desktop-portal` `ScreenCast` + PipeWire; X11 fallback: `XShmGetImage` / `XComposite` |
| **Accessibility Tree** | `ApplicationServices` (`AXUIElement`) | Windows UI Automation (`IUIAutomation` / `UIAutomationCore.dll`) | AT-SPI2 over D-Bus (`org.a11y.Bus`) |
| **HID Input Injection** | `CoreGraphics` (`CGEventPost`) | Win32 `SendInput` | Wayland: `xdg-desktop-portal` `RemoteDesktop` + `libei`; X11: `XTest` |
| **Sandboxed JS Evaluator** | `JavaScriptCore` (`JSContext`) | Embedded QuickJS (cross-compilable via `zig cc` in `windows/build.sh`) or ChakraCore/V8 | Embedded QuickJS or `libjavascriptcoregtk` |
| **caBLE / FIDO2 BLE Scanner** | `CoreBluetooth` (`CBCentralManager`) | WinRT `Windows.Devices.Bluetooth.Advertisement.BluetoothLEAdvertisementWatcher` | BlueZ over D-Bus (`org.bluez.Adapter1` / `org.bluez.Device1`) |
| **Transport** | `AF_UNIX` (`0600`) / stdio | Windows `AF_UNIX` (Win10 1803+) or Named Pipes (`\\.\pipe\oa-*`) / stdio | `AF_UNIX` (`0600`) / stdio |
| **Build Toolchain** | `swiftc` in `apps/openagents-desktop/build.sh` | `zig cc` (C/C++/WinRT) in `apps/openagents-desktop/windows/build.sh` | `cc` + `pkg-config` in `apps/openagents-desktop/linux/build.sh` |

### Cross-Platform Notes

- **QuickJS for `oa-desktop-script`:** Using QuickJS on Windows and Linux (or even optionally across all platforms) keeps the binary small (<1 MB), avoids heavy runtime dependencies, and cross-compiles cleanly with `zig cc` for Windows `x64`/`arm64`.
- **Wayland Security Model:** On Linux Wayland, compositors intentionally forbid silent global screen capture and input injection. The Linux `oa-desktop-helper` must integrate with `xdg-desktop-portal` (`apps/shared/host/linux/portal.c` already exists as a starting point) and persist restore tokens so the user grants access once per session.
- **Windows UIPI / Elevation:** Win32 `SendInput` cannot inject input into elevated (Administrator) windows from a standard-privilege process. The Windows helper should detect elevated foreground windows and return a structured `elevated_target` error.

---

## 6. Phased Rollout & Verification

1. **Phase 1 — Wire Contract & Platform Guards:**
   - Define `helpers-wire.coax` and `helpers-policy.coax` (`#authority`) with unit tests.
   - Add capability bitmask to `host-core.coax` and verify Linux (`linux/build.sh`) and Windows (`windows/build.sh --check`) builds pass with zero macOS references.
2. **Phase 2 — `oa-desktop-script` (macOS):**
   - Smallest surface, no TCC permissions required. Build in `build.sh`, sign in `sign.sh`, add timeout/sandbox tests.
3. **Phase 3 — `oa-desktop-helper` (macOS):**
   - Implement `probe` first, then `serve` (`displays`, `windows`, `screenshot`, `ax_tree`, `CGEvent` actions) behind per-chat user grants and an active-control UI banner.
   - Add integration tests covering `no_grant` refusal, `AXSecureTextField` redaction, and capture-stall restart.
4. **Phase 4 — `oa-cable-ble-scanner` (macOS):**
   - Implement `CoreBluetooth` scanner, add `NSBluetoothAlwaysUsageDescription`, and wire into passkey/pairing flows.
5. **Phase 5 — Windows & Linux Backends:**
   - Implement native helpers under `apps/openagents-desktop/helpers/{windows,linux}/` and flip the corresponding capability bits per platform as each backend reaches parity.
