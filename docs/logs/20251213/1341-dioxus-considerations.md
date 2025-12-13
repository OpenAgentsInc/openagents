Alright. With “infinite dev resources,” this isn’t really a question of *can you* build a purpose-built Rust UI stack with `web-sys` + `wgpu` (you can) — it’s a question of **what you want to own** as a long-term moat vs what you want to treat as commodity.

Below is the trade space in the way I’d frame it for **Coder (coding agent platform)**.

## What Dioxus buys you (fast, broad, “good enough”)

Dioxus is already aiming at the same “one Rust UI across targets” story:

* Multiple renderers including **web-sys**, webview, **SSR**, “liveview” (server-driven), and an **experimental WGPU renderer**. ([GitHub][1])
* UI model built around **HTML + CSS**, with web being native DOM and desktop/mobile being webview/hybrid approaches. ([Dioxus Labs][2])
* It explicitly warns that **liveview has been deprioritized and may be removed** — relevant if you wanted a server-driven UI. ([Dioxus Labs][3])

So if your goal is: “ship Coder Studio quickly on web + desktop” and spend your differentiation budget on **MechaCoder + workflow-as-code**, Dioxus is very attractive.

Where Dioxus will feel limiting (for your specific product):

* **IDE-class UX** (terminal-heavy, diff views, huge lists, complex focus/IME, extreme performance constraints) often runs into the edges of generic component frameworks.
* If you want a **Zed/VSCode-caliber editor shell** with custom text rendering, layout, and “always 120fps” feel, you’ll end up fighting the abstractions and/or needing to drop down a lot.

## What “roll your own with web-sys + wgpu” really means

You’re proposing *two* very different things that share Rust but not much else:

### A) `web-sys` DOM-first renderer

This is basically: “write a UI framework, but the browser does layout/accessibility.”

Pros:

* Best-in-class **accessibility**, text, selection, IME, scrolling, semantics “for free.”
* CSS layout and responsiveness are solved problems.
* Debuggability is great (devtools).

Cons:

* You are still building a framework: state model, scheduling, diff/patch, event system, async resources, routing, devtools, etc.
* You’ll reinvent 60–80% of what Dioxus already gives you.

If you go this route, the biggest reason is: you want a **purpose-built reactive model + workflow-driven UI** that matches your mental model (e.g., “runs”, “artifacts”, “agents”, “policies”), and you want that model to be your moat.

### B) `wgpu` GPU-first renderer (desktop + wasm)

This is: “Flutter/Zed-style UI engine in Rust.”

Pros:

* Unified rendering across native + web (wgpu runs on Vulkan/Metal/D3D/OpenGL and can run on WebGL2/WebGPU in wasm). ([GitHub][4])
* You can build an IDE shell that feels *insanely* fast and consistent.
* Perfect fit for highly custom UI: terminals, timelines, graph views, animation, shader effects, 3D.

Cons (even with infinite resources, these are *real*):

* You must own **text shaping + font fallback + selection + IME + accessibility**. That’s the hidden multi-year tax.
* You must build (or adopt) a **layout engine**, input system, focus navigation, clipboard, drag/drop, platform integrations.
* Web adds extra constraints: `wgpu` on wasm uses WebGPU/WebGL2 pathways; real-world compatibility and feature selection can be fiddly. ([Docs.rs][5])

If you go GPU-first, you’re effectively building “GPUI-like infrastructure” — which *can* be a moat, but it’s a different company than “agent workflows.”

## The decision for *Coder specifically*

Your stated wedge is:

> Others ship prototypes. We ship **operationalized agent workflows** that plug into Git/CI/deploy and keep working.

That wedge **doesn’t require** owning a whole UI engine. It requires owning:

* multi-agent execution
* workflow-as-code
* artifacts + traceability
* integrations (GitHub, CI, deploy)
* policy + budget controls

So the question becomes: **Is UI engine a strategic moat for you, or a delivery vehicle?**

### If UI is a delivery vehicle (my default recommendation)

* Use **Dioxus** (or similar) for Studio UI now.
* Put your “infinite resources” into making the **agent platform** undeniable.
* Keep the UI architecture modular so you can swap renderer later if needed.

Why: Dioxus already gives you the cross-platform surface area (web-sys/webview/SSR, and even experimental wgpu rendering). ([GitHub][1])
Your moat is not pixels; it’s **workflow repeatability**.

### If UI is a strategic moat (Zed-class product ambition)

Then build a **purpose-built wgpu desktop client** for the IDE shell *and* keep web DOM-based for accessibility and reach.

The “best of both” architecture looks like:

* **One shared UI/state model** (runs, artifacts, panes, routing, commands)
* **Two renderers**

  * Web: DOM renderer (web-sys) for accessibility and fast iteration
  * Desktop: wgpu renderer for terminal/editor performance

This avoids the trap of trying to make GPU-rendered UI also satisfy browser-grade semantics and accessibility on day one.

## A crisp “infinite resources” plan

If you truly have infinite resources, the highest-leverage approach is:

1. **Adopt Dioxus now** to ship Coder Studio fast and validate workflows-as-code.
2. In parallel, build a **next-gen desktop shell** (wgpu) only after you’ve proven which UI primitives matter (terminal, diff, timeline, graph, PR review, etc.).
3. Keep the agent/workflow runtime completely UI-agnostic (events → state → commands), so the UI can be swapped without rewriting the product.

