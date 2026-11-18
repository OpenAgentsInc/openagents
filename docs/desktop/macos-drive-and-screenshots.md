# Driving and Capturing Screenshots for macOS Apps

Status: Design  
Scope: `apps/shared/host/`, `apps/openagents-desktop/`, `apps/coder-desktop/`, `apps/everglade-desktop/`, `apps/metaverse-engine-desktop/`, `coax/web/drive.coax`

## Purpose

Agents and automated checks need a single, deterministic way to launch or attach to a macOS application, inspect its UI and accessibility state, drive semantic and input actions without stealing the user's keyboard or mouse focus, advance time predictably, and capture repeatable screenshots.

This document specifies the unified drive-and-capture architecture for macOS apps in this repository. It builds on the shared `coax drive` socket and offscreen Metal host (`apps/shared/host/drive.c` and `apps/shared/host/apple/ui-host.m`) and extends the contract to cover hybrid and pure AppKit apps (`apps/coder-desktop/`, `apps/openagents-desktop/native/`) that combine Metal surfaces, native `NSView` trees, embedded terminal surfaces, sheets, and WindowServer chrome.

---

## Goals and Non-Goals

### Goals

1. **One driver protocol across all macOS app shapes.** Whether an app renders pure Coax GPU quads (`openagents-desktop` recovery shell, `everglade-desktop`), embeds Metal inside an AppKit chrome shell (`coder-desktop`), or renders native AppKit views driven by Coax Native (`openagents-desktop/native`), the driver speaks the same `coax drive` line/length-prefixed socket protocol.
2. **Zero focus stealing and zero modal hangs.** Driving and capturing an app must never move the user's cursor, steal active-application focus, pollute the general `NSPasteboard`, or block on an unseen `NSOpenPanel`, `NSSavePanel`, or alert sheet.
3. **Deterministic frames.** Animations, carets, clocks, backing scale (`2x` Retina), and asynchronous engine turns settle predictably under an explicit agent clock before a frame is read back.
4. **Permissionless default capture, fidelity capture when needed.** Default screenshots read back in-process textures and view bitmaps without requiring macOS Screen Recording (TCC) authorization. Optional WindowServer capture via `ScreenCaptureKit` captures system chrome, vibrancy, popovers, and child sheets by `CGWindowID` without capturing the user's desktop.
5. **Text first, pictures second.** Every screenshot capture pairs with a machine-readable element (`tree`) and `NSAccessibility` (`ax`) snapshot so structural regressions fail fast with line diffs before pixel comparison runs.

### Non-Goals

- Driving arbitrary third-party closed-source macOS apps via global `CGEventPost` injection. System-wide HID injection steals focus, requires Accessibility TCC in CI, and breaks parallel sessions.
- Introducing a second scripting DSL. Batch runs (`--do`) and interactive sessions (`--drive SOCKET`) execute the exact same operation grammar.

---

## Repository Context and App Topologies

macOS apps in this repository fall into three rendering topologies, all adhering to the repository's Coax-first boundary (state, element IDs, and operation dispatch in Coax; thin C/Objective-C/Swift carrier for OS surfaces):

| Topology | Examples | Rendering Surface | Current Drive / Shot Coverage |
| --- | --- | --- | --- |
| **1. Shared Metal Host** | `apps/openagents-desktop` (recovery), `apps/everglade-desktop` | Single `CAMetalLayer` / offscreen `MTLTexture` (`apps/shared/host/apple/ui-host.m`) | Full offscreen `--shot`, `--do`, `--drive`, `tree`, `ax`, `choose`, agent clock |
| **2. Hybrid AppKit + Metal / IOSurface** | `apps/coder-desktop`, `apps/metaverse-engine-desktop` | `NSWindow` + `NSTitlebar` + native `NSView` panes (e.g. `libghostty` IOSurface/Metal views) + embedded `UiHost` (`UiApp.embedded`) | Partial (`--preview-state` or custom flags); pure `h->target` readback misses AppKit siblings |
| **3. Coax Native AppKit** | `apps/openagents-desktop/native` | AppKit `NSWindow` + `NSView` hierarchy driven by Coax Native / JavaScriptCore bridge | Needs shared `UiDrive` adapter and view/window capture backend |

A complete macOS drive-and-capture system must support all three topologies behind one client (`coax/web/tool.sh drive native SOCKET OP...`) and one CLI surface.

---

## Architecture Overview

