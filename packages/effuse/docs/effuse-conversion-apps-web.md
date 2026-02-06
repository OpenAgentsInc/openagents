# Effuse Conversion: apps/web UI

This document describes the conversion of the OpenAgents web app (`apps/web`) to render almost all user-facing UI with **Effuse** (Effect-native UI in `packages/effuse`), while keeping React for routing, auth, data loading, and a few shell pieces.

**Effuse** is a lightweight, type-safe UI layer built on Effect TypeScript: HTML templates (`html` tagged literals), a DOM service (render/swap), and optional stateful components. It was restored from the former autopilot-desktop (Tauri) codebase and lives in `packages/effuse` with no Tauri dependency. See `packages/effuse/docs/` for framework docs.

---

## Goals of the conversion

- **Unify UI rendering** on a single, Effect-native abstraction so future desktop or other runtimes can share the same “page” logic.
- **Keep the web app working** with existing stack: TanStack Start, Convex, WorkOS AuthKit, Effect runtime, HUD (DotsBackground).
- **Minimize risk**: Data loading and side effects stay in React/Effect; Effuse is used only for turning data into DOM.

---

## What stayed React (and why)

React is still responsible for:

- **Route loaders** – Auth checks, redirects, loader data (e.g. `signInUrl` for `/login`).
- **Convex / WorkOS** – Providers and hooks.
- **HUD backgrounds** – `DotsGridBackground` and gradient overlays from `@openagentsinc/hud/react`; no Effuse equivalent, so they remain React.
- **EffuseMount** – A small React component that mounts a div, runs an Effect program to fill it, and optionally runs an `onRendered` callback for event delegation.
- **Autopilot chrome** – Sidebar (nav + user menu), Blueprint panel (edit/save/export), and the bottom-right control panel (Export Blueprint, Clear messages, Reset agent). These are stateful and tightly coupled to auth/Effect; they stayed React.

Everything else in the listed routes is rendered by Effuse.

---

## Architecture: EffuseMount and data flow

### EffuseMount

Effuse does not replace the router or the DOM root. Instead, React renders a **container div** and hands it to Effuse:

- **`EffuseMount`** (`apps/web/src/components/EffuseMount.tsx`):
  - Renders a single `<div ref={ref} className={...} />`.
  - In a `useEffect` (with configurable `deps`), runs `run(container)` where `run` is `(container: Element) => Effect.Effect<void>`.
  - The Effect program uses Effuse’s `DomServiceTag` to `render(container, content)` (innerHTML swap).
  - Optional **`onRendered`**: called after the Effect completes, so the parent can attach event listeners (form submit, button clicks, scroll) to the freshly rendered DOM.

So: **React owns the tree and the lifecycle; Effuse owns the content of one node.** When `deps` change, the program runs again and replaces that content.

### Navigation (no full page refresh)

Effuse templates use normal anchors (`<a href="/login">`) for internal links. To keep navigation SPA (no document reload), `EffuseMount` intercepts same-origin `<a>` clicks inside the container and calls `router.navigate({ href })`.

If you need to opt out (force a full reload), add `data-router-ignore` to the anchor.

### Data flow

- **Route loaders** (React/TanStack) fetch or compute data (e.g. auth, `signInUrl`, `userId`).
- **Route components** keep any local state (e.g. `modules`, `signatures`, `tools`, `messages`, `input`, `isBusy`) and load data in `useEffect` (e.g. via `AgentApiService`).
- They derive a **payload** (e.g. `pageData`, `autopilotChatData`) and pass it into the Effuse run function:

  ```ts
  <EffuseMount
    run={(el) => runModulesPage(el, pageData)}
    deps={[pageData]}
  />
  ```

- The Effuse program is **pure view**: it receives the payload and returns an Effect that renders HTML (using `html` and `rawHtml` from `@openagentsinc/effuse`). No async calls inside the view; all data is passed in.

### Event handling (autopilot chat)

The chat column is Effuse-rendered (form, Stop/Send, scroll-to-bottom). React has no direct refs to those nodes. So:

- **Form submit** – In `onRendered`, the parent finds `#chat-form` inside the container and attaches `submit`. On submit it reads `input[name="message"]`, calls `chat.sendMessage({ text })`, clears the input, and scrolls to bottom.
- **Stop / Scroll to bottom** – Buttons are rendered with `data-action="stop"` and `data-action="scroll-bottom"`. `onRendered` attaches click listeners to those nodes.
- **Input sync** – So that React state `input` stays in sync for re-renders, `onRendered` attaches an `input` listener and calls `setInput(el.value)`.
- **Scroll position** – The scrollable div has `data-scroll-id="autopilot-chat-scroll"`. `onRendered` stores it in `scrollRef` and attaches a scroll listener for `recomputeIsAtBottom`. Scroll-to-bottom is implemented by querying the container for `[data-autopilot-bottom]` and calling `scrollIntoView`.

