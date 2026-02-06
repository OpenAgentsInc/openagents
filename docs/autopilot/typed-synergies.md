# Typed and OpenAgents Autopilot / Effect / DSE: Synergies and Learnings

This doc explores **Typed** (`~/code/typed`, [tylors/typed](https://github.com/tylors/typed))—an Effect-native toolkit for full-stack web applications—and how it aligns with our **Autopilot**, **Effect**, **Effuse**, and **DSE** approach. It summarizes what we can learn and where shared patterns or adoption could add value.

**Typed in one sentence:** Effect-first full-stack web toolkit: **html** tagged templates with **Fx** (push-based reactivity), **SSR + hydration**, type-safe **routing** and **HTTP API** (OpenAPI, client derivation), **DOM** and **context** services, built to run everywhere (browser, Node, workers) and to be tested by default.

---

## 1. Typed recap (relevant bits)

### 1.1 Core and web framework

- **@typed/core:** Main entry; re-exports and Node integration.
- **@typed/fx:** Push-based reactive programming: `RefSubject` for state, Fx streams for events, composable operators. Models browser behavior declaratively.
- **@typed/context:** Context-aware Effect data structures (Queue, Cache, Pool, Request/RequestResolver, Repository/Model).
- **@typed/template:** HTML via tagged template literals (`html\`...\``). Templates are **`Fx<RenderEvent | null>`**—reactive streams. Supports:
  - **Server:** `serverLayer`, `renderToHtmlString(fx)` for SSR.
  - **Client:** `renderLayer(window)`, `renderToLayer` for DOM; **hydration** via `hydrate` / `hydrateToLayer`.
  - Directives, type-safe event handlers, `RenderQueue` (sync / mixed).
- **@typed/dom:** DOM services (Window, Document, Element, Storage, History, Navigator).
- **@typed/router**, **@typed/route**, **@typed/navigation:** Type-safe routing (AST-based route parsing, params, guards), navigation service (memory for tests, window for browser).

### 1.2 Server and client

- **@typed/server:** Type-safe HTTP API: `HttpApi`, `HttpApiGroup`, endpoints with annotations. OpenAPI spec generation; **client derivation** (from `@effect/platform/HttpApiClient`). Mocks via `@effect/schema` + fast-check. Can serve template-rendered content.
- **@typed/vite-plugin:** Vite integration for Typed apps (e.g. simple-fullstack uses Vavite for isomorphic SSR + client).

### 1.3 Utilities and testing

- **@typed/async-data:** AsyncData (loading / success / failure), progress, schema integration.
- **@typed/id**, **@typed/path**, **@typed/decoder**, **@typed/guard**, **@typed/wire:** IDs, path DSL, decoding, guards, wire format.
- **@typed/ui:** Link, hyperscript helpers, hooks (useClickAway, usePagination).
- **@typed/template/Test**, **@typed/template/Vitest:** Template testing (HTML, DOM, hydration); Vitest + Effect resources for tests.
- **@typed/storybook:** Storybook renderer for Typed components.

Runs everywhere: client, SSR, static, Web Workers, Service Workers. Heavy emphasis on type safety, dependency inversion, and long-term maintainability over extreme performance.

---

## 2. Alignment with our constraints and goals

| Our constraint / goal | Typed angle |
|-----------------------|-------------|
| **Effect everywhere** (web app, workers) | Typed is Effect-native: Layers, Context, Fx, type-safe errors. Same runtime and patterns we use. |
| **Effuse: Effect-native UI** | Typed template is also Effect + html tagged literals, but **reactive** (Fx + RefSubject in template). Effuse is one-shot render (payload in, DOM swap); Typed is stream-of-render-events with first-class hydration. |
| **Type-safe API + client** (DELEGATION RPC) | Typed server: HttpApi/HttpApiGroup, OpenAPI, client from Effect Platform. Same idea: define API in types, derive client; we could adopt or mirror. |
| **Routing** | We use TanStack Router. Typed has type-safe router/route/navigation with Effect; if we ever want Effect-native routing, Typed is an option; otherwise we keep TanStack and learn from their route/param typing. |
| **SSR + hydration** | Typed has serverLayer + renderToHtmlString and client hydrate. We use TanStack Start for SSR and Effuse on client; we could learn hydration patterns or adopt Typed template for selected routes. |
| **Testability** | Typed: template Test, Vitest, memory navigation. We can adopt similar patterns for testing Effuse-rendered UI (render to string/DOM, assert). |
| **No containers** (Autopilot) | Typed is a front-end/Backend-for-Frontend toolkit; no opinion on where the agent runs. Fits our Workers + DO setup. |

So Typed is **highly aligned** with our stack: same Effect ecosystem, same “type-safe end-to-end” philosophy. The main difference is **template model** (Typed = Fx-based reactive + hydration; us = Effuse one-shot + React shell) and **routing** (Typed vs TanStack). We can learn from Typed without replacing our choices.

---

## 3. Synergies

### 3.1 Template: Fx vs one-shot (Effuse)

**Typed:** Template is `Fx<RenderEvent | null>`. You embed `RefSubject` and event handlers (e.g. `onclick=${RefSubject.increment(count)}`) in the template; the stream drives DOM updates. SSR = `renderToHtmlString(fx)`; client = `renderLayer(window)`; hydration = `hydrate(rendered)`.

**Us:** Effuse = `html\`...\`` → `TemplateResult` → `dom.render(container, content)` (one-shot). React owns loaders and shell; we pass payload into Effuse and re-run on changes.

**Synergy:**
- **Keep Effuse one-shot** for simplicity and React coexistence; we don’t need to adopt Fx in the template today.
- **Learn from Typed:** (1) **Directive system** for dynamic behavior in templates. (2) **Type-safe event handlers** (errors in handlers surface in types; we could add handler types to Effuse). (3) **RenderQueue** (sync vs async) if we ever need prioritized or batched updates. (4) **Hydration story** if we ever do Effect-native SSR of Effuse: Typed’s server layer + static layer + hydrate is a reference.
- **Optional future:** If we wanted “reactive Effuse,” we could introduce a small Fx (or Effect Stream) that re-renders on payload changes, similar to Typed’s model, without switching to full Typed template.

**Learning:** Tagged html + Effect is shared; the split is “reactive stream (Typed) vs one-shot (Effuse).” For our current hybrid React+Effuse, one-shot is the right fit; Typed’s patterns are there when we need stricter hydration or reactivity.

### 3.2 Type-safe API and client (HttpApi vs RPC)

Typed server: **HttpApi** / **HttpApiGroup** with typed endpoints; OpenAPI generation; client from `@effect/platform/HttpApiClient`. Our **DELEGATION-full-effect-integration** doc mentions RPC and type-safe client; Typed’s approach is the same idea on HTTP.

**Synergy:**
- Define API surface as types/groups; generate OpenAPI; derive client. We could use **@typed/server** for a BFF or internal API, or mirror its patterns with Effect Platform’s HttpApi in our own code.
- **Mocks:** Typed uses `@effect/schema` + fast-check for mock server/client. We can do the same for our Convex/Worker APIs when writing tests.

**Learning:** “Single source of truth for API shape → OpenAPI → type-safe client” is a pattern we want; Typed and Effect Platform already implement it.

### 3.3 Router and navigation

Typed: **@typed/route** (AST, path/query schemas), **@typed/router** (matching, current route, guards), **@typed/navigation** (memory for tests, window for browser). All Effect services.

We use **TanStack Router** (file-based, loaders, search params). No need to switch.

**Synergy:**
- **Testing:** Memory-based navigation in Typed lets tests drive “navigate to X” without a real browser. We could add a small navigation abstraction in tests that sets route/params and then runs Effuse or loaders.
- **Type-safe params:** Typed’s Route.* types (Params, Query, etc.) are a reminder to keep our route params and search params typed (e.g. Zod or Schema in loaders).

**Learning:** Effect-native routing is optional for us; the testing and typing patterns (memory nav, typed params) are reusable.

### 3.4 Context and layers

Typed uses **Context** and **Layer** for all services (DOM, RenderTemplate, RenderContext, Navigation, Router, etc.). We do the same in Effect code (runtime, tools, Convex, etc.).

**Synergy:** No conflict. We can align naming and layering (e.g. “template render context” vs “Effuse mount context”) when we document our own layers. Typed’s **@typed/context** (context-aware Queue, Cache, etc.) is a reminder to use Effect’s data structures with our services where it helps.

### 3.5 Testing (template Test, Vitest)

Typed provides **@typed/template/Test** (HTML, DOM, hydration) and **@typed/template/Vitest** (Effect resources for tests). We render Effuse in Jest/Vitest and in the browser; we don’t have a formal “template test” helper.

**Synergy:**
- Add a small test helper that renders an Effuse template to a string or to a mounted div and asserts on structure or text. Same idea as Typed’s Test: isolate template behavior without full app.
- Use **Effect + Vitest** (e.g. `@effect/vitest`) in our test setup where we run Effect programs (e.g. Convex or API layers).

**Learning:** “Render template in test env, assert” is a shared goal; we can keep it minimal and still benefit.

### 3.6 Async data and loading states

**@typed/async-data:** AsyncData (loading / success / failure), progress, schema. We handle loading in React (loaders, Suspense, local state).

**Synergy:** If we move more data-fetching into Effect (e.g. loaders as Effect), an AsyncData-like type (or Typed’s) could standardize “loading / success / error” in the type system and map to React or Effuse. Optional; useful when we formalize loader contracts.

---

## 4. What we should learn

- **Template:** Directives, type-safe event handlers, RenderQueue, and hydration flow as a reference for any future “Effuse SSR” or “reactive Effuse” work.
- **API:** HttpApi/HttpApiGroup + OpenAPI + client derivation as the pattern for type-safe BFF or internal APIs; mocks via schema + fast-check.
- **Testing:** Template test utilities and memory navigation for tests; Effect + Vitest for any Effect-heavy tests.
- **Layers:** Continue using Context/Layer consistently; align with Typed’s service boundaries when we document our own.

---

## 5. Optional next steps

- **Short term:** Add a link to this doc from `docs/autopilot/spec.md`. No code change required.
- **When we add type-safe API client:** Review Typed server and Effect Platform HttpApiClient; adopt or mirror for our RPC/BFF.
- **When we add Effuse tests:** Implement a small “render Effuse to string/DOM and assert” helper; take inspiration from @typed/template/Test.
- **If we ever do Effect-native SSR for Effuse:** Study Typed’s serverLayer, renderToHtmlString, and hydrate flow; decide whether to adopt Typed template for those routes or implement a minimal equivalent for Effuse.

---

## 6. References

- **Typed repo:** `~/code/typed` (or https://github.com/tylors/typed)
- **Typed docs (in repo):** `readme.md`, `CLAUDE.md`, `packages-summary.md`, `basics.md`; `docs/` (per-package API); `examples/counter`, `examples/simple-fullstack`, `examples/simple-ssr`, `examples/todomvc`, `examples/fx-tracing`
- **Our docs:** `docs/autopilot/effuse-conversion-apps-web.md`, `docs/autopilot/DELEGATION-full-effect-integration.md`, `docs/autopilot/tanstack-start-effect-comparison.md`