```text
+-------------------------------------------------------------------------+
| Driver Client (`coax drive native SOCKET ...` or batch `--do "..."`)    |
+------------------------------------+------------------------------------+
                                     | AF_UNIX stream socket (0600)
                                     | Req:  "<op>\n"
                                     | Resp: "<byte-len>\n<payload>"
                                     v
+-------------------------------------------------------------------------+
| macOS App Process (Main RunLoop / Shared Carrier)                       |
|                                                                         |
|  1. Transport & Clock (`apps/shared/host/drive.c`)                      |
|     - Non-blocking poll per frame (windowed) or blocking serve (shot)   |
|     - Agent clock (`clock +MS`, `clock settle [within MS]`)             |
|                                                                         |
|  2. Host Intercepts (`UI_DRIVE_HOST` on macOS)                          |
|     - `ax`, `ax-press ID`, `ax-key KEY` (NSAccessibility & NSEvent)     |
|     - `choose [PATH|done]`, `cancel` (headless NSOpenPanel stand-in)    |
|     - Isolated `NSPasteboard` (`pasteboardWithUniqueName`)              |
|     - `screenshot [FLAGS] OUT.png` -> Capture Engine                    |
|                                                                         |
|  3. Coax App Dispatch (`UiApp.drive` / Native Bridge)                   |
|     - `tree`, `about ID`, `tap ID`, `drag ID DX,DY`, `type ID TEXT`     |
|     - `key COMBO`, `menu ITEM`, `state NAME`, domain ops (`send`, etc.) |
|                                                                         |
|  4. Capture Engine (Three Tiers)                                        |
|     - Tier 0: Offscreen `MTLTexture` readback (pure Metal, no window)   |
|     - Tier 1: In-process Composite (AppKit `NSView` + Metal/IOSurface)  |
|     - Tier 2: `ScreenCaptureKit` `SCScreenshotManager` (window chrome)  |
+-------------------------------------------------------------------------+
```

---

## Wire Protocol and Execution Modes

### 1. Interactive Socket Mode (`--drive SOCKET`)

- **Socket:** `AF_UNIX` `SOCK_STREAM` bound to an explicit path (typically `$TMPDIR/oa-drive-<pid>.sock` or `~/.openagents/run/<app>-<id>.sock`) with mode `0600` (`umask(0177)` before `bind`).
- **Framing:** Matches `apps/shared/host/drive.c` and `coax/web/drive.coax`:
  - **Request:** One UTF-8 line per operation terminated by `\n` (`<op>\n`).
  - **Response:** Decimal byte length, `\n`, followed by exactly `<len>` UTF-8 bytes (`<len>\n<bytes>`).
- **RunLoop Integration:**
  - In pure Metal hosts, `ui_frame` calls `ui_drive_poll(h)` on each tick.
  - In AppKit-owned run loops (`NSApplicationMain` in `coder-desktop` and `openagents-desktop/native`), a `dispatch_source_t` (`DISPATCH_SOURCE_TYPE_READ` on the listening and client fds, targeting `dispatch_get_main_queue()`) accepts clients and dispatches `ui_drive` safely on the main thread even when no frame is scheduled.

### 2. Batch One-Shot Mode (`--shot OUT.png` + `--do "OP ; OP"`)

For CI checks and rapid single-frame generation without managing a daemon:

```sh
OpenAgents \
  --shot build/check/approval.png \
  --tree build/check/approval.tree \
  --size 2640,1760 \
  --still-clock \
  --do "access supervised ; send list the files ; clock settle ; state status"
```

1. Opens in offscreen or non-activating hidden-window mode with `ui_agent_clock(h, 1)`.
2. Renders one initial layout frame (`dt = 0`) so geometry and element IDs exist.
3. Splits `--do` on `;`, trimming surrounding whitespace (with `\;` escape for literal semicolons), and executes each operation sequentially through `ui_drive`.
4. Writes `--tree` (`tree` output) and `--ax` (`ax` output) if requested.
5. Captures `--shot OUT.png` and exits `0` (or non-zero if any operation returned an `error:` status).

---

## Operation Vocabulary

Operations are layered so host mechanics stay in C/Objective-C while semantics stay in Coax:

### Host-Owned Operations (`drive.c` + macOS `UI_DRIVE_HOST`)