That way:

* You get speed *and* you preserve the option to own the whole UI stack later.
* You don’t bet the company on building text/IME/accessibility before your agent platform wins.

If you tell me which direction you’re leaning (Dioxus-first vs GPU-first), I’ll outline the concrete architecture boundaries (state model, event bus, renderer trait, widget primitives) so you can start implementing without locking yourself in.

[1]: https://github.com/DioxusLabs/dioxus?utm_source=chatgpt.com "DioxusLabs/dioxus: Fullstack app framework for web, ..."
[2]: https://dioxuslabs.com/learn/0.7/essentials/ui/?utm_source=chatgpt.com "Building User Interfaces"
[3]: https://dioxuslabs.com/learn/0.7/beyond/project_structure/?utm_source=chatgpt.com "Project Structure"
[4]: https://github.com/gfx-rs/wgpu?utm_source=chatgpt.com "gfx-rs/wgpu: A cross-platform, safe, pure-Rust graphics API."
[5]: https://docs.rs/wgpu/?utm_source=chatgpt.com "wgpu - Rust"


Exactly — **Figma is the proof** that you can go “hardcore WASM + custom GPU renderer” and still be a **product company**, not “a WASM company.”

Figma used WebAssembly (compiled from C++) and a GPU-based canvas renderer (initially WebGL, now WebGPU) because it was the only way to deliver a smooth “infinite canvas” editing experience in the browser. ([Figma][1]) And they still kept plenty of “regular app UI” around it (they even mention UI pieces in TypeScript/React in their plugin-system writeup). ([Figma][2])

That maps cleanly to **Coder**:

* Your “canvas” isn’t vector shapes.
* Your “canvas” is **IDE-class surfaces**: code editor, terminal, diffs, run timelines, graphs, trace viewers.

## The real lesson from Figma

They didn’t “roll their own everything.” They *owned the parts that matter*:

* **Rendering engine** because it’s the product’s core interaction loop ([Figma][3])
* **Performance pipeline** (they talk about renderer restructuring + WebAssembly work yielding big gains) ([Figma][4])
* **Sandbox/plugin architecture** because extensibility and safety were core ([Figma][2])

Everything else can be “normal UI” as long as the core surface is elite.

## So: Dioxus vs “own framework” isn’t binary

### Option 1: Dioxus everywhere

Great if you want to maximize velocity and minimize “UI engine tax.”

* You’ll ship faster.
* But IDE-grade performance/behavior (text, selection, IME edge cases, massive lists, terminal rendering) will eventually push you into custom components anyway.

### Option 2: Full custom UI framework (web-sys + wgpu as the whole app)

You *can* do it with infinite resources, but it’s a choice to become an “app platform company” internally:

* You will own **text shaping, IME, accessibility, layout, input, focus, clipboard, drag/drop** across web + desktop.
* That’s not “WASM work,” that’s “browser work.” Figma literally describes it as “building a browser inside a browser.” ([Figma][5])

### Option 3 (the Figma move): Hybrid shell + custom surfaces

This is the sweet spot for Coder.

* Use **Dioxus for the app shell**:

  * auth, billing, projects, settings
  * workflow editor (forms + tables)
  * run history, artifacts list, logs list
* Use a **purpose-built wgpu surface** only where it’s your moat:

  * code editor (or at least diff viewer + timeline first)
  * terminal
  * trace/timeline/graph viewer

On web, those wgpu surfaces can be wasm-backed and embedded like a “canvas region,” while the rest remains normal DOM/UI. That’s basically the pattern behind high-performance web apps like Figma (GPU canvas for the core, standard UI around it). ([Figma][1])

## How this supports your “operationalize prototypes” wedge

Your differentiator is **workflow-as-code + multi-agent execution**. The UI only needs to be “owned” where it enables:

* extremely fast inspection of agent runs
* robust diffs and patch queues
* timeline/trace exploration
* terminal/log ergonomics

Everything else should stay commodity so you don’t burn attention on UI plumbing.

## If you want a crisp decision rule

Build custom wgpu surfaces **only** when all 3 are true:

1. It’s used *constantly* (hours/day)
2. It must feel “native fast” (60–120fps, huge docs, heavy interaction)
3. It’s directly tied to your moat (agent workflows, run artifacts, review gates)

Otherwise: Dioxus.

If you tell me which surfaces you consider “core” for Coder v1 (editor? terminal? run timeline? diff viewer?), I’ll propose an architecture boundary that lets you drop wgpu surfaces into a Dioxus shell cleanly (shared state model + command bus + renderer interface) without locking you into either path.

[1]: https://www.figma.com/blog/webassembly-cut-figmas-load-time-by-3x/ "Figma is powered by WebAssembly | Figma Blog"
[2]: https://www.figma.com/blog/how-we-built-the-figma-plugin-system/ "How to build a plugin system on the web and also sleep well at night | Figma Blog"
[3]: https://www.figma.com/blog/figma-rendering-powered-by-webgpu/ "Figma Rendering: Powered by WebGPU | Figma Blog"
[4]: https://www.figma.com/blog/keeping-figma-fast/ "Keeping Figma Fast | Figma Blog"
[5]: https://www.figma.com/blog/building-a-professional-design-tool-on-the-web/ "Building a professional design tool on the web | Figma Blog"
