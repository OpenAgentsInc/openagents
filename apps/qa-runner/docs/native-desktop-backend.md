# Native-desktop execution backend

The qa-runner drives web targets (`runner.ts`, real chromium) and terminal/TUI
targets (`terminal-backend.ts`, a PTY). The **native-desktop backend**
(`src/native-desktop-backend.ts`) is the desktop-app sibling: it focuses a real
desktop application, reads its **OS accessibility (AX) tree**, synthesizes
click/type, captures **screenshots**, and records a public-safe `QaRunResult`
(`backend = "native-desktop"`) plus an AX-tree snapshot + timeline — reusing the
same `result.ts` schema + tripwire the browser/terminal runners use, so the
brain/target/artifact contracts are unchanged.

This is the real implementation of the `NativeDesktopDriver` seam that #6186
left **spec-only** in `src/backend.ts` (follow-up #6199).

## macOS driver choice (and why)

The runtime engine is injected behind `NativeDesktopRuntime`
(`src/native-desktop-runtime.ts`). The implemented tier is **macOS**, and it
shells out to the OS's own, dependency-free native tools:

- **`osascript` + AppleScript "System Events"** — reads the live AX tree
  (`role` / `title` / `value` per UI element) and synthesizes AX presses
  (`perform action "AXPress"`) and keystrokes. This goes through the **same
  macOS Accessibility API** that trycua/cua's `cua-driver` uses under the hood.
- **`screencapture`** (always present on macOS) — writes a PNG of the screen.
- **`cliclick`** (optional, Homebrew) — synthesizes a pointer click for the
  `point:x,y` selector fallback when an AX node is not directly addressable.

**Why not trycua/cua here?** trycua/cua (`cua-driver`, the backend droid-control
uses) is the eventual cross-OS option, but it is a Python/agent stack that must
be installed, and for the local-host path it would **still** require the same
macOS Accessibility permission to read the AX tree. The dependency-free
osascript/screencapture path gives a locally-runnable macOS-first driver now,
and because the engine is injected, a trycua/cua adapter can replace
`macosNativeDesktopRuntime` later **without touching the backend**.

`cliclick` is the only optional, non-OS binary, and only for the `point:x,y`
click fallback; the AX-press and screenshot paths use OS tools only.

## Requirement: macOS Accessibility permission (owner-grantable)

Reading the AX tree and synthesizing input require the **process driving the
tools** (your terminal / the test runner) to hold macOS Accessibility
permission:

> System Settings → Privacy & Security → Accessibility → enable the controlling
> app (e.g. Terminal / iTerm / the IDE running the tests).

This is **owner-grantable, not code-fixable**. `macosNativeDesktopRuntime()
.available()` is honest: it performs an actual System Events AX read and returns
`false` when the permission is absent (or `osascript` is missing), so the
backend refuses instead of faking.

## Owner-gated / armed by env (default OFF)

Driving a real desktop is privileged; it does not turn itself on. The backend is
**INERT** unless explicitly armed:

- env: `QA_NATIVE_DESKTOP=1` (or `true`), or
- code: `{ armed: true }`.

Un-armed → `NativeDesktopNotArmedError` (the runtime is never even touched).

## Honest about helper + permission

When **armed but the runtime is not usable** (helper binary missing, or macOS
Accessibility permission not granted), the backend throws
`NativeDesktopUnavailableError`. It **never silently falls back** and **never
fakes a result**.

## Scenario shape

A `NativeDesktopScenario` names an `app` and a list of steps:

| step | effect |
| --- | --- |
| `focus` | launch (if needed) + foreground the app |
| `ax-snapshot` | read the app's AX tree (kept for later assertions + artifact) |
| `screenshot` | capture a PNG into the run dir, recorded in the timeline |
| `click` | AX-press a `AXRole:Name` node, or `point:x,y` pointer fallback |
| `type` | keystroke text into the focused element (only the LENGTH is recorded — never the raw text, which may be a credential) |
| `wait` | bounded settle delay between live UI actions (clamped to 30 seconds, injectable in tests) |
| `assert-ax-contains` / `assert-ax-not-contains` | assert on the latest AX snapshot's roles/titles/values |

A failed assertion is a **real red**: `status = "fail"`, the failure reason is
recorded, and teardown still runs.

`nativeDesktopExample(app)` is the shipped deterministic example (focus → read
AX → screenshot → assert the AX tree has an `AXWindow`);
`nativeDesktopExampleWrong(app)` asserts a node that can never exist to prove a
red is a real red.

## Driver adapter

`nativeDesktopDriverFromRuntime(runtime, { app })` (in `src/backend.ts`) adapts a
`NativeDesktopRuntime` to the spec's `NativeDesktopDriver` contract, so the
runner stays driver-agnostic. The original spec-only `nativeDesktopDriver()`
stub is unchanged (it still throws `NativeDesktopDriverNotImplementedError`).

## Windows tier — spec only

`windowsNativeDesktopRuntime()` implements the identical `NativeDesktopRuntime`
contract but is **spec-only**: `available()` is honestly `false` and every action
throws. The Windows engine (UI Automation via PowerShell, or a trycua/cua
adapter) lands in a second pass.

## Deterministic in CI

The runtime is injected, so unit tests
(`src/native-desktop-backend.test.ts`) pass a **fake** `NativeDesktopRuntime`
(scripted AX tree + screenshot, no real desktop, no permission, no network) and
prove the full focus → read-AX → screenshot → assert → teardown lifecycle plus
the armed / un-armed / unavailable branches and the credential-redaction rule.
When macOS Accessibility permission **is** granted on the host, one real proof
focuses Finder, reads its real AX tree, screenshots it, and asserts a window
(otherwise it skip-lives and says the permission is owner-grantable).

## Khala Code packaged smoke

The first real Khala Code packaged-app drive is documented in
[`docs/qa/khala-code-packaged-native-ax-runbook.md`](../../../docs/qa/khala-code-packaged-native-ax-runbook.md).
It launches the built Electrobun `Khala Code.app`, uses fixture/no-spend
desktop env, drives hotbar -> composer -> send through this backend, and writes
boot/hotbar/submitted screenshots.

## Artifact contract is unchanged

The emitted `result.json` is the same public-safe `QaRunResult`
(`backend = "native-desktop"`) the browser/terminal runners emit. The AX-tree
snapshot and timeline are separate JSON artifacts; the screenshot PNG(s) and
both JSON artifacts are listed under `artifacts.screenshots` so the shared
`QaRunArtifacts` contract is unchanged. The public-safety tripwire
(`assertPublicSafeResult`) is applied to every artifact on write.
