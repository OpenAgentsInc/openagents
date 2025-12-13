# wgpui: The Case for Owning the Pixel

**2025-12-13 13:45**

---

## The Core Thesis

Every major platform advancement in software has come from teams that refused to trust the abstraction beneath them. Figma didn't trust the browser's rendering. Discord didn't trust Electron's performance. Zed didn't trust any existing editor framework. The pattern is clear: **when your product's core experience is the interface itself, you must own the rendering layer.**

The question isn't whether wgpu can deliver GPU-accelerated rendering across web, mobile, and desktop. It can. The question is whether we have the conviction to pay the tax that comes with that ownership.

---

## Why the Distrust is Justified

### The Framework Graveyard

Look at the carnage:

- **React Native**: Perpetually "almost native" for a decade. Bridge overhead. Layout engine limitations. Two-render-tree synchronization bugs.
- **Flutter**: Google's abandoned child pattern emerging. Impeller still not solving the jank. Platform integration always fighting the framework.
- **Electron**: 300MB for "Hello World." Memory bloat is architectural, not accidental.
- **Capacitor/Ionic**: Web views all the way down. You're one WebView2 bug from catastrophe.
- **Tauri**: Better than Electron, but you're still painting to a web view. The abstraction leak is inevitable.

The problem isn't that these frameworks are bad. The problem is they're **optimized for someone else's use case**. They solve the 80% problem and leave you bleeding on the remaining 20%.

### What "Tight Control" Actually Means

Tight control isn't about NIH syndrome. It's about:

1. **Knowing exactly what GPU commands are issued per frame.** No hidden allocations. No surprise shader compilations. No mystery stalls.

2. **Identical pixel output across platforms.** Not "mostly the same." Not "platform-appropriate." Identical.

3. **Sub-millisecond input-to-pixel latency.** The render loop is your loop. No framework scheduler between intention and expression.

4. **Text rendering that doesn't suck.** cosmic-text + glyph atlas = you own the pipeline. No platform text API quirks.

5. **Zero dependency on platform UI frameworks.** No UIKit. No AppKit. No Android Views. No DOM boxing you in.

---

## Why wgpu

### The Technical Case

wgpu is the only serious cross-platform GPU abstraction that:

- **Compiles to native** (Vulkan, Metal, DX12) *and* WebGPU/WebGL
- **Has a safe Rust API** that prevents most GPU programming footguns
- **Is actively maintained** by Mozilla/gfx-rs with a clear trajectory
- **Supports compute shaders** for when you need them (and you will)

The backend story:
```
wgpu
├── Vulkan (Linux, Android, Windows)
├── Metal (macOS, iOS)
├── DX12 (Windows)
├── WebGPU (Chrome, Firefox, Safari)
└── WebGL2 (fallback for older browsers)
```

This is the narrowest waist possible. One API. One shader language (WGSL). Every platform.

### The Ecosystem Reality

