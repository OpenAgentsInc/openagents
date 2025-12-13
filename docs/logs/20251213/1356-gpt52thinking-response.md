> Here's what Opus says. [paste docs/logs/20251213/1345-wgpui-manifesto.md] I agree with its analysis about building WGPUI but thinking we should perhaps apply the same logic to Dioxus. If we need some Taffy/CSS simpler shit than doing everything in WGPUI maybe we should just build that ourselves so it all lives in our codebase (more malleable to coding agents than having them try to read Dioxus docs/codebase). Or if we're going to do that build-it-ourself approach anyway, maybe we should just try to build everything in WGPUI to start. Thoughts?

Yeah — I buy the instinct, but I’d separate **“own the pixels”** from **“own the UI framework.”** They’re different taxes.

If you conflate them, you’ll either:

* rebuild Dioxus *and* wgpui (too much), or
* build everything in wgpui and accidentally sign up for “browser platform engineering” (text/IME/a11y/forms/routing/etc.) when your moat is workflows + multi-agent execution.

Here’s how I’d think about it.

## The real stack has 6 layers you can choose to own

1. **Domain + event model** (agents/runs/artifacts/workflows)
2. **UI state/runtime** (reactivity, scheduling, diffing)
3. **Layout** (flex/grid constraints)
4. **Widgets** (lists, panes, tables, editors, terminal)
5. **Renderer** (DOM vs GPU scene)
6. **Platform glue** (windows, input, IME, accessibility)

**Dioxus** mostly gives you 2 + (some of) 3 + 5 (DOM) + routing.
**wgpui** is basically 5 (+ eventually 6).

Your question is: “If we want maximum malleability for coding agents, should we own 2/3/4 as well?”

## Key observation: “Agent malleability” is mostly about *surface area*

Coding agents don’t need Dioxus internals if **your codebase exposes a small, opinionated UI layer**.

So the highest-leverage move is usually:

* **own your design system + widgets + patterns** (4), and
* treat Dioxus as an implementation detail behind that.

That gets you 90% of the “it lives in our repo, agents can change it” benefit without rebuilding the world.

## Three viable strategies (with infinite resources)

### Strategy A: Dioxus shell + wgpui canvases, but “own the app framework”

This is my favorite for Coder.

* **Keep Dioxus** for shell-y, boring UI: auth, billing, project list, settings, workflow editor forms.
* **Build wgpui surfaces** for the core IDE canvases: terminal, diff viewer, run timeline, graph/trace viewers (maybe later editor).
* **Wrap Dioxus in your own UI kit** so agents rarely touch raw Dioxus patterns:

  * `coder_ui::Button`, `coder_ui::Pane`, `coder_ui::Table`, `coder_ui::RouteShell`, etc.
  * one “blessed” state pattern (signals/store) and one routing pattern
  * strict internal conventions + in-repo docs

Result: you “own” what agents edit day-to-day, while still benefiting from DOM accessibility, forms, input, etc.

This is essentially the “Figma move”: custom renderer for the core surfaces, standard UI around it.

### Strategy B: Fork/Vendor Dioxus and treat it as *your code*

If your main worry is “agents can’t modify an external dependency,” then just remove that concern:

* Fork Dioxus (or vendor the subset you use) into your org and pin it.
* Copy the relevant docs into your repo and annotate with “Coder conventions.”
* You still wrap it behind `coder_ui` so your app doesn’t depend on its API surface directly.

This gives you malleability + control without inventing a new reactive runtime.

### Strategy C: Full wgpui for everything (the “own the universe” path)

If you go all-in on wgpui from day 1, you will absolutely get the tight, consistent IDE feel.

But the tax you’re signing up for isn’t “rendering.”
It’s the entire long tail of platform UI:

* IME (CJK input) across web + native
* accessibility tree
* focus/navigation semantics
* text selection, clipboard, native menus, drag/drop
* form controls (workflow editor is basically forms + tables)

With infinite resources, you can do it — but it’s a deliberate decision to spend a **big chunk** of engineering attention on UI platform parity rather than agent/workflow dominance.

## My take on your “if we build anything ourselves, why not build everything in wgpui?”

Because “build some ourselves” can mean “own the developer-facing surface” (widgets + conventions) without owning “everything below” (reactivity/layout/a11y).

The sweet spot is:

**Own:**

* wgpui renderer + text pipeline for core canvases (terminal/diff/timeline)
* your UI kit + widgets + conventions (agent-editable, in-repo)
* your domain event model (runs/artifacts/workflows)

**Borrow:**

* Dioxus runtime + DOM for shell and forms (or vendor/fork it if you need control)

That gets you:

* elite feel where it matters
* low risk + fast iteration where it doesn’t
* maximum “agents can change it” because your public UI surface is *yours*, not Dioxus

## A concrete north-star boundary

If you want the cleanest separation:

* `ui_kernel` (yours): command bus + app state + “view model” + theming tokens
* `ui_shell_dioxus` (adapter): renders shell screens from `ui_kernel`
* `ui_surface_wgpui` (adapter): renders canvases from `ui_kernel`
* `surfaces/*` (yours): terminal, diff, timeline, trace graph implemented on wgpui primitives
* `widgets/*` (yours): panes/tables/forms used by shell

Then switching Dioxus later is a renderer swap, not a rewrite.

If you tell me which 2 surfaces you want to “feel Figma-level” first (terminal + diff? terminal + timeline?), I’ll give you a 30/60/90-day build order that keeps the architecture honest and avoids accidentally rebuilding a browser before you ship the agent platform.