| Operation | Behavior |
| --- | --- |
| `clock +MS` | Advances the agent clock by `MS` milliseconds in deterministic `1/60 s` (`16.67 ms`) steps (at least 1 frame), running layout/update on each step. |
| `clock settle [within MS]` | Steps frames `1/60 s` of wall time apart until `UiApp.busy(user)` returns `0` (no pending async stream, subprocess probe, or animation), up to `MS` (default `10000`). Fails loudly with `timeout` if still busy at deadline. |
| `screenshot [OPTS] OUT.png` | Renders if `stale`, captures pixels via the selected capture tier, writes an sRGB PNG atomically (`OUT.png.tmp` -> `rename`), and replies `wrote OUT.png WxH (tier=...)`. |
| `window size WxH [@SCALE]` | Sets logical window/viewport dimensions in points (or backing pixels in `--shot` mode) and backing scale factor (default `2.0`). |
| `ax` | Walks reachable `NSAccessibility` elements from the window/view root (`ui_ax_reached`), returning `<id> <role>[/<subrole>] "<label>" [value "..."] [disabled] [focused[ ring]] at X Y size WxH` per line. |
| `ax-press ID` | Invokes `accessibilityPerformPress` (`AXPress`) on the element matching `accessibilityIdentifier == ID`. |
| `ax-key KEY` | Synthesizes a window-local `NSEvent` key press (`tab`, `shift-tab`, `space`, `enter`, `escape`, arrows) delivered directly to the target `NSWindow` / first responder without touching the OS HID stream. |
| `choose` / `choose PATH` / `choose done` / `cancel` | Inspects or fulfills an intercepted `NSOpenPanel` / `NSSavePanel` request headlessly. |
| `status` | Reports host mode (`offscreen`, `hidden`, `windowed`), viewport `WxH@dpr`, `clock_ms`, `busy`, `choosing`, and `tcc_screen_capture`. |
| `quit` | Cleanly closes the drive session and terminates a `--shot` / headless run. |

### App-Owned Operations (`UiApp.drive` in Coax)

| Operation | Behavior |
| --- | --- |
| `tree` | Emits the Coax element table (`components/core/elements.coax`): `<id> <role> "<label>" at X Y size WxH`. |
| `about ID` | Returns `value`, `enabled`, `selected`, and bounds metadata for `ID`. |
| `tap ID` / `double-tap ID` | Dispatches pointer down/up at the center of `ID` (or invokes its semantic handler). |
| `drag ID DX,DY` / `scroll ID DX,DY` | Dispatches deterministic drag or wheel deltas in logical points against `ID`. |
| `type ID TEXT` | Focuses `ID` (or `-` for current focus) and inserts UTF-8 text via `NSTextInputClient` / Coax input handler. |
| `key COMBO` | Dispatches shortcut combinations (`cmd+b`, `alt+cmd+t`, `cmd+1`, `shift+up`) through the app's menu/key router. |
| `menu TITLE` | Triggers the `NSMenuItem` or Coax menu command with matching title or action ID. |
| `state NAME` | Queries deterministic app state projections (`status`, `desk`, `last`, `pasteboard`, `window`). |
| `fixture NAME` | Loads canned deterministic state (`demo`, `signed-out`, recorded stream) without live network calls. |

---

## Three-Tier Screenshot Capture Pipeline

Different macOS UI stacks require different readback paths. `screenshot` selects the lowest-overhead tier capable of representing the app's active views, and accepts an explicit override (`screenshot --tier=metal|composite|window OUT.png` or `--crop ID`).

### Tier 0: Offscreen Metal Target (`--tier=metal`)

- **Used when:** The app runs on `apps/shared/host/apple/ui-host.m` with `offscreen = 1` and has no native `NSView` children (`h->root == nil`).
- **Mechanism:**
  1. Allocates an `MTLTexture` with `MTLPixelFormatBGRA8Unorm`, `MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead`, and `MTLStorageModeShared` at exact backing dimensions `W x H`.
  2. Encodes the Coax vertex buffer (`ui_draw` or `ui_compose_draw`), commits the `MTLCommandBuffer`, and calls `[cmd waitUntilCompleted]`.
  3. Reads bytes synchronously via `[target getBytes:bytesPerRow:fromRegion:mipmapLevel:0]` and encodes sRGB PNG via `ImageIO` (`CGImageDestination` with `UTTypePNG`).
- **Properties:** Zero windows, zero TCC permissions, works over SSH and headless CI runners, ~1–3 ms per capture.

### Tier 1: In-Process Layer & View Composite (`--tier=composite`, Default for Windowed / Hybrid Apps)