wgpu isn't a toy. It powers:
- **Bevy** (growing game engine ecosystem)
- **Dioxus experimental renderer** (they're trying the same thing)
- **wgpu-rs** examples demonstrate the full API surface

The tooling is mature enough. The documentation is adequate. The community is responsive.

---

## What wgpui Should Be

### Not a Framework. A Rendering Engine.

wgpui should be opinionated about **pixels**, not about **architecture**.

It should answer:
- How do I draw a quad with rounded corners?
- How do I render text efficiently?
- How do I compose layers with proper alpha?
- How do I handle DPI and resize?
- How do I integrate input events?

It should *not* answer:
- How should I structure my application state?
- What reactive model should I use?
- How should I route between views?
- What component abstraction do I need?

The analogy: wgpui is to UI what wgpu is to graphics. Low-level. Correct. Composable. Not trying to be everything.

### The Minimal Surface

```rust
// Scene primitives
pub struct Quad { bounds, background, border, corner_radii }
pub struct TextRun { glyphs, origin, color }
pub struct Path { commands, fill, stroke }

// Rendering
pub trait Renderer {
    fn begin_frame(&mut self, size: Size, scale: f32);
    fn draw_quad(&mut self, quad: &Quad);
    fn draw_text(&mut self, text: &TextRun);
    fn draw_path(&mut self, path: &Path);
    fn end_frame(&mut self);
}

// Platform
pub trait Platform {
    fn create_window(&self, config: WindowConfig) -> Window;
    fn run(&self, handler: impl EventHandler);
}

// Text
pub struct TextSystem {
    fn shape(&self, text: &str, style: &TextStyle) -> ShapedText;
    fn measure(&self, text: &str, style: &TextStyle) -> Size;
}
```

That's it. Everything else is composition.

### The Layout Question

Layout is where the purity breaks. Two options:

**Option A: Taffy (CSS Flexbox/Grid)**
- Pro: Familiar mental model. Handles complex cases.
- Con: Another dependency. CSS semantics don't always map cleanly.

**Option B: Custom layout**
- Pro: Tight control. Can optimize for our exact use cases.
- Con: Significant implementation effort. Edge case city.

Recommendation: **Start with Taffy, but keep the abstraction clean enough to swap.**

The layout engine should be injected, not baked in:

```rust
pub trait LayoutEngine {
    fn measure(&mut self, node: LayoutNode, constraints: Constraints) -> Size;
    fn position(&mut self, node: LayoutNode, bounds: Bounds);
}

pub use taffy_layout::TaffyEngine as DefaultLayoutEngine;
```

---

## What We Already Have

From the exploration over the past few days:

1. **Proof of concept renderer** (`crates/openagents-web`): Working wgpu WASM demo with quads, colors, animation, resize handling.

2. **Element system design** (from the 1142-wgpu-setup.md log): Element trait with `request_layout` and `paint` phases. Taffy integration. Theme system.

3. **Text rendering pipeline**: cosmic-text integration with glyph atlas. Shaping works. Measurement works.

4. **Web platform**: WebPlatform struct that initializes wgpu on a canvas, handles events, runs the animation loop.

What's missing:
- Native platform (winit integration)
- Mobile platform (Android/iOS surface creation)
- Input handling (keyboard, mouse, touch, IME)
- Focus management
- Accessibility (this is the hard one)

---

## The Honest Tax Bill

If we build wgpui properly, we're signing up for:

### Must Have (Year 1)
- [ ] Quad rendering with rounded corners, borders, shadows
- [ ] Text rendering with shaping, layout, selection, cursor
- [ ] Image/texture rendering
- [ ] Input events (mouse, keyboard, touch)
- [ ] Platform integration (web, macOS, Linux, Windows)
- [ ] Basic layout (Taffy)
- [ ] 60fps on all targets

### Should Have (Year 1-2)
- [ ] IME support (CJK input, dead keys, compose)
- [ ] Mobile targets (iOS, Android)
- [ ] Path rendering (custom shapes, icons)
- [ ] Animation primitives
- [ ] Clipping and masking
- [ ] Basic accessibility (focus rings, labels)

### Nice to Have (Year 2+)
- [ ] Full accessibility tree (screen readers)
- [ ] Blur/effects
- [ ] 3D transforms
- [ ] Custom shaders
- [ ] Video playback

### The Stuff That's Actually Hard
1. **Text selection and cursor placement.** Unicode is a nightmare. Grapheme clusters. RTL. Bidirectional text.
2. **IME integration.** Every platform does it differently. Getting this wrong makes your app unusable in Asia.
3. **Accessibility.** A custom renderer has no semantic layer. You have to build it.
4. **Platform integration.** File dialogs. Clipboard. Drag and drop. Context menus. Native feel.

---

## The Strategic Question

The real question from the 1341-dioxus-considerations.md log was correct:

> Is UI engine a strategic moat for you, or a delivery vehicle?

For Coder (the coding agent platform), the moat is **workflow-as-code** and **multi-agent execution**. Not pixels.

But.

The Figma argument is also correct. Figma's moat became the rendering engine because **the product is the canvas**.

For Coder:
- The **chat thread** is a canvas (messages, artifacts, inline code, diffs)
- The **run timeline** is a canvas (parallel agent visualization, state flow)
- The **terminal** is a canvas (ansi rendering, selection, scrollback)
- The **diff viewer** is a canvas (side-by-side, inline, change annotations)

If those surfaces feel mediocre, the product feels mediocre. If those surfaces feel Figma-level responsive, the product feels elite.

---

## The Hybrid Path Forward

The pragmatic answer:

### Short Term (Next 30 days)
1. Keep Dioxus for the app shell (auth, settings, project list, routing)
2. Embed wgpui canvases for performance-critical surfaces
3. Build a clean bridge: Dioxus signals -> wgpui scene updates

### Medium Term (60-90 days)
1. Prove out the critical surfaces (chat thread, terminal)
2. Add native platform support (winit + wgpu)
3. Validate mobile (iOS/Android feasibility study)

### Long Term (6+ months)
1. Evaluate whether to expand wgpui scope
2. Contribute back to wgpu ecosystem
3. Open source wgpui if it becomes genuinely useful

---

## The Conviction Check

Building wgpui makes sense if and only if:

1. **We believe the product will live or die on interaction quality.** Latency matters. Smoothness matters. Consistency matters.

2. **We're willing to invest 6-12 months of sustained effort** on rendering infrastructure before it pays back.

3. **We have the Rust GPU talent** or can hire it. This is specialized work.

4. **We're prepared to maintain it forever.** The alternative is perpetual dependency on someone else's priorities.

If all four are true, build it. Own the pixel.

If any are false, use Dioxus everywhere and invest in what actually differentiates the product.

---

## My Actual Recommendation

Build wgpui. Here's why:

1. **The foundation already exists.** The openagents-web proof of concept works. The shader is correct. The architecture is sound.

2. **Dioxus is the escape hatch.** If wgpui takes longer than expected, Dioxus can carry the product. We're not betting the company.

3. **The moat compounds.** Every surface built with wgpui is a surface no competitor can replicate without the same investment.

4. **Mobile is coming.** Every serious product needs mobile. wgpu on iOS/Android is proven. Dioxus mobile is not.

5. **The timing is right.** WebGPU is shipping in all major browsers. wgpu is mature. The ecosystem is ready.

The path:
- wgpui = the rendering engine (quads, text, input, platform)
- Dioxus = the state management and routing layer (signals, router, server functions)
- Hybrid architecture = Dioxus shell with embedded wgpui canvases

This gives us:
- Web: Dioxus SSR + hydration with wgpui canvas for core surfaces
- Desktop: Same Dioxus architecture with native window
- Mobile: wgpui rendering with platform-native lifecycle (future)

The bet is that tight GPU control on the surfaces that matter will compound into a product that simply *feels* better than anything built on commodity frameworks.

That bet is worth making.

---

*"We choose to go to the Moon in this decade and do the other things, not because they are easy, but because they are hard."* — JFK

The hard thing is owning the pixel. The easy thing is trusting someone else's abstraction. The right thing depends on what you're building.

For a coding agent platform where the interface *is* the product, own the pixel.
