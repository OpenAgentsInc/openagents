# Embedded Browser Right-Side Pane in OpenAgents Desktop

**Status:** Specification — Ready for Review & Phased Implementation (2026-11-26)  
**Scope:** `apps/openagents-desktop/` (Coax page, C bridge, macOS / Linux / Windows hosts, build & packaging), `coax/lib/agent/desktop/` (`flags.coax`, `browser-route.coax`), `apps/shared/host/` (Metal & Vulkan compositors), `proto/coder/`, `coax/lib/agent/tools/` (Browser agent tool / MCP attach).  
**Related & Prior Specs:**
- Mirrors the Antigravity-style expandable right-side pane pattern established by `apps/openagents-desktop/page/filepane.coax` (#13760, #13881, #13992) and `apps/openagents-desktop/page/metaversepane.coax` ([../desktop/metaverse-viewer-and-gamebuilder-pane-spec.md](../desktop/metaverse-viewer-and-gamebuilder-pane-spec.md), #14037–#14042)
- Evolves and supersedes the single-tab prototype Web Preview pane (`apps/openagents-desktop/page/preview.coax`, `apps/openagents-desktop/Window+Preview.swift`, #13888)
- Conforms to [../../components/rendering.md](../../components/rendering.md), [../desktop/openagents-v1-spec.md](../desktop/openagents-v1-spec.md), and [../INVARIANTS-CROSS-CUTTING.md](../INVARIANTS-CROSS-CUTTING.md)

---

## Table of Contents

1. [Product Vision, Goals & Hard Invariants](#1-product-vision-goals--hard-invariants)
2. [How the File Viewer, 3D Viewer & Web Preview Work Today](#2-how-the-file-viewer-3d-viewer--web-preview-work-today)
3. [Browser Engine Options & Trade-Off Analysis](#3-browser-engine-options--trade-off-analysis)
4. [Recommended Architecture: Pluggable `oa_browser_host` Hybrid](#4-recommended-architecture-pluggable-oa_browser_host-hybrid)
5. [UX, Chrome & Responsive Shell Integration](#5-ux-chrome--responsive-shell-integration)
6. [Auto-Opening, Routing Policy & Profile Isolation](#6-auto-opening-routing-policy--profile-isolation)
7. [Compositing, Input & Cross-Platform Host Architecture](#7-compositing-input--cross-platform-host-architecture)
8. [Bidirectional Chat ↔ Browser & Agent Automation](#8-bidirectional-chat--browser--agent-automation)
9. [Coax Page State, C-ABI Bridge, Flags & Proto Contracts](#9-coax-page-state-c-abi-bridge-flags--proto-contracts)
10. [File-by-File Implementation Map](#10-file-by-file-implementation-map)
11. [Phased Delivery Plan, Verification Matrix & Risk Register](#11-phased-delivery-plan-verification-matrix--risk-register)

---

## 1. Product Vision, Goals & Hard Invariants

### 1.1 Why Our Own Browser Pane Belongs Next to the File Viewer and 3D Viewer

OpenAgents Desktop is built around a unified **3-region responsive shell** (`apps/shared/app/view/shell.coax` + `apps/openagents-desktop/page/layout/layout.coax`): a collapsible left chat sidebar, a centered conversation column, and an **expandable right-side workspace pane**. Two rich surfaces already use this exact shell pattern:

1. **File Viewer & Editor (`page/filepane.coax`):** Multi-tab strip with colored language badges, `+` quick-open, **`⛶` Expand/Restore** (full main area beside the sidebar), **`◫` Hide** (collapsing the pane while preserving open tabs and scroll), Row 2 interactive breadcrumbs + inline toolbar, and a collapsible File Explorer sidebar.
2. **Metaverse 3D Viewer & GameBuilder (`page/metaversepane.coax`):** Identical two-row Coax chrome, multi-tab strip with RGB badges, `⛶` Expand/Restore, `◫` Hide, collapsible Studio/Inspector drawer, live bidirectional Chat ↔ 3D round-trip, and native Metal/Vulkan viewport hosting with both `cd_over_rects` overlay hole-cutting and zero-copy `composition-v2` external texture compositing.

The missing third pillar of everyday software and agentic work is the **Web**: inspecting local dev servers (`localhost:3000`, `:5173`), reading docs/PRs/issues, authenticating with OAuth providers, visually verifying UI changes, letting an agent browse/click/fill/test a web app while the human watches side-by-side, and expanding into a full widescreen browser without leaving OpenAgents Desktop.

Today, `page/preview.coax` (`Window+Preview.swift`, #13888) provides a lightweight single-tab `WKWebView` preview on macOS only, with no multi-tab strip, no Expand/Restore, no Hide-preserving-tabs, no inline omnibox, no back/forward history, no Console/Network/DOM drawer, no cross-platform Linux/Windows support, no CDP/agent automation protocol, and no overlay hole-cutting.

This specification defines **`page/browserpane.coax`**—our own multi-tab, expandable, agent-native browser pane—and rigorously evaluates the web-engine options behind it.

### 1.2 Goals

1. **Identical ergonomics to File Viewer & 3D Viewer:** Same two-row Coax chrome (`38pt` tab strip + `30pt` navigation/omnibox bar), up to 12 open tabs with colored origin/status badges, `+` new tab, **`⛶` Expand/Restore** to full main area beside the left sidebar, **`◫` Hide** preserving live tabs/scroll/sessions, collapsible right DevTools/Agent drawer (`◧`), and smooth `app_presence` spring slide.
2. **Engine-agnostic Coax shell:** All browser chrome, tab state, URL normalization/safety, routing policy, profile policy, find-in-page state, viewport presets, and agent permission UX live in Coax behind a narrow C-ABI (`oa_browser_host.h`), decoupling UI & policy from the underlying web rendering engine.
3. **Zero installer bloat for `--minimal` & immediate shippability:** Phase 1 ships with near-zero binary overhead using OS-native WebViews (`WKWebView` on macOS, `WebKitGTK 6` on Linux, `WebView2` on Windows); Phase 3 adds an optional, signed, on-demand downloadable Chromium/CEF Out-of-Process helper (`oa-browser-helper`) for full CDP, WebGPU parity, and zero-copy GPU texture compositing.
4. **First-class Chat ↔ Browser round-trip:** Pick any DOM element (`⌖`) to attach its CSS selector, accessible name, bounding box, computed styles, and screenshot to the composer; attach console errors or failed network requests in one click; let agents inspect, click, type, and test inside the live open tab over an owner-only attach socket.
5. **Human Supremacy & Security Confinement:** The human can pause or take over an agent-driven tab instantaneously; untrusted web content runs out-of-process with strict scheme/navigation allowlists, separated cookie/storage profiles (`Personal`, `Workspace`, `Agent Ephemeral`), and explicit human confirmation before agent credential entry, payments, or file uploads.
6. **Zero regression to Inkling-only v1.0 release:** Strictly gated behind `DC_FLAG_BROWSER` (default `false`) and stubbed out of `--minimal` builds (`page/v1-stubs.coax`).

### 1.3 Non-Goals

- Replacing Chrome/Safari/Firefox as a full consumer browser with a Chrome Web Store extension marketplace or browser-sync cloud service in v1.
- Writing a clean-room HTML5/CSS3/ES2025 JIT web browser engine in Coax for the arbitrary public web.
- Embedding Electron or Node.js inside OpenAgents Desktop.
- Allowing autonomous agents to silently read the user's `Personal` browser cookies/passwords or submit sensitive forms without explicit profile binding and approval.

### 1.4 Hard Architectural Invariants

| # | Invariant | Source of Truth | Enforcement |
|---|---|---|---|
| **I-1** | **Coax Owns Chrome, State & Policy; Host Owns Engine Effects Only:** Coax owns layout, tabs, URL normalization/filtering, routing policy, omnibox editing, drawer UI, and agent-lease rules. Swift/ObjC/C/C++ only wraps the web engine lifecycle, texture/view presentation, JS/CDP transport, and OS dialogs. | `apps/INVARIANTS.md` | Firm-reason headers + `coax check` |
| **I-2** | **One Engine Abstraction (`oa_browser_host.h`):** `page/browserpane.coax` NEVER calls `WKWebView`, `WebKitGTK`, `WebView2`, or `CEF` APIs directly. Every backend implements the exact same C-ABI verb/event surface so engines can be swapped per platform or user setting without rewriting UI or agent tools. | This spec §4, §9 | Compile checks across macOS/Linux/Windows |
| **I-3** | **Minimal v1.0 Release Isolation:** `DC_FLAG_BROWSER` defaults `false` in `coax/lib/agent/desktop/flags.coax`. `--minimal` builds (`app-v1.coax` + `page/v1-stubs.coax`) MUST omit all browser pane code, WebKit/CEF links, and helper binaries. | `docs/desktop/openagents-v1-spec.md` | `build.sh --minimal` smoke check |
| **I-4** | **Strict Scheme & Process Confinement:** Web content runs out-of-process (`WKWebView` WebContent process, WebView2/WebKitGTK child process, or `oa-browser-helper`). Only `http://`, `https://`, and explicitly vetted internal `openagents://` routes may load; `file://`, `javascript:`, `data:` top-level navigations, and local-network SSRF from untrusted origins are enforced by `#authority` policy. | `browser-route.coax` (`#authority`) | `#test` & mutation checks |
| **I-5** | **Human Supremacy & Target Leases:** An agent driving a browser tab NEVER locks out human pointer/keyboard takeover. Clicking or typing inside an agent-controlled tab immediately pauses agent control. Concurrent agent runs targeting the same tab/session obey `#authority` lease conflict checks. | `agents-catalog.coax` pattern | `#authority` unit tests |
| **I-6** | **Bounded Memory & Zero-Hz Background Throttling:** Hidden panes (`◫`), occluded tabs, and background windows throttle rendering and media (`0 Hz` compositing for hidden views; background tabs muted unless explicitly unmuted; LRU tab hibernation when open tabs exceed RAM budget). | `components/rendering.md` | Engine budget telemetry + tests |
| **I-7** | **Proto-First Contracts:** Every browser route, DOM selection, tab session snapshot, and agent automation command/response gets a versioned JSON Schema under `proto/coder/` and a `proto/registry.json` row. | `docs/INVARIANTS-CROSS-CUTTING.md` | `proto` registry validation |

---

## 2. How the File Viewer, 3D Viewer & Web Preview Work Today

Understanding the exact machinery already shipped in `apps/openagents-desktop/` ensures the Browser pane slots into the existing shell with zero redundant infrastructure.

### 2.1 Right-Side Pane Shell & Expansion Mechanics (`page/layout/layout.coax`)

- **Priority Stack (`cp_side_pane`, lines 28–49):** Resolves which right-side surface renders inside `(px, pw, h)`:
  `forge` → `side_chat` → `preview` → `term` → `sim` → `mv` (Metaverse/3D) → `fp` (File Viewer) → `review` → `todos`/`pane` (sub-agent transcript).
- **Split Geometry & Spring Motion (`cp_layout`, lines 66–84):**
  - Default split width `CP_PANE = 520pt`, min `CP_PANE_LEAST = 360pt`, max `CP_PANE_MOST = 1080pt` (dynamically widened on ultrawide displays via `cp_pane_spec_for` while preserving `CP_PANE_MAIN = 456pt` for chat).
  - Keyed `app_presence` spring slide (`coder.shell.pane`) animates open/close cleanly.
  - Below `816pt` total window width, `app_shell_layout` switches to `APP_PANE_FULL` (full window width, Row 1 pushed below `cp_bar(s) = 38pt` to clear macOS traffic lights).
- **Expand / Restore (`⛶`, line 145):**
  ```coax
  if (cp_fp_shown(s) && s[0].fp[0].expanded) || (cp_mv_shown(s) && s[0].mv[0].expanded) {
    s[0].pane_x = left; s[0].pane_w = window_w - left; s[0].col_w = 0;
    cp_side_pane(s, f, left, window_w - left, h);
    ...
  }
  ```
  Adding `(cp_br_shown(s) && s[0].br[0].expanded)` gives the Browser pane the exact same one-click widescreen expansion beside the left sidebar (and full edge-to-edge canvas when the sidebar is collapsed via `⌘B`).

### 2.2 Two-Row Chrome, RGB Exemption & Native Viewport Hosting

| Mechanism | File Viewer (`filepane.coax`) | 3D Viewer (`metaversepane.coax` + `Window+Metaverse.swift`) | Prototype Preview (`preview.coax` + `Window+Preview.swift`) |
|---|---|---|---|
| **Row 1 (38pt)** | Multi-tab pills (up to 16), colored lang badges, `+`, `⛶`, `◫` | Multi-tab pills (up to 12), colored RGB badges, `+`, `◧`, `⛶`, `◫` | Single header bar (no tabs, no `⛶`, no `◫`) |
| **Row 2 (30pt)** | Interactive path breadcrumbs + `↻`, `≡`, `⋮` | Breadcrumbs + mode segmented control + rev pill + `📷` + `⋮` | URL chip (opens dialog) + `↻` + viewport preset + DOM picker + `📷` + port chips |
| **Monochrome Skin Exemption** | Marks `fp_color_u0..u1`, `fp_color_o0..o1` (`host-api.coax:66-130`) so syntax/badges stay full color under Noir skin | Same RGB exemption for tab badges and status pills | None |
| **Body Rendering** | Pure Coax GPU (`Sheet` / `UiEditor` / `MdDoc`) — no native subview | Native `MetaverseViewportView` (`CAMetalLayer`) **OR** zero-copy `MTLTexture` into `composition-v2` (`cd_mv_set_ext_texture`) | Native `WKWebView` subview mounted on `canvas` over `cd_preview_rect` |
| **Overlays Above Native View** | Pure Coax (naturally z-ordered) | `cd_over_rects` (`page/overlay.coax`) → `CAShapeLayer` even-odd cutout mask + `hitTest` pass-through (or seamless when `useCompositionV2`) | **Gap:** hides whole `WKWebView` whenever `rect[4] != 0` (any overlay open), causing a flash |
| **Routing Policy** | Extension + size heuristics in `Window+Files.swift` | `#authority` `coax/lib/agent/desktop/metaverse-route.coax` (`oa_dc_mv_route`) | `cp_preview_normalize_url` (http/https only) |
| **Chat Round-Trip** | Line-range jump, Copy with Path:Line, File Explorer | `world.turn.v1` selection attachment, `TK_WORLD` cards, `gamebuilder mcp --attach` Unix socket | `oaDomPick` JS handler → dialog → Markdown block in composer; `takeSnapshot` PNG |
| **Cross-Platform** | macOS + Linux + Windows (100% Coax) | macOS Metal live; Linux/Windows Vulkan designed (`metaverse/gpu/`) | macOS only (`WKWebView`); unimplemented in `linux/` and `windows/` |

**Key Takeaway:** Every piece of shell geometry, tab chrome, `#authority` routing, overlay hole-cutting, external-texture compositing, and `--minimal` compile-time stubbing is **already proven in production** inside `apps/openagents-desktop/`. The Browser pane does not need to invent a new UI framework—it reuses the exact architecture of `filepane.coax` and `metaversepane.coax`.

---

## 3. Browser Engine Options & Trade-Off Analysis

The central architectural decision is **which web rendering & JavaScript engine powers the Browser pane's viewport**. There are five realistic options.

### Option A: OS-Native WebViews (`WKWebView` / `WebKitGTK 6` / `WebView2`)

Wrap each OS's built-in system web engine behind a shared C-ABI:
- **macOS:** `WKWebView` (`WebKit.framework`, already linked in `Window+Preview.swift`).
- **Linux:** `WebKitGTK 6.0` (`libwebkitgtk-6.0.so`, dynamically loaded via `dlopen` so builds never hard-fail on headless servers).
- **Windows:** `Microsoft Edge WebView2` (`WebView2Loader` COM API, pre-installed on Windows 10/11).

| Strengths | Weaknesses |
|---|---|
| **~0 MB binary overhead** on macOS & Windows; instant cold start (<50 ms). | **Three divergent engines & APIs** (WebKit on macOS/Linux vs Chromium/Edge on Windows); subtle layout, CSS, and WebGPU differences. |
| **OS-maintained security updates** — Apple, distro maintainers, and Microsoft ship zero-day patches without forcing an OpenAgents release. | **No unified Chrome DevTools Protocol (CDP).** WebView2 exposes CDP, but `WKWebView` and `WebKitGTK` do not expose public CDP; agent automation must rely on injected JS bridges (`WKUserContentController` / `WebKitUserContentManager`), which cannot intercept cross-origin iframes, raw HTTP headers, or low-level input events cleanly. |
| Native macOS trackpad inertia, system fonts, Keychain/Passkeys, hardware video decode, low idle RAM (~40–80 MB). | **No zero-copy GPU texture sharing in `WKWebView`** (must mount as child `NSView` + `cd_over_rects` hole-cutting; frosted Coax menus cannot blur live web pixels underneath). |
| Zero C++ build-chain bloat; ships fastest (weeks, not quarters). | WebKitGTK Wayland subsurface compositing and WebGPU support lag behind Chromium on Linux. |

---

### Option B: Chromium Embedded Framework (`CEF` / `libcef`)

Embed Chromium's multi-process Blink + V8 engine via CEF, with two distribution sub-variants:
- **B1 — Bundled inside OpenAgents Desktop:** Ship `Chromium Embedded Framework.framework` / `libcef.so` / `libcef.dll` directly inside the app package.
- **B2 — On-Demand Hydrated Sidecar Engine (`oa-browser-helper`):** Keep OpenAgents Desktop lean (~20 MB); when a user enables Pro/Agent Browser mode, download and verify (Ed25519 + SHA-256, identical to `openagents.desktop.update.v1` and local Inkling model hydration) a signed `oa-browser-helper` bundle into `~/.openagents/engines/browser/`.

| Strengths | Weaknesses |
|---|---|
| **100% modern web compatibility:** Every SaaS app, Google Workspace, Figma, Stripe, OAuth/WebAuthn, WebGPU (`openagents.com` & `gamebuilder/web`), WASM, WebRTC, Service Workers. | **Heavyweight footprint:** ~150–250 MB compressed download (~400–600 MB on disk) and ~150–300 MB baseline RAM across browser/GPU/renderer processes. (B2 eliminates installer bloat for users who don't enable it.) |
| **Native Chrome DevTools Protocol (CDP) & WebDriver BiDi** identically across macOS, Linux, and Windows — gold standard for agentic browsing, full Accessibility Tree snapshots, network interception, console streams, trace capture, and trusted synthetic input (`Input.dispatchMouseEvent`). | **Security-patch cadence:** Must track Chromium Extended Stable / Stable releases and ship signed `oa-browser-helper` updates every 2–4 weeks for V8/Blink CVEs. |
| **Off-Screen Rendering (OSR) with Zero-Copy Shared GPU Textures (`CefRenderHandler::OnAcceleratedPaint`):** Hands an `IOSurface` (macOS Metal), `DMA-BUF` (Linux Vulkan), or `D3D11/Vulkan` shared handle (Windows) directly into OpenAgents' GPU compositor (`composition-v2`)! Eliminates native-subview z-order bugs and lets frosted Coax popovers blur live web pages seamlessly. | **Packaging & build complexity:** Requires prebuilt CEF artifacts per target (`macos-arm64`, `macos-x64`, `linux-x64`, `linux-arm64`, `windows-x64`, `windows-arm64`) and a dedicated multi-process helper executable (`oa-browser-helper` + macOS `.app` helper bundles). |
| **Full multi-process Site Isolation**, per-profile `CefRequestContext` cookie/storage jars, and built-in DevTools frontend (inspectable in our drawer or pop-out). | macOS native scroll physics / Look Up / system Passkeys feel slightly less "Apple-native" than `WKWebView` unless tuned carefully. |

---

### Option C: Alternative Open Engines (`Servo` / `Ladybird`)

Embed an independent non-Blink/non-WebKit engine:
- **Servo (Rust):** Parallel CSS/layout (`Stylo`), `WebRender` GPU compositor, `SpiderMonkey` JS engine. Actively revived by Igalia with an embeddable C/Rust API and WebRender texture output.
- **Ladybird (`LibWeb`/`LibJS`, C++23):** Clean-room browser engine with strict multi-process isolation (`WebContent`, `RequestServer`, `ImageDecoder`).

| Strengths | Weaknesses |
|---|---|
| True independence from Google (Blink) and Apple (WebKit); zero ad-tech telemetry baggage. | **Not daily-driver web-compatible yet:** Complex modern SPAs, OAuth/SSO flows, Cloudflare/bot checks, WebRTC, and full WebGPU regularly break or stall. |
| Servo's WebRender architecture aligns well with GPU-composited offscreen textures; smaller footprint than Chromium (~50–90 MB). | **Immature security & DevTools story:** Neither matches Chromium/WebKit's decade of multi-process fuzzing for untrusted public websites, nor provides complete CDP/DevTools parity for coding/browsing agents. |
| Strong strategic alignment with OpenAgents' sovereignty ethos long term. | High upstream churn (API breaks frequently; Ladybird targets first alpha in 2026). |

**Verdict on Option C:** Do **not** bet v1 production browsing on Servo or Ladybird alone, **but** design `oa_browser_host.h` so either can plug in later as an experimental engine backend.

---

### Option D: Clean-Room Coax Browser Engine

Write HTML5, CSS3, layout, DOM, and JS engine natively in Coax on top of `components/backend/gpu.coax`.

| Strengths | Weaknesses |
|---|---|
| 100% Coax, tiny binary, zero external dependencies, direct Metal/Vulkan rendering. | **Unviable for the public web:** Full HTML5 + CSS (Grid/Flex/Subgrid/Cascade/Fonts/BiDi/Shaping) + JIT ES2025 JS + Web APIs + TLS/HTTP3 + media codecs + sandboxed security is a **10M+ LOC, multi-decade effort**. |

**Verdict on Option D:** Reject for general web browsing. **Adopt selectively** for trusted OpenAgents-native documents (`openagents://` internal pages, Markdown/docs, and OpenAgents 3D/GameBuilder realms routed to `filepane.coax` or `metaversepane.coax`).

---

### Option E: Pluggable `oa_browser_host` Hybrid (OS-Native WebView First + On-Demand CEF OSR Helper Second) — **RECOMMENDED**

Combine the strengths of Options A, B2, and D behind **one Coax-owned Browser Pane (`page/browserpane.coax`)** and **one narrow engine ABI (`oa_browser_host.h`)**:

| Dimension | Option A (OS WebViews) | Option B1 (Bundled CEF) | Option B2 (On-Demand CEF) | Option C (Servo/Ladybird) | **Option E (Recommended Hybrid)** |
|---|---|---|---|---|---|
| **Time to ship multi-tab browser pane** | Fast (2–3 wks) | Slow (8–12 wks) | Medium-Slow (8–10 wks) | High risk | **Fast (Phase 1 in 2–3 wks, CEF OSR in Phase 3)** |
| **Desktop installer bloat** | ~0 MB | +200 MB | ~0 MB base (+200 MB opt-in) | +60–90 MB | **~0 MB base (+200 MB opt-in Pro/Agent engine)** |
| **Public web compatibility** | High (macOS/Win), Medium (Linux) | Highest | Highest | Low–Medium | **High immediately → Highest with CEF** |
| **Cross-platform parity (macOS/Linux/Win)** | Divergent engines | Identical | Identical | Partial | **Unified UX Day 1 → Identical engine with CEF** |
| **Agent automation (CDP / A11y / Network)** | JS shim only (limited on WebKit) | Full native CDP | Full native CDP | Minimal | **JS bridge Day 1 → Full native CDP with CEF** |
| **Coax GPU overlay compositing (`composition-v2`)** | Child view + `cd_over_rects` cutouts | Zero-copy GPU texture | Zero-copy GPU texture | WebRender texture | **`cd_over_rects` Day 1 → Zero-copy GPU texture with CEF** |
| **Security patch burden** | OS-managed | High (every 2–4 wks) | Medium (separate signed engine channel) | High | **OS-managed by default; independent signed channel for CEF** |
| **Future Servo/Ladybird / Coax-native routing** | N/A | Hard | Hard | Native | **First-class via `oa_browser_host.h`** |

---

## 4. Recommended Architecture: Pluggable `oa_browser_host` Hybrid

```
┌───────────────────────────────────────────────────────────────────────────────────┐
│ OpenAgents Desktop (Coax Page: page/browserpane.coax + #authority browser-route) │
│   • Row 1 (38pt): Multi-tab strip (12 tabs), RGB badges, +, ◧ Drawer, ⛶, ◫       │
│   • Row 2 (30pt): Back/Fwd/Reload, Inline Omnibox (UiEditor), TLS/Profile chip,  │
│                   Viewport preset, DOM Picker ⌖, Shot 📷, Agent Pill, ⋮ Menu      │
│   • Right Drawer: Console | Network | DOM/A11y | Agent Log | Bookmarks           │
│   • #authority Policy: URL validation, profile isolation, agent lease & approval  │
└────────────────────────────────────────┬──────────────────────────────────────────┘
                                         │  Narrow C-ABI (oa_browser_host.h)
                                         │  Commands: open/nav/reload/back/fwd/resize/
                                         │            input/eval/cdp/shot/profile/close
                                         │  Events:   url/title/load/favicon/console/
                                         │            net/dom_pick/frame_tex/crashed
                 ┌───────────────────────┼───────────────────────┐
                 ▼                       ▼                       ▼
┌────────────────────────────┐ ┌─────────────────────────┐ ┌────────────────────────┐
│ Backend 1: OS-Native       │ │ Backend 2: Pro / Agent  │ │ Backend 3: Native Coax │
│ (Default, ~0 MB overhead)  │ │ (On-Demand Signed Pack) │ │ & Future Engines       │
│                            │ │                         │ │                        │
│ • macOS: WKWebView         │ │ • oa-browser-helper     │ │ • openagents:// views  │
│ • Linux: WebKitGTK 6       │ │   (CEF Out-of-Process)  │ │ • 3D/Realm -> mvpane   │
│ • Windows: Edge WebView2   │ │ • OSR Shared Texture    │ │ • Markdown -> filepane │
│ • Unified JS bridge        │ │   (IOSurface/DMA-BUF/Vk)│ │ • Experimental Servo / │
│   (DOM pick, console, a11y)│ │ • Native CDP + BiDi     │ │   Ladybird plug-in     │
│ • Child view + over_rects  │ │ • Full Site Isolation   │ │                        │
└────────────────────────────┘ └─────────────────────────┘ └────────────────────────┘
```

### Why This Wins

1. **Immediate Value with Zero Regressions:** Upgrading `page/preview.coax` → `page/browserpane.coax` on `WKWebView` (macOS), `WebKitGTK 6` (Linux), and `WebView2` (Windows) gives users a sleek, expandable multi-tab browser pane right beside chat immediately—without adding 200 MB to the desktop binary or blocking on Chromium build infrastructure.
2. **Independent Engine Upgrade Channel:** Shipping `oa-browser-helper` (CEF OSR) as a signed, on-demand hydrated artifact (identical to how OpenAgents handles local Inkling weights and Ed25519-verified updates) keeps the desktop installer tiny, lets us push Chromium security updates independently of desktop releases, and unlocks full CDP + zero-copy GPU texture compositing for power users and autonomous browser agents.
3. **Zero Throwaway Work:** Because Coax owns 100% of the chrome, tab state, routing, drawer UI, and agent protocol from Day 1, switching a user or platform from Backend 1 (OS-Native) to Backend 2 (CEF OSR) changes **zero lines** of `page/browserpane.coax` or `browser-route.coax`.

---

## 5. UX, Chrome & Responsive Shell Integration

### 5.1 Responsive Shell States (Mirroring `filepane.coax` & `metaversepane.coax`)

#### State A: Split Mode (Default — `520pt`, draggable `360..1080pt`+)
```
┌──────────────┬───────────────────────────────┬────────────────────────────────────────────────────┐
│ Left Sidebar │ Conversation Column (≥456pt)  │ [● localhost:5173 ×][● Docs ×] [+]    [◧][⛶][◫]   │ ← Row 1 (38pt)
│ (272pt,      │                               ├────────────────────────────────────────────────────┤
│ collapsible) │ User: "Fix the header align   │ [←][→][↻] [🔒 localhost:5173/app     ] [📱][⌖][📷][⋮]│ ← Row 2 (30pt)
│              │ on mobile and verify it"      ├────────────────────────────────────────────────────┤
│  Chats       │                               │ [🤖 Agent driving · Pause / Take Over]             │ ← Row 3 (26pt, contextual)
│  Pinned      │ ┌───────────────────────────┐ ├────────────────────────────────────────────────────┤
│  Recent      │ │ Browser · Clicked #nav    │ │                                                    │
│              │ │ [Screenshot] [Console: 0] │ │               Live Web Viewport                    │
│              │ │ [Open Tab] [Copy Selector]│ │     (WKWebView / WebKitGTK / WebView2 / CEF)       │
│              │ └───────────────────────────┘ │                                                    │
│              │                               │                                                    │
│              │ ┌───────────────────────────┐ │                                                    │
│              │ │ + @ Ask or steer…     [↑] │ │ [Profile: Workspace]          [390×844 · 0 errors] │
│              │ └───────────────────────────┘ │                                                    │
└──────────────┴───────────────────────────────┴────────────────────────────────────────────────────┘
                                               ▲ Draggable split divider (cp_edge == 2, 4pt hit)
```

#### State B: Expanded Mode (`⛶` — `expanded == true`, Full Main Area Beside Sidebar)
Clicking **`⛶`** (or pressing `⇧⌘E`) sets `s[0].br[0].expanded = true`. In `page/layout/layout.coax:145`, the browser pane claims `pane_x = left`, `pane_w = window_w - left`, `col_w = 0`. Pressing `⌘B` collapses the left sidebar for an edge-to-edge full browser experience; clicking `⛶` again (or pressing `Esc` when focus is in chrome) snaps immediately back to split chat+browser.

```
┌──────────────┬────────────────────────────────────────────────────────────────────────────────────┐
│ Left Sidebar │ [● localhost:5173 ×][● GitHub PR #14095 ×][● Stripe Docs ×] [+]       [◧][⛶][◫]  │ ← Row 1 (38pt)
│ (272pt,      ├────────────────────────────────────────────────────────────────────────────────────┤
│ ⌘B toggle)   │ [←][→][↻] [🔒 https://github.com/OpenAgentsInc/openagents/pull/14095 ] [🖥][⌖][📷][⋮]│ ← Row 2 (30pt)
│              ├──────────────────────────────────────────────────────────────┬─────────────────────┤
│              │                                                              │ [Console][Net][DOM] │ ← Collapsible
│              │                                                              │ [Agent][Bookmarks]  │   Drawer (300pt)
│              │                   Widescreen Web Viewport                    │                     │
│              │            (120 Hz scroll · Full DevTools drawer)            │ ⚠ 1 warning         │
│              │                                                              │ ✖ TypeError: ...    │
│              │                                                              │   [Attach to Chat]  │
│              │                                                              │                     │
│              │                                                              │ GET /api/v1 200 18ms│
│              │                                                              │ POST /auth  401 42ms│
└──────────────┴──────────────────────────────────────────────────────────────┴─────────────────────┘
```

#### State C: Narrow Window Mode (`APP_PANE_FULL`)
Below `816pt`, the browser pane occupies `[0, window_w]` with Row 1 pushed below `cp_bar(s)` (`38pt`) to clear macOS traffic lights. Clicking **`◫` Hide** returns to chat while keeping every open tab, web process, and session intact.

### 5.2 Chrome Specification

- **Row 1 (`CP_BR_BAR1_H = 38.0pt`, flush with titlebar):**
  - Up to `CP_BR_TABS = 12` tab pills (shrinking with `app_fit`, min `64pt`, overflow chevron menu beyond fit).
  - Each pill shows: 14×14 favicon or colored RGB origin/profile badge (exempt from monochrome Noir filter via `fp_color_u0..u1`), page title / hostname, loading spinner or audio-playing indicator (click to mute), agent-driving dot when leased to an agent, and `×` close button.
  - Trailing buttons (identical order to `metaversepane.coax`): `+` New Tab (`⌘T`), `◧` Drawer toggle (`⌥⌘I`), `⛶` Expand/Restore (`⇧⌘E`), `◫` Hide-preserving-tabs (`⌥⌘B`).
- **Row 2 (`CP_BR_BAR2_H = 30.0pt`, Navigation & Omnibox):**
  - Left: `←` Back (`⌘[`), `→` Forward (`⌘]`), `↻`/`✕` Reload/Stop (`⌘R`).
  - Center: **Inline Omnibox (`UiEditor`, `CP_KB_BROWSER_URL`)** — focuses on `⌘L`, shows clean origin + path when unfocused, full editable URL or search query when focused, with a leading security/profile chip (`🔒` HTTPS, `⚡` localhost/dev port, `🧪` Agent Ephemeral, `⚠️` HTTP/invalid TLS).
  - Right:
    - Detected local dev-server port dropdown chip (auto-populated from `termpane.coax` / process listener probe, carrying forward `preview.coax`'s best feature),
    - Viewport preset cycle (`Desktop` / `Tablet 768×1024` / `Mobile 390×844`),
    - `⌖` **DOM Element Picker** (hover highlight + click to attach element to chat composer),
    - `📷` **Screenshot to Chat** (attaches viewport PNG directly to composer),
    - `⋮` **More menu** (Find in Page `⌘F`, Zoom `- / 100% / +`, Switch Profile, Open in System Browser, Clear Site Data, Switch Engine `System WebView / Chromium Pro`, View Source in File Pane).
- **Row 3 (`CP_BR_BAR3_H = 26.0pt`, Contextual — only shown when active):**
  - **Agent Control Banner:** Amber/blue accent bar `"Agent (Coder) is interacting with this tab · [Pause] [Take Over]"` + subtle glowing viewport border.
  - **Find-in-Page Bar (`⌘F`):** Query editor, `N / M` match count, `↑` / `↓`, `×`.
  - **Approval Banner:** Inline non-blocking confirmation when an agent requests a gated action (e.g., `"Agent wants to submit form on checkout.stripe.com · [Allow Once] [Deny]"`).
- **Collapsible Right Drawer (`CP_BR_DRAWER_W = 300.0pt`, toggled via `◧`):**
  - **Console:** Live `log`/`warn`/`error`/unhandled-rejection stream with one-click **"Attach errors to chat"**.
  - **Network:** Compact method/status/URL/duration table with failed-request filter and **"Copy as cURL / Attach to chat"**.
  - **DOM / A11y:** Selected node summary, attributes, computed box model, and accessibility role/name tree.
  - **Agent Log:** Step-by-step audit trail of every agent browser command (`goto`, `click`, `type`, `screenshot`) with timestamps and rollback/re-focus.
  - **Bookmarks & History:** Pinned workspace URLs (e.g., project's `localhost` ports, staging URL, GitHub repo) and recent tab history.

---

## 6. Auto-Opening, Routing Policy & Profile Isolation

### 6.1 Canonical `#authority` Router (`coax/lib/agent/desktop/browser-route.coax`)

Following `coax/lib/agent/desktop/metaverse-route.coax`, all URL/link routing decisions are made by a pure `#authority` Coax module tested against mutation and fuzz cases:

| Trigger / Input | Target Pane | Behavior |
|---|---|---|
| `http://localhost:*`, `http://127.0.0.1:*`, `*.local` clicked in chat/terminal or detected dev server | **Browser Pane** (`Workspace` profile) | Auto-opens or focuses existing tab for that origin. |
| `OA-SHOW-URL: <url>` emitted by an agent tool | **Browser Pane** | Opens/focuses tab inside pane and renders inline preview card in transcript. |
| `http://` / `https://` link clicked in chat transcript (when `DC_FLAG_BROWSER` on and "Open links in Browser Pane" enabled) | **Browser Pane** (`Personal` or `Workspace`) | Opens in new tab (or `⌘`-click opens in system default browser). |
| `openagents.com/realms/...` or `gamebuilder.org/g/...` | **Metaverse Pane** (if `DC_FLAG_METAVERSE` on) else **Browser Pane** | Delegates to `metaverse-route.coax` first so 3D worlds open natively. |
| `openagents://browser/open?url=...&profile=...` | **Browser Pane** | Deep-link handler via `Activation.swift` / Linux/Windows single-instance socket. |
| Local `.html` / `.htm` file opened from File Explorer | **File Viewer** by default; `"Preview in Browser"` button in `⋮` opens via loopback static server (`http://127.0.0.1:<ephemeral>/...`), **never** raw `file://`. | Prevents `file://` cross-directory exfiltration. |
| `mailto:`, `tel:`, `ssh:`, `vscode:`, `cursor:`, unknown custom OS schemes | **External OS Handoff** (after confirmation for non-standard schemes) | Never loaded inside webview. |
| `javascript:`, `vbscript:`, `data:`, `blob:`, `file://`, control chars, >4096 B | **Rejected (`BR_PANE_REJECT`)** | Blocked before reaching host webview. |

### 6.2 Three Isolated Browser Profiles (Cookie & Storage Jars)

Every tab belongs to one of three explicit storage partitions (`WKWebsiteDataStore` / `WebKitNetworkSession` / `WebView2 CoreWebView2Environment` / `CefRequestContext`):

1. **`Workspace` (Default for dev servers & project links):** Persistent per-repo/folder storage under `~/.openagents/browser/profiles/workspace-<repo-hash>/`. Keeps local dev logins intact across restarts without bleeding cookies between projects.
2. **`Personal` (Opt-in for docs, GitHub, SaaS):** Persistent user profile under `~/.openagents/browser/profiles/personal/`. **Agents NEVER get automatic read/write access** to `Personal` tabs unless the user explicitly clicks *"Grant agent access to this tab"* for the current run.
3. **`Agent Ephemeral` (Default for autonomous agent browsing):** Non-persistent in-memory storage (`WKWebsiteDataStore.nonPersistent()` / CEF incognito `CefRequestContext`). Automatically wiped when the tab or run closes; zero access to personal cookies or saved passwords.

### 6.3 Network & Localhost Guardrails

- **Private-Network Access Guard:** Public remote origins (`https://evil.example`) cannot navigate or `fetch` `localhost`/`127.0.0.1`/RFC1918 targets via agent-injected scripts without explicit opt-in.
- **Download Quarantine:** Downloads write to `~/Downloads` (or workspace `.openagents/downloads/` for agent runs) with filename sanitization, size caps, and zero auto-execution.
- **Permission Prompts:** Camera, microphone, geolocation, clipboard-read, and notifications default to **Deny** in `Agent Ephemeral` and prompt via Coax `AppDialog` in `Workspace`/`Personal`.

---

## 7. Compositing, Input & Cross-Platform Host Architecture

### 7.1 Two Viewport Compositing Paths (Matching `MetaverseViewportView.swift`)

#### Path 1 — Native Child View + `cd_over_rects` Hole-Cutting (Phase 1: OS-Native WebViews)
- **macOS (`WKWebView`):** Mounted as a subview of `canvas` positioned from `cd_browser_rect(page, &rect)` (logical top-left → AppKit bottom-left). Unlike prototype `Window+Preview.swift` (which hides the entire `WKWebView` whenever *any* overlay opens), we apply the exact `CAShapeLayer` even-odd cutout mask + `hitTest` pass-through pattern proven in `PleScenePanelView` / `MetaverseViewportView.swift` (`pageOverRects(in:)` from `coder_page_over_rects`). Coax menus, dialogs, command palette, and toasts float cleanly over the live page with zero flashing.
- **Linux (`WebKitGTK 6`):** Embedded via Wayland subsurface (`wl_subsurface`) synchronized to the Vulkan window host (`apps/shared/host/linux/ui-host.c`), clipped/hidden beneath active Coax overlays using `cd_over_rects`, with input region cutouts via `wl_region`.
- **Windows (`WebView2`):** Child `HWND` hosted inside `apps/shared/host/windows/ui-host.c` (`ICoreWebView2Controller::put_Bounds`), clipped via `SetWindowRgn` against `cd_over_rects` (or `ICoreWebView2CompositionController` + DirectComposition visual tree).

#### Path 2 — Off-Screen Rendering (OSR) Zero-Copy GPU Texture into `composition-v2` (Phase 3: `oa-browser-helper` CEF)
- Identical to `MetaverseViewportView.swift`'s `onExternalTextureUpdated` → `cd_mv_set_ext_texture`:
  - **macOS:** CEF `OnAcceleratedPaint` delivers a `Mach port / IOSurfaceRef` → imported as `MTLTexture` (`device.makeTexture(descriptor:iosurface:plane:)`) → sampled directly by `composition-v2` in `apps/shared/host/apple/ui-host.m`.
  - **Linux:** CEF delivers a `DMA-BUF` fd → imported via `VK_EXT_external_memory_dma_buf` into `apps/shared/host/vulkan-ui.c`.
  - **Windows:** CEF delivers a `HANDLE` (NT shared DXGI texture) → imported via `VK_KHR_external_memory_win32` into `apps/shared/host/vulkan-ui.c`.
- **Payoff:** Zero child-window z-order issues, true rounded corners and spring slide-in clipping during `coder.shell.pane` animations, live backdrop blur behind frosted Coax menus/command palette, and instant zero-readback GPU screenshots.

### 7.2 Focus, Keyboard, IME & Gesture Arbitration

- **Three distinct keyboard targets in `page/input.coax`:**
  1. `CP_KB_BROWSER_URL`: Coax omnibox (`UiEditor`) — receives typing, `⌘A/C/V`, autocomplete, `Return` to navigate, `Esc` to revert & return focus to viewport.
  2. `CP_KB_BROWSER_FIND`: Coax find-in-page bar (`UiEditor`).
  3. `CP_KB_BROWSER_VIEW`: Active web viewport — receives raw key events, IME composition, trackpad scroll/pinch, and mouse clicks.
- **Reserved Desktop Chord Interception:** Before forwarding keys to the web viewport, the host reserves shell-level chords (`⌘L` focus omnibox, `⌘T` new tab, `⌘W` close active tab, `⌘R` reload, `⌘[`/`⌘]` back/forward, `⇧⌘E` expand/restore, `⌥⌘B` hide pane, `⌘K` command palette, `⌘1..9` tab switch) unless the page is in explicit keyboard-capture/fullscreen mode.
- **Drag-and-Drop Safety:** Dragging a local file onto the chat column attaches it to chat; dragging onto the browser viewport only forwards to a `<input type="file">` drop target (never triggers top-level `file://` navigation).

### 7.3 Resource & Thermal Governance

- **Hidden / Occluded Views:** When `s[0].br[0].hidden` is true or another right-side pane is in front, active tab rendering pauses (`0 Hz` OSR frames; `WKWebView` hidden and throttled).
- **Tab Hibernation:** Up to `CP_BR_TABS = 12` tabs may be open in the strip, with a configurable active-renderer cap (default `4` live web processes; least-recently-used background tabs freeze/discard their renderer while preserving URL, title, favicon, scroll offset, and thumbnail snapshot, restoring transparently on click).
- **Audio Policy:** Background tabs auto-mute unless explicitly pinned/unmuted by the user; Row 1 pill shows a clickable speaker icon whenever a tab emits audio.

---

## 8. Bidirectional Chat ↔ Browser & Agent Automation

This is what makes our own browser **dramatically better than switching to Chrome**: tight, structured, human-supervised integration with the chat column and coding agents.

### 8.1 Human → Chat Context Injection

1. **DOM Element Picker (`⌖`):** Clicking any element captures `{ selector, xpath, tag, role, accessible_name, text_excerpt, bbox_px, computed_style_summary, url, viewport, screenshot_crop_path }` formatted as `openagents.desktop.browser-selection.v1` and attaches a structured chip into the composer (upgrading `Window+Preview.swift`'s prototype `oaDomPick`).
2. **One-Click Console / Network Attach:** From the drawer, click *"Send 3 errors to chat"* or *"Attach failed POST /api/login"* to inject structured stack traces / status codes / payloads into the composer.
3. **Annotated Viewport Screenshot (`📷`):** Captures the exact viewport (or picked element crop) into `~/.openagents/shots/` and attaches it to the composer.
4. **Open Current Page Source in File Viewer:** If the active URL maps to a local workspace dev server or HTML source, *"Open in File Viewer"* switches seamlessly to `filepane.coax`.

### 8.2 Agent → Browser Automation (`Browser` Tool & MCP Attach)

Mirroring `gamebuilder mcp --attach` (`Window+Metaverse.swift`), the desktop app exposes an owner-only Unix domain socket (`0600` inside `0700` `~/.openagents/desktop/browser.sock`, verified with `getpeereid` on macOS / `SO_PEERCRED` on Linux / token-authenticated `AF_UNIX` on Windows).

Workers (`oa-worker`, Claude Code, Inkling, external MCP clients) connect to the live open desktop pane (or spawn a headless tab in `Agent Ephemeral` profile) using one **canonical command table** (`browser_commands`):

| Verb | Parameters | Returns | Notes |
|---|---|---|---|
| `browser.open` | `url`, `profile?` (`ephemeral`\|`workspace`), `viewport?` | `tab_id`, `url`, `title` | Auto-opens Browser pane (if user allows) or creates background tab. |
| `browser.navigate` | `tab_id`, `url`, `wait_until?` | `url`, `status`, `title` | Enforces `#authority` `browser-route.coax` before navigation. |
| `browser.snapshot` | `tab_id`, `mode` (`a11y`\|`dom`\|`text`), `max_nodes?` | Compact numbered Accessibility/DOM tree (`@e1`, `@e2`, ...) | Token-efficient ref handles (`@e14`) so agents click reliably without brittle CSS selectors. |
| `browser.click` | `tab_id`, `ref` or `selector` or `(x, y)` | `ok`, `url_after`, `navigated` | Scrolls into view, highlights ripple in viewport so human sees what agent clicked. |
| `browser.type` | `tab_id`, `ref`, `text`, `submit?` | `ok` | Gated by approval if target is `<input type="password">` or payment field. |
| `browser.press` | `tab_id`, `key`, `mods?` | `ok` | Dispatches trusted key event. |
| `browser.scroll` | `tab_id`, `dx`, `dy`, `ref?` | `scroll_x`, `scroll_y` | Smoothly updates viewport. |
| `browser.screenshot` | `tab_id`, `full_page?`, `ref?` | `path`, `w`, `h` | Saves bounded PNG/JPEG and can emit a transcript card (`TK_BROWSER`). |
| `browser.console` | `tab_id`, `level?`, `since_seq?` | `entries[]` | Reads structured console logs & uncaught exceptions. |
| `browser.network` | `tab_id`, `failed_only?`, `since_seq?` | `requests[]` | Reads metadata (headers scrubbed of `Authorization`/`Cookie` by default). |
| `browser.eval` | `tab_id`, `expression` | `result_json` (bounded 64 KB) | Restricted to `Workspace`/`Agent Ephemeral` tabs; disabled on `Personal` unless explicitly granted. |
| `browser.wait` | `tab_id`, `selector?` / `text?` / `idle_ms?`, `timeout_ms` | `matched` | Deterministic wait without arbitrary sleeps. |

### 8.3 Human Supremacy & Safety Gates

- **Visual Lease Indicator:** While an agent holds a lease on a tab, the tab pill pulses a distinct badge, Row 3 shows `"Agent is driving · [Pause] [Take Over]"`, and the viewport renders a subtle colored highlight frame + transient click ripples.
- **Instant Takeover:** Any mouse click or keypress inside the viewport by the human immediately pauses the agent's lease (`BR_LEASE_PAUSED_BY_HUMAN`) and queues subsequent agent browser verbs until the human clicks *"Resume Agent"* or sends a steering message.
- **Sensitive Action Confirmation:** Before executing `browser.type` into sensitive fields (`type="password"`, `autocomplete="cc-*"`), `browser.click` on payment/delete/OAuth-consent buttons (detected via heuristic + origin policy), or any mutating verb on a `Personal` profile tab, Coax surfaces the inline Row 3 approval banner.
- **Secret Redaction:** `browser.network` and `browser.snapshot` automatically redact `Authorization`, `Cookie`, `Set-Cookie`, and `<input type="password">` values before returning results to any model/agent.

---

## 9. Coax Page State, C-ABI Bridge, Flags & Proto Contracts

### 9.1 Feature Flag (`coax/lib/agent/desktop/flags.coax`)

```coax
DC_FLAG_BROWSER :: 30;
DC_FLAG_COUNT   :: 31;
```
- `name = "browser"`, `default_on = false`, `desc = "Expandable multi-tab right-side Browser pane with DevTools drawer and agent automation."`
- While migrating, `DC_FLAG_PREVIEW` (`15`) remains compatible: when `DC_FLAG_BROWSER` is enabled, `browserpane.coax` takes precedence over legacy `preview.coax` (and inherits SnapShot region capture + dev-server port detection).

### 9.2 Non-Colliding Constant Allocations (`page/browserpane.coax`)

Verified against current highest IDs in `apps/openagents-desktop/page/` (`CP_ASK_SIDEBAR_VIEW :: 221`, `CP_CLOUD_BYO :: 521`, palette `39..45`, `CP_KB_*` up to `9`, `CP_SCR_*` up to `13`):

- **Host Asks (`CP_ASK_BR_*`):** `230..249`
  - `230` `CP_ASK_BR_NAVIGATE`, `231` `CP_ASK_BR_BACK`, `232` `CP_ASK_BR_FORWARD`, `233` `CP_ASK_BR_RELOAD`, `234` `CP_ASK_BR_STOP`, `235` `CP_ASK_BR_NEW_TAB`, `236` `CP_ASK_BR_CLOSE_TAB`, `237` `CP_ASK_BR_SELECT_TAB`, `238` `CP_ASK_BR_PICK_TOGGLE`, `239` `CP_ASK_BR_SHOT`, `240` `CP_ASK_BR_FIND`, `241` `CP_ASK_BR_MUTE`, `242` `CP_ASK_BR_EXTERNAL`, `243` `CP_ASK_BR_ZOOM`, `244` `CP_ASK_BR_PROFILE`, `245` `CP_ASK_BR_ATTACH_LOGS`, `246` `CP_ASK_BR_AGENT_PAUSE`, `247` `CP_ASK_BR_AGENT_APPROVE`, `248` `CP_ASK_BR_ENGINE_INSTALL`, `249` `CP_ASK_BR_OPEN_SOURCE`
- **UI Action IDs (`CP_BR_*`):** `540..579`
- **Command Palette Codes (`CP_PC_BR_*`):** `46..51` (`Toggle Browser`, `Open URL in Browser`, `New Browser Tab`, `Expand Browser`, `Pick DOM Element`, `Browser Screenshot`)
- **Keyboard & Scroll Targets:**
  - `CP_KB_BROWSER_URL :: 10`, `CP_KB_BROWSER_FIND :: 11`, `CP_KB_BROWSER_VIEW :: 12`
  - `CP_SCR_BROWSER_DRAWER :: 14`

### 9.3 Coax Page State Structs (`page/browserpane.coax`)

```coax
CP_BR_TABS       :: 12;
CP_BR_URL_CAP    :: 2048;
CP_BR_TITLE_CAP  :: 128;
CP_BR_ORIGIN_CAP :: 128;
CP_BR_BAR1_H     :: 38.0f32;
CP_BR_BAR2_H     :: 30.0f32;
CP_BR_BAR3_H     :: 26.0f32;
CP_BR_DRAWER_W   :: 300.0f32;

CP_BR_PROFILE_WORKSPACE :: 0;
CP_BR_PROFILE_PERSONAL  :: 1;
CP_BR_PROFILE_EPHEMERAL :: 2;

CP_BR_ENGINE_SYSTEM :: 0; // WKWebView / WebKitGTK / WebView2
CP_BR_ENGINE_CEF    :: 1; // On-demand oa-browser-helper (CEF OSR + CDP)

CP_BR_DRW_CONSOLE   :: 0;
CP_BR_DRW_NETWORK   :: 1;
CP_BR_DRW_DOM       :: 2;
CP_BR_DRW_AGENT     :: 3;
CP_BR_DRW_BOOKMARKS :: 4;

CpBrowserTab :: struct {
  used: bool;
  id: int;                              // stable host tab id
  profile: int;                         // CP_BR_PROFILE_*
  engine: int;                          // CP_BR_ENGINE_*
  url: [CP_BR_URL_CAP] u8; url_n: int;
  title: [CP_BR_TITLE_CAP] u8; title_n: int;
  origin: [CP_BR_ORIGIN_CAP] u8; origin_n: int;
  loading: bool;
  load_pct: int;                        // 0..100
  can_back: bool;
  can_forward: bool;
  secure: bool;                         // https or loopback
  audible: bool;
  muted: bool;
  hibernated: bool;
  zoom_pct: int;                        // default 100
  err_count: int;                       // console error badge
  warn_count: int;
  agent_leased: bool;                   // currently controlled by an agent run
  agent_paused: bool;                   // human paused/took over
  agent_label: [48] u8; agent_label_n: int;
}

CpBrowserPane :: struct {
  open: bool;
  front: bool;
  hidden: bool;                         // ◫ collapsed preserving tabs & live webviews
  expanded: bool;                       // ⛶ full main area beside left sidebar
  drawer_open: bool;                    // ◧ collapsible DevTools/Agent drawer
  drawer_tab: int;                      // CP_BR_DRW_*
  tabs: [CP_BR_TABS] CpBrowserTab;
  active: int;                          // -1 if no tabs
  next_id: int;
  viewport: int;                        // CP_VP_DESKTOP | CP_VP_TABLET | CP_VP_MOBILE
  picking: bool;                        // ⌖ DOM element picker active
  find_open: bool;
  find_matches: int;
  find_cur: int;
  more_menu: bool;
  ports_menu: bool;
  ports: [8] int; nports: int;
  ext_tex_ready: bool;                  // true when CEF OSR texture is bound in composition-v2
  body_x: f32; body_y: f32; body_w: f32; body_h: f32;
  // Pending approval prompt from agent
  approval_open: bool;
  approval_id: int;
  approval_text: [256] u8; approval_text_n: int;
}
```

### 9.4 Engine Abstraction Header (`apps/openagents-desktop/br_host.h`)

A narrow, platform- and engine-neutral C ABI (matching `mv_host.h` and `gb_host.h`):

```c
typedef struct OaBrowserHost OaBrowserHost;

typedef enum {
  OA_BR_BACKEND_SYSTEM = 0, // WKWebView / WebKitGTK / WebView2
  OA_BR_BACKEND_CEF_OSR = 1 // Out-of-process Chromium/CEF helper with shared GPU textures + CDP
} OaBrowserBackendKind;

typedef struct {
  void (*on_url)(void *ctx, int tab_id, const char *url, int secure);
  void (*on_title)(void *ctx, int tab_id, const char *title);
  void (*on_load_state)(void *ctx, int tab_id, int loading, int pct, int can_back, int can_fwd);
  void (*on_console)(void *ctx, int tab_id, int level, const char *msg, const char *src, int line);
  void (*on_network)(void *ctx, int tab_id, const char *method, int status, const char *url, int ms);
  void (*on_dom_pick)(void *ctx, int tab_id, const char *selection_json);
  void (*on_frame_texture)(void *ctx, int tab_id, void *native_tex_handle, int w, int h);
  void (*on_crashed)(void *ctx, int tab_id, const char *reason);
} OaBrowserCallbacks;

OaBrowserHost *oa_br_create(OaBrowserBackendKind kind, void *native_parent, const OaBrowserCallbacks *cb, void *ctx);
void           oa_br_destroy(OaBrowserHost *h);
int            oa_br_tab_open(OaBrowserHost *h, int tab_id, int profile, const char *url);
void           oa_br_tab_close(OaBrowserHost *h, int tab_id);
void           oa_br_tab_activate(OaBrowserHost *h, int tab_id);
void           oa_br_tab_navigate(OaBrowserHost *h, int tab_id, const char *url);
void           oa_br_tab_nav_action(OaBrowserHost *h, int tab_id, int op); // 0=back, 1=fwd, 2=reload, 3=stop
void           oa_br_tab_set_bounds(OaBrowserHost *h, int tab_id, float x, float y, float w, float h, float dpr);
void           oa_br_tab_set_over_rects(OaBrowserHost *h, int tab_id, const float *rects4, int count);
void           oa_br_tab_set_picking(OaBrowserHost *h, int tab_id, int active);
int            oa_br_tab_exec_cmd(OaBrowserHost *h, int tab_id, const char *cmd_json, char *out_json, int out_cap);
```

### 9.5 Proto Contracts (`proto/coder/` & `proto/registry.json`)

Four new versioned JSON Schemas registered in `proto/registry.json`:
1. `openagents.desktop.browser-route.v1` (`proto/coder/desktop-browser-route-v1.json`) — `#authority` route classification (`pane`, `profile`, `normalized_url`, `reason`).
2. `openagents.desktop.browser-selection.v1` (`proto/coder/desktop-browser-selection-v1.json`) — DOM element pick payload attached to chat composer.
3. `openagents.desktop.browser-tab-state.v1` (`proto/coder/desktop-browser-tab-state-v1.json`) — Persisted session restore state (tab URLs, titles, profiles, active index, expanded/hidden flags).
4. `openagents.desktop.browser-cmd.v1` (`proto/coder/desktop-browser-cmd-v1.json`) — Request/response envelope for agent automation over `~/.openagents/desktop/browser.sock` and MCP.

---

## 10. File-by-File Implementation Map

| Area | New / Modified Files | Responsibility |
|---|---|---|
| **Authority & Flags** | `coax/lib/agent/desktop/flags.coax` | Add `DC_FLAG_BROWSER :: 30` (`default_on = false`). |
| | `coax/lib/agent/desktop/browser-route.coax` *(new, `#authority`)* | URL normalization, scheme confinement, loopback/private-network rules, profile classification, JSON codecs + `#test` suite. |
| **Proto Contracts** | `proto/coder/desktop-browser-{route,selection,tab-state,cmd}-v1.json` *(new)*, `proto/registry.json` | Versioned wire schemas. |
| **Coax Desktop Page** | `apps/openagents-desktop/page/browserpane.coax` *(new)* | Full two-row chrome, multi-tab strip, inline omnibox, viewport presets, Row 3 contextual banners, collapsible 5-tab DevTools/Agent drawer, C exports. |
| | `apps/openagents-desktop/page/layout/layout.coax` | Wire `cp_br_shown(s)` into `cp_other_pane_open`, `cp_side_pane`, and the `expanded` full-width override (line 145). |
| | `apps/openagents-desktop/page/state.coax`, `input.coax`, `palette.coax`, `persist.coax`, `v1-stubs.coax` | Add `br: [1] CpBrowserPane`, keyboard/scroll routing, palette entries, tab session persistence, and `--minimal` stubs. |
| | `apps/openagents-desktop/page/checks/browserpane-checks.coax` *(new)* | Unit/layout/interaction checks for tabs, expand/hide, omnibox, approval banners, and drawer. |
| **C Engine Bridge** | `apps/openagents-desktop/br_host.h`, `br_host.c` *(new)* | Engine-neutral C-ABI dispatch + owner-only Unix attach socket (`browser.sock`). |
| **macOS Host** | `apps/openagents-desktop/Window+Browser.swift` *(new)*, `BrowserViewportView.swift` *(new)*, `Window.swift` | Phase 1 `WKWebView` backend with `WKWebsiteDataStore` profiles, `CAShapeLayer` `cd_over_rects` hole-cutting, unified JS bridge (`oa-browser-bridge.js`), and `syncBrowserFrame()`. |
| **Linux Host** | `apps/openagents-desktop/linux/browser_webkit.c` *(new)*, `linux/main.c` | Dynamically loaded `libwebkitgtk-6.0` Wayland subsurface backend implementing `br_host.h`. |
| **Windows Host** | `apps/openagents-desktop/windows/browser_webview2.c` *(new)*, `windows/main.c` | Edge `WebView2` COM backend implementing `br_host.h`. |
| **Phase 3 Pro Engine** | `apps/oa-browser-helper/` *(new separate build target)* | Out-of-process CEF OSR helper publishing `IOSurface`/`DMA-BUF`/Win32 handles + native CDP over IPC; hydrated via signed manifest. |
| **Agent Integration** | `coax/lib/agent/tools/browser.coax` *(new)*, `apps/openagents-cli/` | Exposes `Browser` tool + `openagents browser mcp --attach` talking to `browser.sock`. |

---

## 11. Phased Delivery Plan, Verification Matrix & Risk Register

### 11.1 Phased Delivery Plan

- **Phase 0 — Policy, Contracts & Shell Skeleton (~1 week):**
  - Add `DC_FLAG_BROWSER`, `#authority` `browser-route.coax` with full security tests, `proto/coder/desktop-browser-*.json`, and `v1-stubs.coax` isolation.
  - Verify `build.sh --minimal` stays byte-clean and `layer-check` passes.
- **Phase 1 — Expandable Multi-Tab Browser Pane on OS-Native WebViews (~2–3 weeks):**
  - Implement `page/browserpane.coax` (Row 1 tab strip, Row 2 omnibox + nav + viewport presets, `⛶` Expand/Restore, `◫` Hide, `cd_over_rects` hole-cutting so popovers/menus float cleanly without flashing).
  - Ship macOS `WKWebView` backend (`Window+Browser.swift`) with 3 isolated `WKWebsiteDataStore` profiles and unified `oa-browser-bridge.js` (DOM picker, console capture, fetch/XHR instrumentation, a11y tree walk).
  - Add Linux `WebKitGTK 6` and Windows `WebView2` backends behind the same `br_host.h`.
  - Deprecate legacy single-tab `page/preview.coax` once feature-complete (migrating its SnapShot & port-chip flows into `browserpane.coax`).
- **Phase 2 — DevTools Drawer & Human-Supervised Agent Automation (~2–3 weeks):**
  - Build collapsible right drawer (`Console`, `Network`, `DOM/A11y`, `Agent Log`, `Bookmarks`).
  - Bring up owner-only `~/.openagents/desktop/browser.sock` + `openagents browser mcp --attach` + `Browser` worker tool.
  - Implement visual agent lease border, instant human Pause/Takeover, Row 3 sensitive-action approval banner, secret redaction, and `TK_BROWSER` transcript cards.
- **Phase 3 — On-Demand `oa-browser-helper` (CEF OSR + Native CDP + Zero-Copy GPU Textures) (~4–6 weeks):**
  - Build signed, on-demand downloadable `oa-browser-helper` packages for macOS Arm64/x64, Linux x64/Arm64, Windows x64/Arm64.
  - Wire `OnAcceleratedPaint` (`IOSurface`/`DMA-BUF`/Win32 shared texture) directly into `composition-v2` (matching `cd_mv_set_ext_texture`) for seamless frosted overlays and spring-animated pane slides.
  - Wire native CDP (`DOMSnapshot`, `Accessibility.getFullAXTree`, `Network`, `Runtime`, `Input`) behind `oa_br_tab_exec_cmd` for identical cross-platform agent reliability.
- **Phase 4 — Polish & Long-Term Sovereignty (Ongoing):**
  - Session restore across restarts (`persist.coax`), tab hibernation under memory pressure, download manager row, keyboard-driven navigation hints, and an experimental `OA_BR_BACKEND_SERVO` / `Ladybird` plug-in slot behind `br_host.h`.

### 11.2 Verification Matrix

| Layer | Automated Check |
|---|---|
| **Authority & Security** | `coax/bin/coax test coax/lib/agent/desktop/browser-route.coax --library` — scheme denylist (`javascript:`, `file:`, `data:`), SSRF/traversal checks, profile routing, JSON codecs. |
| **Coax Page UI** | `coax/bin/coax test apps/openagents-desktop/page/checks/browserpane-checks.coax` — 12-tab lifecycle, Expand/Restore geometry, Hide preserving tabs, omnibox focus, approval banner, drawer tabs. |
| **Minimal Isolation** | `apps/openagents-desktop/build.sh --minimal` compiles cleanly with zero WebKit/CEF symbols. |
| **Overlay Compositing** | Live macOS drive test (`docs/desktop/macos-drive-and-screenshots.md`): open Command Palette (`⌘K`) and `⋮` menu over a live scrolling page; assert zero occlusion or viewport flicker. |
| **Agent Lease & Human Supremacy** | Integration test over `browser.sock`: agent opens `localhost` fixture, snapshots `@e*` refs, clicks button; simulated human click pauses lease immediately and blocks subsequent agent mutation until resumed. |
| **Secret Redaction** | Assert `Authorization`, `Cookie`, and `<input type="password">` values never appear in `browser.network` or `browser.snapshot` outputs. |
| **Cross-Platform Builds** | `apps/openagents-desktop/linux/build.sh` and `windows/build.sh --check` link cleanly with optional runtime `dlopen` / WebView2 loader. |

### 11.3 Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **JS-shim limitations on `WKWebView` (cross-origin iframes, strict CSP sites)** | High | Medium | Document clearly for Backend 1; Phase 3 CEF OSR + native CDP bypasses page CSP/iframe limits cleanly out-of-process. |
| **Chromium CVE maintenance burden in Phase 3** | High | High | Keep CEF as an opt-in, separately signed, independently updatable `oa-browser-helper` artifact so security patches ship in hours without rebuilding Desktop. |
| **AppKit / Wayland / Win32 child-view z-order & focus glitches** | Medium | Medium | Reuse proven `cd_over_rects` cutout mask (`MetaverseViewportView.swift`) in Phase 1; eliminate permanently via `composition-v2` zero-copy textures in Phase 3. |
| **Agent prompt-injection via untrusted web page content** | High | High | Run autonomous browsing in `Agent Ephemeral` profile by default, redact secrets, require Row 3 human confirmation for credentials/payments/forms, and preserve instant human takeover. |
| **RAM bloat from many open tabs** | Medium | Medium | Enforce 4-renderer active cap + LRU tab hibernation (`hibernated = true`) and `0 Hz` compositing when pane is hidden (`◫`). |