- **Used when:** The app owns AppKit `NSView` hierarchies (`openagents-desktop/native`, `coder-desktop` chrome, or `ui_child` native subviews) and must run without Screen Recording TCC permissions or visible focus.
- **Window Lifecycle for Headless AppKit:**
  - Launch with `NSApp.setActivationPolicy(.prohibited)` (or `.accessory` when menu bar validation is tested).
  - Create the `NSWindow` with `backing: .buffered`, fixed `contentRect`, and force Retina backing metrics where supported or normalize scaling.
  - Place the window off-focus (`window.orderBack(nil)` with `alphaValue = 0.0` or offscreen origin `(-16000, -16000)` while retaining backing store allocation) so the user never sees a flashing window.
  - Under `--drive` or `--shot`, set every `CAMetalLayer.framebufferOnly = NO` so drawable textures permit CPU/blit readback.
- **Compositing Steps:**
  1. Flush pending layout and display passes on the main thread:
     ```objc
     [window.contentView layoutSubtreeIfNeeded];
     [CATransaction flush];
     ```
  2. Allocate an sRGB `NSBitmapImageRep` / `CGContextRef` at `bounds.size * backingScaleFactor`.
  3. Render the AppKit view tree via `[contentView cacheDisplayInRect:contentView.bounds toBitmapImageRep:rep]`.
  4. For GPU-backed subviews that `cacheDisplayInRect:` leaves blank (`CAMetalLayer`, `MTKView`, `libghostty` surfaces, or embedded `UiView`), walk the subview tree in back-to-front z-order:
     - Force a synchronous present/draw into a readable `MTLTexture` (or snapshot the last presented texture retained when `drive` mode is active).
     - Convert the subview's frame into root backing coordinates and composite the texture over (or under, when `h->under` is set) the `CGContextRef`.
  5. If `--crop ID` is given, look up the element's bounding rect in `tree` or `ax`, convert to pixel coordinates, and crop before PNG encoding.
- **Properties:** Captures Swift/AppKit controls, text fields, split views, and embedded Metal/terminal surfaces together; requires **no TCC permissions**; never steals focus.

### Tier 2: WindowServer Single-Window Capture (`--tier=window`)

- **Used when:** Testing native macOS window framing, shadow/corner masking, `NSVisualEffectView` vibrancy, native `NSMenu` popups, `NSPopover`, or attached `NSWindow` sheets.
- **Mechanism (`ScreenCaptureKit`, macOS 14+):**
  1. Preflight permission with `CGPreflightScreenCaptureAccess()`. Never call capture APIs blindly if preflight is `false` during automated runs unless `--request-tcc` was explicitly passed; instead return `error: screen_recording_denied (use --tier=composite or grant Screen Recording)`.
  2. Query `SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: false)` and match `SCWindow.windowID == CGWindowID(window.windowNumber)`.
  3. Build `SCContentFilter(desktopIndependentWindow: scWindow)` so **only** the target window (and its attached sheets/popovers) is rendered—never surrounding desktop wallpaper or other user windows.
  4. Configure `SCStreamConfiguration`:
     - `width = Int(scWindow.frame.width * scale)`, `height = Int(scWindow.frame.height * scale)`
     - `showsCursor = false`
     - `captureResolution = .best`
     - `ignoreShadowsSingleWindow = true` (unless `--with-shadow` is passed, keeping golden dimensions exact)
  5. Capture a single frame with `SCScreenshotManager.captureImage(contentFilter:configuration:)` and write sRGB PNG.
- **Properties:** Pixel-exact WindowServer output including system titlebars and sheets, isolated from the rest of the desktop.

---

## Determinism and Hermetic Isolation Rules

Screenshots become flaky if host state leaks into the app. Every driven macOS run enforces seven isolation invariants:

1. **Agent Clock (`ui_agent_clock`):** Wall-clock `dt` is ignored unless `--real-time` is passed. Caret blink timers, spinners, and spring animations read the host clock, so between two `coax drive` commands zero animation time passes.
2. **Pinned Wall "Now" (`--still-clock`):** Relative timestamps ("2m ago", clock headers) read a pinned epoch (`2026-11-01T12:00:00Z`) and fixed locale/timezone (`en_US_POSIX`, `UTC`).
3. **Pinned Backing Scale (`2.0`):** Offscreen and composite captures default to `dpr = 2.0` (`sRGB` color space) regardless of whether the host Mac has a 1x external monitor, a 2x Studio Display, or runs headless.
4. **Private Pasteboard:** Under `--drive` or `--shot`, the host binds `NSPasteboard.withUniqueName()` instead of `NSPasteboard.general`, allowing parallel tests to run `Copy` and `Paste` without clobbering the user's clipboard or racing each other.
5. **Modal Panel Interception:** Calling file open/save or alert APIs under `--drive` never calls `[panel runModal]` or `[panel beginSheetModalForWindow:]`. Instead it populates `h->choosing` and emits an accessible status state that `choose PATH` or `cancel` resolves.
6. **Focus & Event Isolation:** Key and mouse actions construct targeted `NSEvent` values sent to `[window sendEvent:]` or invoke `NSAccessibility` / Coax handlers directly. `NSApp.activate` is never called.
7. **Filesystem & Keychain Sandboxing:** Check harnesses launch the binary with an isolated `--settings PATH`, temporary `HOME`, `BROWSER=/usr/bin/true`, and mock credential/account fixtures so developer logins never leak into screenshots.

---

## Verification Workflow: Text First, Pictures Second

Following `apps/openagents-desktop/check.sh` and Recommendation 5 of `docs/research/exact/04-apps-audit-2026-11-10.md`:

1. **Assert operation outputs** (`state status`, `state desk`, `focus ID`).
2. **Diff the element/AX tree** (`goldens/<case>.tree` or `.ax`) line by line after normalizing temporary home paths (`$home` -> `~`). Most layout, missing-button, and label bugs fail here with an immediate text diff.
3. **Compare the screenshot** (`--compare actual.png golden.png [--diff diff.png]`):
   - Dimensions must match exactly.
   - Fails if more than `0.1%` of pixels differ by more than `48 / 255` on any channel.
   - On failure, writes a side-by-side or red-highlighted `actual.diff.png` artifact so an agent or human reviewer can inspect the visual delta immediately.

---

## Security and Safety Boundaries

- **Opt-in activation:** The drive socket listener only starts when `--drive SOCKET` is explicitly passed on the command line. Production launches from Finder/Dock open no socket.
- **Local user-only permissions:** The Unix domain socket is created with `0600` permissions inside a `0700` user directory, unlinked cleanly on `ui_close` and stale-checked on startup. No TCP socket is opened on macOS.
- **Path confinement:** `screenshot OUT.png` and `choose PATH` operate under the launching user's UID and reject empty or directory output paths; atomic temp-file writes prevent partial PNG reads.

---

## Implementation Plan

### Phase 1: Shared macOS Capture & Drive Extraction (`apps/shared/host/apple/`)
- Extract the socket polling + `dispatch_source_t` RunLoop integration from `ui-host.m` / `drive.c` into a reusable `apple/mac-drive.m` helper that both `UiHost` apps and pure AppKit `NSApplication` apps (`coder-desktop`, `openagents-desktop/native`) can attach to an `NSWindow`.
- Add `screenshot --crop ID` and atomic `.tmp` rename to `ui_shot_write`.

### Phase 2: Tier 1 In-Process Composite & Tier 2 `ScreenCaptureKit` Backend
- Implement `ui_mac_capture_composite(NSWindow *, const char *png)` combining `cacheDisplayInRect:toBitmapImageRep:` with readable (`framebufferOnly = NO`) `CAMetalLayer` / `IOSurface` subview blits.
- Implement `ui_mac_capture_window_sck(NSWindow *, const char *png)` gated on `CGPreflightScreenCaptureAccess()` using `SCScreenshotManager` + `SCContentFilter(desktopIndependentWindow:)`.

### Phase 3: Adopt in `coder-desktop` and `openagents-desktop/native`
- Wire `--drive SOCKET`, `--shot OUT.png`, `--do STEPS`, and `--tree OUT.tree` in `apps/coder-desktop/App.swift` (replacing ad-hoc `--preview-state` screenshot glue with full interactive driving across sidebar, Ghostty terminal surfaces, and preview panes).
- Wire the same flags in `apps/openagents-desktop/native/App.swift` and `EnvironmentWindow.swift`, mapping `NSAccessibilityIdentifier` tags on native AppKit controls to `ax`, `ax-press`, and `type`.

### Phase 4: Diff Artifact Output & Golden Check Unification
- Extend `--compare A.png B.png` in `apps/shared/host/` to emit `A.diff.png` on mismatch and add a shared `apps/shared/host/check-lib.sh` used by `openagents-desktop/check.sh`, `coder-desktop`, and `everglade-desktop`.