When `deps` (e.g. `autopilotChatData`) change, Effuse re-runs and replaces the DOM; the previous nodes (and their listeners) are discarded. The next `onRendered` runs and reattaches to the new nodes. No cleanup is required for the old listeners because the nodes are gone.

---

## What was converted, by area

### 1. Marketing layout (header)

- **Before**: React header in `_marketing.tsx`: `Link` (OpenAgents), `a` (Log in), `HatcheryButton` (Start for free). Nav visibility toggled by `pathname === '/'`.
- **After**: Same layout and behavior, but the header is produced by **`effuse-pages/header.ts`** (`runMarketingHeader(container, isHome)`). The marketing layout still renders the HUD background in React, then:

  ```tsx
  <EffuseMount run={(el) => runMarketingHeader(el, isHome)} deps={[isHome]} className="shrink-0" />
  <Outlet />
  ```

- **Home** (`/`) and **Login** (`/login`) bodies were already Effuse (`runHomePage`, `runLoginPage`) in `effuse-pages/home.ts` and `login.ts`; only the header was moved to Effuse here.

**Files:**

- `apps/web/src/effuse-pages/header.ts` – Renders `<header>` with logo and conditional nav.
- `packages/effuse-ui/src/hatcheryButton.ts` – Effuse helper that renders the “HatcheryButton” SVG-frame button used on marketing pages (Tailwind classes only; no component CSS).
- `apps/web/src/app.css` – App globals (Tailwind import, fonts, theme tokens, scrollbars).
- `apps/web/src/routes/_marketing.tsx` – Uses `EffuseMount` for header; background + Outlet unchanged.

### 2. Catalog routes: modules, signatures, tools

- **Before**: Each route was a full React page: HUD background, React header, `KranoxFrame`, and a list of `<details>` items built from API data (modules/signatures/tools). Data loaded in `useEffect` via `AgentApiService`.
- **After**:
  - **Background** – Still React (DotsGridBackground + gradient).
  - **Content** – One Effuse program per route that renders the same app shell (header with “OpenAgents” + title, main with title block and scrollable list). List state: loading, error, or an array of items; each item is a `<details>` with summary and stringified JSON (or derived strings like `promptSummary` for signatures).

Payloads are plain data (no React nodes):

- **Modules** – `{ errorText: string | null, sorted: Array<{ moduleId, description, signatureIdsJson }> | null }`. `signatureIdsJson` is `safeStableStringify(m.signatureIds)`.
- **Signatures** – `{ errorText, sorted: Array<{ signatureId, promptSummary, inputSchemaJson, outputSchemaJson, promptIrJson, defaultsJson }> }`. `promptSummary` comes from `summarizePromptIr(s.promptIr)` in the route.
- **Tools** – `{ errorText, sorted: Array<{ name, description, usage, inputSchemaJson, outputSchemaJson }> }`.

Route components still do:

- `Route.useLoaderData()` for `userId`, redirect if unauthed.
- `useState` + `useEffect` to fetch modules/signatures/tools and set error state.
- `useMemo` to build the payload (sorted list + stringified fields).
- `useCallback` for `run = (el) => runModulesPage(el, pageData)` (or signatures/tools).
- Render: background div + `<EffuseMount run={run} deps={[pageData]} className="..." />`.

**Files:**

- `apps/web/src/effuse-pages/modules.ts` – `runModulesPage(container, data)`.
- `apps/web/src/effuse-pages/signatures.ts` – `runSignaturesPage(container, data)`.
- `apps/web/src/effuse-pages/tools.ts` – `runToolsPage(container, data)`.
- `apps/web/src/routes/modules.tsx`, `signatures.tsx`, `tools.tsx` – Load data, build payload, render background + EffuseMount.

### 3. Autopilot chat column

- **Before**: The center column of `/autopilot` was React: header “Autopilot”, scrollable message list (user/assistant bubbles, `Streamdown` for streaming text, `ToolCard` for tool parts), scroll-to-bottom button, and form (controlled input, Send/Stop). Sidebar and Blueprint panel were already separate.
- **After**:
  - **Chat column** – Rendered by **`effuse-pages/autopilot.ts`** (`runAutopilotChat(container, data)`). It receives:
    - `messages`: array of `{ id, role, renderParts }`. Each part is either `{ kind: 'text', text, state }` or `{ kind: 'tool', toolName, toolCallId, state, inputJson, outputJson?, errorText?, preliminary?, usage?, description? }` (all display strings).
    - `isBusy`, `isAtBottom`, `inputValue`.
  - **Tool cards** – Implemented as `<details>` in Effuse (no React state for expand/collapse).
  - **Streaming** – No `Streamdown`; Effuse just renders the current text. Streaming is reflected by re-running with updated `messages` and `inputValue`.
  - **Form and buttons** – Rendered in Effuse; behavior is wired in React via `onRendered` (see “Event handling” above).

