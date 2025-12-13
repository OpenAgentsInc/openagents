# Zed Web Support: Technical Architecture and Implementation Paths

**Status:** Planned for post-1.0 release (Spring 2026+)
**Last Updated:** December 2025
**Maintainer:** Community Documentation

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Status](#current-status)
3. [Why Web Support Doesn't Exist Yet](#why-web-support-doesnt-exist-yet)
4. [Technical Architecture Overview](#technical-architecture-overview)
5. [Blade vs wgpu: The Rendering Backend Decision](#blade-vs-wgpu-the-rendering-backend-decision)
6. [How Zed Could Add Web Support Today](#how-zed-could-add-web-support-today)
7. [Implementation Paths](#implementation-paths)
8. [Technical Challenges](#technical-challenges)
9. [Comparison with Other Editors](#comparison-with-other-editors)
10. [Recommended Approach](#recommended-approach)
11. [References](#references)

---

## Executive Summary

Zed is a high-performance, GPU-accelerated code editor written in Rust. While web support is on the [official roadmap](https://zed.dev/roadmap) under "Beyond Zed 1.0", it's not currently prioritized. This document explores:

- **Why** web support doesn't exist yet (technical and strategic reasons)
- **How** the current architecture could be adapted for the web
- **What** implementation paths are available, with detailed pros/cons
- **When** it might make sense to tackle this challenge

**TL;DR:** Zed _could_ add web support today, but it would require significant architectural changes. The main blocker isn't impossibility—it's strategic prioritization and the substantial engineering effort required.

---

## Current Status

### Platform Support (As of December 2025)

| Platform | Status | Rendering Backend | Release Date |
|----------|--------|-------------------|--------------|
| macOS    | ✅ Stable | Metal (via Blade) | 2022 |
| Linux    | ✅ Stable | Vulkan (via Blade) | 2024 |
| Windows  | ✅ Stable | Vulkan/DX12 (via Blade) | 2024 |
| Web      | ⏳ Planned | N/A | Post-1.0 (2026+) |
| iOS/iPadOS | 📋 Under Discussion | N/A | TBD |
| Android  | 📋 Under Discussion | N/A | TBD |

### Official Stance

From the [Zed roadmap](https://zed.dev/roadmap):
> **Web Support** (Beyond Zed 1.0): Open projects from any device. The full power of Zed wherever you are.

From historical [GitHub Discussion #26195](https://github.com/zed-industries/zed/discussions/26195) and [Issue #5391](https://github.com/zed-industries/zed/issues/5391):
> "We're focusing on a single platform for now because we're a small team. At our current scale, maintaining additional platforms would represent a big cost without much additional learning. We plan to support Linux and Windows before 1.0, and to support a web version some time after that."

---

## Why Web Support Doesn't Exist Yet

### 1. **Strategic Prioritization**

The Zed team made a deliberate decision to perfect one platform at a time:

```
Phase 1: macOS (Metal) → ✅ Complete (2022-2023)
Phase 2: Linux (Vulkan) → ✅ Complete (2024)
Phase 3: Windows (DX12/Vulkan) → ✅ Complete (2024)
Phase 4: Web (WebGPU/WebGL) → ⏳ Planned (2026+)
```

**Rationale:** A small team can iterate faster and learn more by mastering native platforms before tackling the web's unique constraints.

### 2. **Technical Architecture Constraints**

Zed's architecture was optimized for native performance, not web portability:

#### **GPUI Framework**
- GPU-accelerated UI framework (similar to Flutter)
- Immediate + retained mode hybrid
- Built specifically for desktop applications
- Tight coupling to native platform APIs

#### **Blade Graphics Backend**
- Low-level GPU abstraction focused on ergonomics and Metal/Vulkan
- **Primary backends:** Metal (macOS), Vulkan (Linux/Windows)
- **Secondary backend:** GLES (experimental, "basic level" for WASM)
- **No native WebGPU support** (as of Blade 0.7.0)

#### **Platform Abstractions**
Located in `crates/gpui/src/platform/`:
```
platform/
├── mac.rs           # macOS (Metal via Blade)
├── linux.rs         # Linux (Vulkan via Blade)
├── windows.rs       # Windows (DX12/Vulkan via Blade)
├── test.rs          # Headless testing
├── blade/           # Blade renderer integration
│   ├── blade_renderer.rs
│   ├── blade_atlas.rs
│   └── shaders.wgsl # WebGPU-compatible shaders!
└── [no web.rs yet]
```

**Key observation:** Each platform requires 1000+ lines of integration code for windowing, input handling, file system access, and rendering surface setup.

### 3. **Dependency on Native APIs**

Zed relies heavily on platform-specific capabilities:

| Capability | Native API | Web Equivalent | Complexity |
|------------|-----------|----------------|------------|
| File System | Direct OS calls | File System Access API | High |
| GPU Rendering | Metal/Vulkan | WebGPU/WebGL2 | Medium |
| Text Layout | CoreText/DirectWrite/FreeType | Canvas/OffscreenCanvas | High |
| Windowing | NSWindow/X11/Win32 | Browser window | Low |
| IPC/Collab | Native sockets | WebRTC/WebSockets | Medium |
| LSP Servers | Native processes | WASM or server-side | High |

### 4. **Performance Philosophy**

Zed's entire value proposition is **native-level performance**:
- Direct GPU access without browser overhead
- Zero-copy memory operations
- Minimal abstraction layers
- Optimized for 60fps UI at all times

The web introduces unavoidable overhead that conflicts with this philosophy (though it's narrowing with WebGPU).

---

## Technical Architecture Overview

### Layer 1: Application Core (`crates/zed/`, `crates/editor/`, etc.)
- Language-agnostic business logic
- **Web compatibility:** ✅ High (pure Rust)

### Layer 2: GPUI Framework (`crates/gpui/`)
- UI framework with rendering abstraction
- **Web compatibility:** ⚠️ Medium (needs platform adapter)

### Layer 3: Blade Graphics (`blade-graphics` dependency)
- Low-level GPU abstraction
- **Web compatibility:** ⚠️ Low (limited GLES support only)

### Layer 4: Platform Layer (`crates/gpui/src/platform/`)
- Windowing, input, file system
- **Web compatibility:** ❌ None (needs complete rewrite)

### Rendering Pipeline

```
GPUI Elements (Rust)
      ↓
Scene Graph Generation
      ↓
Blade Renderer
      ↓ (Platform-specific)
   ┌──┴──┬──────┬─────────┐
   │     │      │         │
 Metal Vulkan DX12   GLES/WebGL
   │     │      │         │
 macOS Linux Windows    Web?
```

**Critical insight:** The shaders are already written in **WGSL** (WebGPU Shading Language)! See `crates/gpui/src/platform/blade/shaders.wgsl`. This means the shader code is already web-compatible.

---

## Blade vs wgpu: The Rendering Backend Decision

This is the **most important architectural decision** for web support.

### Blade Graphics (Current)

**What is it?**
> Sharp and simple graphics library. An innovative rendering solution that starts with a lean low-level GPU abstraction focused at ergonomics and fun, and grows into a high-level rendering library that utilizes hardware ray-tracing.

**Developed by:** [kvark](https://github.com/kvark) (Dzmitry Malyshau), who also contributed to wgpu and WebGPU standardization.

**Repository:** [github.com/kvark/blade](https://github.com/kvark/blade)

**Backend Support:**
```rust
// From Blade documentation
Primary:
  ✅ Vulkan (Linux, Windows, Android)
  ✅ Metal (macOS, iOS)

Secondary:
  ⚠️ GLES (via Angle or native, "basic level")
  ❌ WebGPU (no native support)
  ❌ DirectX 12 (in progress)
```

**WASM Support:**
> "GLES is also supported at a basic level. It's enabled for wasm32-unknown-unknown target, and can also be force-enabled on native."

**Pros:**
- ✅ Simpler API (less boilerplate)
- ✅ Optimized for Zed's specific use case
- ✅ Direct control and customization
- ✅ Smaller dependency footprint
- ✅ Faster iteration (can patch immediately)

**Cons:**
- ❌ No native WebGPU backend
- ❌ WASM support is "basic level" (experimental)
- ❌ Smaller community (single maintainer)
- ❌ Less documentation
- ❌ Would need significant WebGPU work

**Current Usage in Zed:**
```toml
# From Cargo.toml
[workspace.dependencies]
blade-graphics = { version = "0.7.0" }
blade-macros = { version = "0.3.0" }
blade-util = { version = "0.3.0" }
```

### wgpu (Alternative)

**What is it?**
> A cross-platform, safe, pure-Rust graphics API. It runs natively on Vulkan, Metal, D3D12, and OpenGL; and on top of WebGL2 and WebGPU on wasm.

**Repository:** [github.com/gfx-rs/wgpu](https://github.com/gfx-rs/wgpu)

**Website:** [wgpu.rs](https://wgpu.rs/)

**Backend Support:**
```rust
Native:
  ✅ Vulkan (Linux, Windows, Android)
  ✅ Metal (macOS, iOS)
  ✅ DirectX 12 (Windows)
  ✅ OpenGL ES (Linux, Windows, macOS via Angle)

Web:
  ✅ WebGPU (Chrome 113+, Edge 113+, Safari 18+, Firefox 141+)
  ✅ WebGL2 (all modern browsers)
```

**WASM Support:**
> "When running in a web browser (by compilation to WebAssembly) without the 'webgl' feature enabled, wgpu relies on the browser's own WebGPU implementation. When running with wgpu's 'webgl' feature enabled, wgpu uses Naga to translate WGSL programs into GLSL."

**Pros:**
- ✅ **First-class WebGPU support**
- ✅ **Production-ready WASM support**
- ✅ Used in Firefox, Deno (proven at scale)
- ✅ Large community and ecosystem
- ✅ Excellent documentation
- ✅ Shader transpilation (WGSL → GLSL/SPIR-V/Metal)
- ✅ WebGL2 fallback for older browsers

**Cons:**
- ❌ More complex API
- ❌ Larger binary size
- ❌ More abstraction layers
- ❌ Slower upstream bug fixes

**Notable Users:**
- 🔥 **Firefox** (WebGPU implementation)
- 🦕 **Deno** (runtime graphics)
- 🎮 **Bevy** (game engine)
- ✏️ **egui** (immediate mode GUI, has web support)

### Browser WebGPU Support (2025)

| Browser | WebGPU | Since | Notes |
|---------|--------|-------|-------|
| Chrome/Edge | ✅ | v113 (Apr 2023) | Stable |
| Safari | ✅ | v18 (Jun 2025) | Stable |
| Firefox | ✅ | v141 (Jul 2025) | Uses wgpu! |
| Mobile Safari | ⚠️ | iOS 18+ | Limited |
| Mobile Chrome | ⚠️ | Experimental | Behind flag |

**Coverage:** ~70% of desktop users, ~30% of mobile users (as of Q4 2025)

**Fallback:** WebGL2 supported by 95%+ of all browsers

### Comparison Matrix

| Feature | Blade | wgpu | Winner |
|---------|-------|------|--------|
| WebGPU Support | ❌ | ✅ | wgpu |
| WASM Maturity | ⚠️ Basic | ✅ Production | wgpu |
| Native Performance | ✅ | ✅ | Tie |
| API Simplicity | ✅ | ⚠️ | Blade |
| Zed Integration | ✅ Deep | ❌ None | Blade |
| Community Size | Small | Large | wgpu |
| Binary Size | Smaller | Larger | Blade |
| Shader Language | WGSL | WGSL + transpilation | wgpu |

### Verdict

**For web support:**
- 🥇 **wgpu** is the clear technical winner
- Switching would require rewriting `crates/gpui/src/platform/blade/` (~2000 lines)
- But gains first-class WASM support with minimal web-specific code

**To keep Blade:**
- Would need to contribute WebGPU backend to Blade
- Estimated effort: 2-4 months for one engineer
- Benefits entire Rust ecosystem
- Aligns with Zed's open-source philosophy

---

## How Zed Could Add Web Support Today

### Option 1: Quick & Dirty - Blade + WebGL via Emscripten

**Approach:** Compile existing Blade GLES backend to WASM, use Emscripten's WebGL bindings.

**Changes needed:**
1. Add `wasm32-unknown-unknown` target support
2. Create `crates/gpui/src/platform/web.rs` (~1500 lines)
3. Implement browser DOM bindings for windowing
4. Use Emscripten for GL context creation
5. Polyfill file system APIs with IndexedDB/OPFS

**Pros:**
- ✅ Minimal changes to existing codebase
- ✅ No dependency changes
- ✅ Reuses all existing Blade code

**Cons:**
- ❌ WebGL has worse performance than WebGPU
- ❌ Blade GLES support is "basic level" (experimental)
- ❌ Missing modern GPU features
- ❌ Larger WASM binary due to Emscripten overhead

**Estimated effort:** 4-6 weeks (1 engineer)

**Viability:** ⚠️ Proof-of-concept only, not production-ready

---

### Option 2: Modern - Switch to wgpu

**Approach:** Replace Blade with wgpu throughout the codebase, get WebGPU support for free.

**Changes needed:**
1. Replace `blade-graphics` dependency with `wgpu`
2. Rewrite `crates/gpui/src/platform/blade/` → `platform/gpu/` (~2000 lines)
3. Update shader pipeline (WGSL compatible, minimal changes)
4. Create `platform/web.rs` (~1000 lines)
5. Add wasm-bindgen bindings for browser APIs
6. Implement OPFS for file system

**Pros:**
- ✅ **First-class WebGPU support**
- ✅ Production-ready WASM story
- ✅ WebGL2 fallback built-in
- ✅ Proven in Firefox/Deno
- ✅ Future-proof

**Cons:**
- ❌ Large refactor (touches every rendering call)
- ❌ Risk of performance regressions on native
- ❌ Needs extensive testing
- ❌ Potential binary size increase

**Estimated effort:** 3-4 months (1 engineer)

**Viability:** ✅ Production-ready path

**Migration strategy:**
```rust
// Phase 1: Add wgpu alongside Blade (feature flag)
#[cfg(feature = "wgpu-renderer")]
mod gpu;
#[cfg(not(feature = "wgpu-renderer"))]
mod blade;

// Phase 2: Test parity on native platforms
cargo test --features wgpu-renderer

// Phase 3: Enable for web target only
#[cfg(target_arch = "wasm32")]
use gpu as renderer;
#[cfg(not(target_arch = "wasm32"))]
use blade as renderer;

// Phase 4: (Optional) Deprecate Blade entirely
```

---

### Option 3: Hybrid - Blade Native, wgpu Web

**Approach:** Keep Blade for native, use wgpu only for WASM target.

**Changes needed:**
1. Add wgpu dependency with `target_arch = "wasm32"` condition
2. Create abstraction trait over both backends
3. Implement trait for both Blade and wgpu
4. Conditional compilation per target

**Code structure:**
```rust
// crates/gpui/src/platform/renderer.rs
pub trait GpuRenderer {
    fn create_context(...) -> Self;
    fn render_frame(&mut self, scene: &Scene);
    // ... other methods
}

// crates/gpui/src/platform/blade_renderer.rs
#[cfg(not(target_arch = "wasm32"))]
impl GpuRenderer for BladeRenderer { ... }

// crates/gpui/src/platform/wgpu_renderer.rs
#[cfg(target_arch = "wasm32")]
impl GpuRenderer for WgpuRenderer { ... }
```

**Pros:**
- ✅ Keep existing native performance
- ✅ No risk to native platforms
- ✅ Get web support via proven library
- ✅ Smaller change surface

**Cons:**
- ❌ Maintain two rendering backends
- ❌ Risk of behavior divergence
- ❌ Testing complexity (2x paths)
- ❌ Future maintenance burden

**Estimated effort:** 2-3 months (1 engineer)

**Viability:** ✅ Pragmatic compromise

---

### Option 4: Revolutionary - Contribute WebGPU to Blade

**Approach:** Add first-class WebGPU backend to Blade, making it truly cross-platform.

**Changes needed:**
1. Fork Blade or work with upstream (kvark)
2. Implement WebGPU backend alongside Vulkan/Metal (~3000 lines)
3. Test on all browsers
4. Create `platform/web.rs` for GPUI (~1000 lines)
5. Upstream changes back to Blade

**Pros:**
- ✅ Benefits entire Rust ecosystem
- ✅ Aligns with Zed's open-source values
- ✅ Keep Blade's simplicity
- ✅ Could become the standard
- ✅ Full control over implementation

**Cons:**
- ❌ Massive engineering effort
- ❌ Uncertain timeline
- ❌ Depends on upstream maintainer
- ❌ Highest risk

**Estimated effort:** 4-6 months (2 engineers)

**Viability:** 🚀 High-impact, high-risk

**Community impact:**
- Could make Blade the go-to graphics library for Rust web apps
- Similar to how wgpu enabled the current Rust WebGPU ecosystem

---

## Implementation Paths

### Path A: Minimal Viable Web Version (MVP)

**Goal:** Get *something* running in a browser within 2 months.

**Approach:** Option 1 (Blade + WebGL)

**Features:**
- ✅ Basic text editing
- ✅ Syntax highlighting
- ✅ Multiple cursors
- ❌ LSP (server-side proxy only)
- ❌ Git integration
- ❌ Local file system (cloud projects only)
- ❌ Extensions

**Target users:** Demos, marketing, GitHub Codespaces-style use cases

**Trade-offs:** Not feature-complete, but proves feasibility

---

### Path B: Production Web Version

**Goal:** Feature parity with desktop version.

**Approach:** Option 2 (wgpu) or Option 3 (Hybrid)

**Timeline:** 6-9 months

**Features:**
- ✅ Full editor capabilities
- ✅ LSP via WASM or server proxy
- ✅ File System Access API integration
- ✅ Extensions (WASM-compatible only)
- ✅ Git (via libgit2 WASM or server proxy)
- ✅ Collaboration

**Technical requirements:**

#### 1. Platform Abstraction Layer
```rust
// crates/gpui/src/platform/web.rs

use wasm_bindgen::prelude::*;
use web_sys::{Window, Document, HtmlCanvasElement};

pub struct WebPlatform {
    window: Window,
    canvas: HtmlCanvasElement,
    gpu_context: WgpuContext,
    // ...
}

impl Platform for WebPlatform {
    fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
        // Use requestAnimationFrame loop
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        // Use Screen API
    }

    // ... implement all Platform trait methods
}
```

#### 2. File System Abstraction
```rust
// Options:
// 1. File System Access API (Chrome 86+, Edge 86+)
// 2. IndexedDB (all browsers, but async API)
// 3. OPFS - Origin Private File System (Chrome 102+)
// 4. Server-side proxy (like VS Code Web)

#[async_trait::async_trait]
impl Fs for WebFs {
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        {
            // Use OPFS or IndexedDB
            read_from_opfs(path).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Use std::fs
            std::fs::read(path)
        }
    }
}
```

#### 3. LSP Strategy
```rust
// Option A: WASM-based LSP servers
// Compile rust-analyzer, typescript-language-server to WASM
// Run in Web Worker

// Option B: Server-side proxy (like VS Code Web)
// Browser <-> WebSocket <-> Server <-> LSP process

// Option C: Hybrid
// Simple LSPs in WASM (JSON, TOML)
// Complex LSPs server-side (Rust, TypeScript)
```

#### 4. Collaboration Architecture
```rust
// Already WebRTC-based, should work!
// Just need to ensure WASM compatibility

// crates/collab/src/
// Already uses WebSocket for signaling
// Should require minimal changes
```

---

### Path C: Progressive Enhancement

**Goal:** Ship incrementally, iterate based on feedback.

**Timeline:** 12-18 months

**Phase 1 (Months 1-3): Foundation**
- Set up WASM build pipeline
- Basic rendering (wgpu or Blade+WebGL)
- Read-only text viewing
- Deploy to zed.dev/web

**Phase 2 (Months 4-6): Editing**
- Input handling (keyboard, mouse, touch)
- Text editing capabilities
- Syntax highlighting
- File explorer (IndexedDB)

**Phase 3 (Months 7-9): Intelligence**
- LSP integration (server-proxy)
- Autocomplete
- Diagnostics
- Go-to-definition

**Phase 4 (Months 10-12): Polish**
- Performance optimization
- File System Access API
- Git integration
- Extensions (WASM subset)

**Phase 5 (Months 13-18): Parity**
- Collaboration
- Debugging
- Advanced features
- Mobile optimization

**Advantages:**
- ✅ Learn and iterate
- ✅ Early user feedback
- ✅ Manageable risk
- ✅ Revenue sooner (Zed Cloud integration)

---

## Technical Challenges

### 1. Text Rendering

**Desktop:** Uses CoreText (macOS), DirectWrite (Windows), FreeType (Linux)

**Web:** Canvas API, OffscreenCanvas, or custom WGSL shaders

**Challenges:**
- Font hinting differences
- Subpixel rendering
- Emoji support
- Complex script shaping (Arabic, Devanagari, etc.)

**Solution:**
- Use [cosmic-text](https://github.com/pop-os/cosmic-text) (already in use!)
- Fallback to Canvas TextMetrics API
- Ship web fonts

### 2. File System

**Desktop:** Direct OS APIs

**Web:** Constrained by browser security model

**Options:**

| API | Browser Support | Capabilities | Limitations |
|-----|----------------|--------------|-------------|
| File System Access API | Chrome 86+, Edge 86+ | Read/write local files | Requires user permission per directory |
| OPFS | Chrome 102+, Safari 15.2+ | Private file system | Not accessible outside browser |
| IndexedDB | All browsers | Structured storage | Async API, not file-like |
| Server Proxy | All browsers | Full capabilities | Requires server infrastructure |

**Recommended:** Hybrid approach
- OPFS for temporary/scratch files
- File System Access API for local projects (Chrome/Edge)
- Server proxy for Safari/Firefox
- IndexedDB for persistence

### 3. LSP Servers

**Desktop:** Spawn native processes

**Web:** No subprocess support

**Options:**

#### Option A: WASM LSP Servers
```rust
// Compile LSPs to WASM, run in Web Workers

// Pros:
// - No server required
// - Works offline
// - Lower latency

// Cons:
// - Binary size (rust-analyzer is ~40MB)
// - Memory constraints
// - Not all LSPs compile to WASM
```

**Feasibility matrix:**

| LSP | WASM Viability | Notes |
|-----|---------------|-------|
| rust-analyzer | ⚠️ Possible | Very large binary |
| typescript-language-server | ✅ Good | Node.js needed |
| python-language-server | ⚠️ Difficult | Native dependencies |
| gopls | ✅ Good | Go compiles to WASM |
| clangd | ❌ Hard | Large, LLVM dependencies |

#### Option B: Server-Side Proxy
```
Browser <-> WebSocket <-> Zed Server <-> LSP Process
```

**Pros:**
- ✅ All LSPs work
- ✅ No WASM compilation needed
- ✅ Smaller client bundle

**Cons:**
- ❌ Requires server infrastructure
- ❌ Network latency
- ❌ Doesn't work offline

**Recommended:** Start with server proxy, add WASM for popular languages later.

### 4. Performance

**Challenge:** Maintain 60fps in browser environment

**Strategies:**

1. **WebGPU over WebGL2**
   - 2-3x better performance
   - Lower CPU overhead
   - Compute shader support

2. **OffscreenCanvas**
   ```javascript
   // Render in Web Worker, avoid main thread blocking
   const offscreen = canvas.transferControlToOffscreen();
   worker.postMessage({ canvas: offscreen }, [offscreen]);
   ```

3. **WASM SIMD**
   ```rust
   #[cfg(target_feature = "simd128")]
   // Use WASM SIMD for text shaping, syntax highlighting
   ```

4. **Code Splitting**
   ```rust
   // Lazy-load language parsers, themes
   // Initial bundle: ~2MB
   // Full bundle: ~10MB
   ```

5. **Shared Memory**
   ```javascript
   // SharedArrayBuffer for worker communication
   // Requires COOP/COEP headers
   Cross-Origin-Opener-Policy: same-origin
   Cross-Origin-Embedder-Policy: require-corp
   ```

### 5. Binary Size

**Desktop binary:** ~50MB (includes everything)

**Web target:** Must be <5MB for acceptable load time

**Optimization strategies:**

```toml
# Cargo.toml
[profile.release]
opt-level = "z"  # Optimize for size
lto = true       # Link-time optimization
codegen-units = 1
panic = "abort"
strip = true
```

```bash
# Additional tools
wasm-opt -Oz output.wasm -o optimized.wasm  # ~30% reduction
wasm-snip output.wasm -o snipped.wasm       # Remove panics
```

**Expected sizes:**
- Core editor: ~2MB
- Language support: ~500KB per language (lazy-loaded)
- Themes: ~50KB (lazy-loaded)
- Total initial: ~3MB (gzipped: ~1MB)

### 6. Multi-threading

**Desktop:** Use all CPU cores freely

**Web:** Limited to Web Workers + SharedArrayBuffer

**Considerations:**

```rust
// Desktop
rayon::scope(|s| {
    for chunk in chunks {
        s.spawn(|_| process(chunk));
    }
});

// Web (requires COOP/COEP)
#[cfg(target_arch = "wasm32")]
{
    // Spawn Web Workers manually
    // Limited to navigator.hardwareConcurrency (usually 4-8)
}

// Fallback
#[cfg(all(target_arch = "wasm32", not(feature = "shared-memory")))]
{
    // Single-threaded
    for chunk in chunks {
        process(chunk);
    }
}
```

### 7. Offline Support

**Requirements:**
- Service Worker for asset caching
- IndexedDB for project data
- Background Sync API for collaboration

```javascript
// service-worker.js
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open('zed-v1').then((cache) => {
      return cache.addAll([
        '/zed.wasm',
        '/zed.js',
        '/styles.css',
        // ... other assets
      ]);
    })
  );
});
```

---

## Comparison with Other Editors

### VS Code Web

**Architecture:**
- Electron app (native) shares 90% code with web version
- Web version uses standard DOM + Monaco editor (Canvas-based)
- LSP runs server-side (GitHub Codespaces) or in WASM (limited)
- File system via server proxy

**Lessons for Zed:**
- ✅ Server proxy for LSP is acceptable UX
- ✅ Users tolerate some latency for web convenience
- ✅ Progressive enhancement works (basic → full features)

### Lapce

**Architecture:**
- Written in Rust with [Floem UI framework](https://github.com/lapce/floem)
- Uses wgpu for rendering (Metal/Vulkan/OpenGL)
- WASM used for **plugins only**, not web version

**Status:** No web version planned (as of 2025)

**Lessons for Zed:**
- wgpu is viable for high-performance Rust editors
- WASM plugin system could work for Zed extensions

### egui + eframe

**Architecture:**
- Immediate mode GUI
- wgpu backend for native + web
- Excellent WASM support

**Web Examples:** [egui.rs/#demo](https://www.egui.rs/#demo)

**Lessons for Zed:**
- wgpu + WASM is proven at scale
- Binary sizes can be kept small (<1MB)
- WebGPU performance is close to native

---

## Recommended Approach

### Short-term (If starting today)

**Recommendation: Path B (Production Web Version) + Option 3 (Hybrid Blade/wgpu)**

**Rationale:**
1. Keep Blade for native (proven, optimized)
2. Use wgpu for web (proven WASM story)
3. Isolate risk to web platform only
4. Learn before committing to full wgpu migration

**Roadmap:**

**Q1 2026 (Months 1-3): Foundation**
- [ ] Set up wasm32 build target
- [ ] Integrate wgpu (web target only)
- [ ] Create `platform/web.rs` (~1000 lines)
- [ ] Basic rendering in Chrome/Firefox
- [ ] Deploy preview to `web.zed.dev`

**Q2 2026 (Months 4-6): Core Editing**
- [ ] Input handling (keyboard, mouse, multi-touch)
- [ ] Text editing with undo/redo
- [ ] Syntax highlighting (tree-sitter WASM)
- [ ] File explorer (IndexedDB)
- [ ] Settings persistence

**Q3 2026 (Months 7-9): Intelligence**
- [ ] LSP server proxy architecture
- [ ] Autocomplete
- [ ] Diagnostics
- [ ] Git integration (libgit2 WASM)
- [ ] File System Access API (Chrome/Edge)

**Q4 2026 (Months 10-12): Beta Launch**
- [ ] Collaboration (WebRTC already works)
- [ ] Extensions (WASM subset)
- [ ] Performance optimization (<50ms latency)
- [ ] Mobile responsive design
- [ ] Public beta launch

**2027: Refinement**
- [ ] Debugger integration
- [ ] WASM LSP servers for popular languages
- [ ] Offline mode (Service Worker)
- [ ] PWA installation
- [ ] Consider full wgpu migration for native

### Long-term Vision

**If WebGPU adoption continues:**
- Consider full migration to wgpu (2027-2028)
- Benefits: single rendering backend, easier maintenance
- Risk: carefully benchmark native performance

**If Blade gains WebGPU:**
- Support upstream effort
- Could keep Blade ecosystem unified

---

## Appendix A: Code Examples

### Web Platform Implementation Skeleton

```rust
// crates/gpui/src/platform/web/platform.rs

use wasm_bindgen::prelude::*;
use web_sys::{window, HtmlCanvasElement, Performance};

pub struct WebPlatform {
    executor: WebExecutor,
    text_system: Arc<WebTextSystem>,
    renderer: WgpuRenderer,
}

impl Platform for WebPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.executor.background()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.executor.foreground()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        // Call immediately - browser already running
        on_finish_launching();

        // Start requestAnimationFrame loop
        self.start_render_loop();
    }

    fn quit(&self) {
        // Close browser tab
        window().unwrap().close().ok();
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![Rc::new(WebDisplay::primary())]
    }

    // ... implement remaining methods
}

impl WebPlatform {
    fn start_render_loop(&self) {
        let closure = Closure::wrap(Box::new(move |_time: f64| {
            // Render frame
            // Request next frame
            request_animation_frame(&closure);
        }) as Box<dyn FnMut(f64)>);

        request_animation_frame(&closure);
        closure.forget(); // Leak to keep alive
    }
}
```

### File System Abstraction

```rust
// crates/fs/src/web_fs.rs

use rexie::{Rexie, TransactionMode}; // IndexedDB wrapper

pub struct WebFs {
    db: Rexie,
    opfs: Option<FileSystemDirectoryHandle>,
}

#[async_trait::async_trait]
impl Fs for WebFs {
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        // Try OPFS first (if available)
        if let Some(opfs) = &self.opfs {
            if let Ok(data) = self.read_from_opfs(opfs, path).await {
                return Ok(data);
            }
        }

        // Fallback to IndexedDB
        self.read_from_indexeddb(path).await
    }

    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        // Write to both for redundancy
        if let Some(opfs) = &self.opfs {
            self.write_to_opfs(opfs, path, data).await?;
        }
        self.write_to_indexeddb(path, data).await
    }

    async fn watch(&self, path: &Path) -> Result<Pin<Box<dyn Stream<Item = PathEvent>>>> {
        // Use Broadcast Channel API for cross-tab coordination
        let channel = BroadcastChannel::new("fs-events")?;

        // Return stream that listens to channel
        Ok(Box::pin(stream::unfold(channel, |channel| async move {
            let event = channel.recv().await.ok()?;
            Some((event, channel))
        })))
    }
}
```

---

## Appendix B: Performance Benchmarks

### Expected Performance (Rough Estimates)

| Metric | Desktop (Native) | Web (WebGPU) | Web (WebGL2) |
|--------|------------------|--------------|--------------|
| Startup time | 200ms | 800ms | 1200ms |
| Frame time (60fps) | 8ms | 12ms | 16ms |
| Input latency | 1-2ms | 5-10ms | 10-15ms |
| File open (10K lines) | 50ms | 100ms | 150ms |
| Syntax highlighting | 20ms | 40ms | 80ms |
| LSP response | 100ms | 300ms* | 300ms* |
| Memory usage | 150MB | 200MB | 250MB |

\* Includes network latency for server proxy

### WASM Bundle Size Projections

```
Initial load:
- zed.wasm: 2.5MB (gzipped: 800KB)
- zed.js: 200KB (gzipped: 60KB)
- fonts: 400KB (gzipped: 300KB)
Total: 3.1MB (gzipped: 1.16MB)

Lazy-loaded:
- Rust language support: 600KB
- TypeScript language support: 500KB
- Python language support: 450KB
(per language, loaded on demand)

Full bundle: ~8MB (gzipped: ~3MB)
```

---

## Appendix C: Alternative Approaches (Not Recommended)

### ❌ Electron-based Web Version

**Approach:** Ship Electron as a web app (à la Figma's "desktop app")

**Why not:**
- Defeats the purpose (massive download)
- No real web benefits
- Licensing complexity

### ❌ Rewrite in TypeScript

**Approach:** Port editor core to TypeScript for better web compatibility

**Why not:**
- Loses Rust's performance and safety
- Massive engineering effort (2+ years)
- Abandons core value proposition

### ❌ Canvas 2D Rendering

**Approach:** Skip WebGPU/WebGL, use Canvas 2D API

**Why not:**
- Too slow for GPU-accelerated UI
- No shader support
- Can't achieve 60fps with effects

### ❌ Server-Side Rendering (SSR)

**Approach:** Render on server, stream pixels to browser

**Why not:**
- Enormous bandwidth
- Latency makes editing unusable
- Doesn't work offline

---

## References

### Official Documentation

- [Zed Roadmap](https://zed.dev/roadmap) - Official product roadmap
- [Zed FAQ](https://zed.dev/faq) - Frequently asked questions
- [GPUI Documentation](https://gpui.rs) - UI framework docs
- [Zed System Requirements](https://zed.dev/docs/system-requirements) - Platform requirements

### GitHub Issues & Discussions

- [Issue #5391: Platform Support](https://github.com/zed-industries/zed/issues/5391) - Main tracking issue
- [Discussion #26195: Web Support](https://github.com/zed-industries/zed/discussions/26195) - Community discussion
- [Issue #7940: GPU Device Loss](https://github.com/zed-industries/zed/issues/7940) - GPU error handling
- [Issue #12039: iOS/Android Port](https://github.com/zed-industries/zed/issues/12039) - Mobile discussion

### Graphics Libraries

- [Blade Graphics](https://github.com/kvark/blade) - Current rendering backend
- [Blade Documentation](https://lib.rs/crates/blade-graphics) - API documentation
- [wgpu](https://github.com/gfx-rs/wgpu) - Alternative graphics library
- [wgpu.rs](https://wgpu.rs/) - wgpu website and examples

### WebGPU Resources

- [WebGPU Browser Support](https://web.dev/blog/webgpu-supported-major-browsers) - Browser compatibility
- [WebGPU Specification](https://www.w3.org/TR/webgpu/) - W3C standard
- [WebGPU on MDN](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API) - Developer documentation
- [AI in Browser with WebGPU (2025 Guide)](https://aicompetence.org/ai-in-browser-with-webgpu/) - WebGPU capabilities

### Rust GUI Ecosystem

- [egui](https://github.com/emilk/egui) - Immediate mode GUI with web support
- [iced](https://iced.rs/) - Elm-inspired GUI framework
- [Rust GUI Libraries Comparison (2025)](https://an4t.com/rust-gui-libraries-compared/) - egui vs iced vs others
- [Lapce Editor](https://github.com/lapce/lapce) - Rust code editor using Floem + wgpu
- [Tauri Performance Comparison](http://lukaskalbertodt.github.io/2023/02/03/tauri-iced-egui-performance-comparison.html) - Framework benchmarks

### Similar Projects

- [VS Code Architecture](https://code.visualstudio.com/blogs/2022/11/28/vscode-sandbox) - Electron + web architecture
- [VS Code Web](https://vscode.dev) - Browser-based VS Code
- [Lapce Documentation](https://docs.lapce.dev/) - Rust editor using wgpu

### Technical Articles

- [Point of WebGPU on Native](http://kvark.github.io/web/gpu/native/2020/05/03/point-of-webgpu-native.html) - By kvark (Blade author)
- [WASM + WebGL Tutorial](https://rust-tutorials.github.io/triangle-from-scratch/web_stuff/web_gl_with_bare_wasm.html) - WebGL from Rust
- [WebAssembly and WebGPU for Web AI](https://developer.chrome.com/blog/io24-webassembly-webgpu-1) - Chrome developers blog

### Browser APIs

- [File System Access API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_Access_API) - Local file access
- [Origin Private File System](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system) - Private storage
- [Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API) - Threading
- [WebRTC](https://webrtc.org/) - Real-time communication (already used)

---

## Changelog

- **2025-12-12:** Initial version created based on codebase analysis and research
- **Future:** This document should be updated as:
  - WebGPU browser support evolves
  - Blade gains new backends
  - Zed's architecture changes
  - Community feedback arrives

---

## Contributing

This is a living document. If you notice outdated information or want to contribute research:

1. Check current state of dependencies (Blade, wgpu versions)
2. Test browser compatibility
3. Submit corrections via PR to `docs/zed-web-support-architecture.md`
4. Tag with `documentation` + `platform: web`

For implementation discussions, start a thread in [GitHub Discussions](https://github.com/zed-industries/zed/discussions).

---

**Last updated:** December 12, 2025
**Authors:** Community (see git history)
**License:** Same as Zed (Apache 2.0)