The route still owns:

- `useAgent` / `useAgentChat`, `messages`, `input`, `isBusy`, `toolContractsByName`, `isAtBottom`, scroll refs, blueprint state, and all handlers (submit, stop, export, reset, etc.).
- It builds `autopilotChatData` (including `inputValue: input`) and passes it to Effuse.
- It passes `onRendered={onChatRendered}` so that after each Effuse run it reattaches form submit, Stop, scroll-to-bottom, input listener, and scroll listener.

**Files:**

- `apps/web/src/effuse-pages/autopilot.ts` – `runAutopilotChat(container, data)`. Defines `AutopilotChatData`, `RenderedMessage`, `RenderPart`; renders header, scroll area with `data-scroll-id` and `data-autopilot-bottom`, message list, scroll button, form with `id="chat-form"` and `input name="message"`, Send/Stop buttons with `data-action` where needed.
- `apps/web/src/routes/autopilot.tsx` – Builds `autopilotChatData`, `runAutopilotChatRef`, `onChatRendered`; renders background, sidebar, `<div ref={chatMountRef}><EffuseMount run={...} deps={[autopilotChatData]} onRendered={onChatRendered} /></div>`, Blueprint aside, control panel.

---

## File map

| Area        | Effuse entrypoint              | Route / layout              | Notes                                      |
|------------|---------------------------------|-----------------------------|--------------------------------------------|
| Marketing  | `effuse-pages/header.ts`        | `_marketing.tsx`            | Header only; home/login bodies unchanged.  |
| Home       | `effuse-pages/home.ts`          | `_marketing.index.tsx`      | Already Effuse before this conversion.     |
| Login      | `effuse-pages/login.ts`        | `_marketing.login.tsx`      | Already Effuse; takes `signInUrl`.         |
| Modules    | `effuse-pages/modules.ts`      | `modules.tsx`               | Data from AgentApiService, payload to Effuse. |
| Signatures | `effuse-pages/signatures.ts`   | `signatures.tsx`            | Same pattern; `summarizePromptIr` in route. |
| Tools      | `effuse-pages/tools.ts`        | `tools.tsx`                 | Same pattern.                              |
| Autopilot  | `effuse-pages/autopilot.ts`    | `autopilot.tsx`             | Chat column only; sidebar/blueprint React. |

Shared:

- `apps/web/src/components/EffuseMount.tsx` – Mounts a div, runs `run(container)` with `deps`, calls `onRendered` after run.

---

## Adding or changing an Effuse page

1. **Add a program** in `apps/web/src/effuse-pages/` (e.g. `myPage.ts`):
   - Export a function `runMyPage(container: Element, data: MyPageData): Effect.Effect<void>`.
   - Use `DomServiceTag`, `EffuseLive`, `html` (and optionally `rawHtml`) from `@openagentsinc/effuse`.
   - Build a single `content` (TemplateResult) and call `yield* dom.render(container, content)`.
   - Provide `EffuseLive` and catch errors so one failed render doesn’t break the app.

2. **Define the payload type** (e.g. `MyPageData`) so the route can build it from loader data and state.

3. **In the route**:
   - Keep loader and any local state / fetch logic.
   - Build the payload with `useMemo`, and `run = (el) => runMyPage(el, payload)` with `useCallback`.
   - Render background (if any) and `<EffuseMount run={run} deps={[payload]} onRendered={...} />` as needed.

4. **If you need events** (submit, click):
   - In the Effuse template, use stable selectors (`id`, `data-action`) or `name` on inputs.
   - In `onRendered`, query the container and add listeners; no cleanup needed when the DOM is replaced on next run.

---

## References

- **Effuse package and framework**: `packages/effuse/`, this `packages/effuse/docs/` (INDEX.md, README.md, ARCHITECTURE.md, SPEC.md, DOM.md, etc.).
- **apps/web usage summary**: [INDEX.md](./INDEX.md) → “Usage in apps/web”.
- **Router and Effect integration**: [ROUTER-AND-APPS-WEB-INTEGRATION.md](./ROUTER-AND-APPS-WEB-INTEGRATION.md).
- **Effect in apps/web**: [effect-migration-web.md](./effect-migration-web.md).
- **TanStack Start + Effect comparison**: [tanstack-start-effect-comparison.md](./tanstack-start-effect-comparison.md).
