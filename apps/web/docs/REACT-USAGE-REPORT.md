# How React Is Still Used in apps/web UI

This report describes where and how React is used in the apps/web UI so that a refactor to remove React can be scoped. Effuse (Effect + template literals + DOM) already renders most route content; React remains as the shell for routing, mounting, and state.

---

## 1. Application bootstrap and routing (React-bound)

| Location | What React does |
|----------|------------------|
| **TanStack Start** | `createStart()` in `start.ts` — framework is React-based (Vite + React). |
| **Router** | `createRouter()` in `router.tsx` — `@tanstack/react-router` and `@tanstack/react-start`; route tree, loaders, and context are React-router concepts. |
| **Root** | `__root.tsx` — Uses `<Outlet />`, `<HeadContent>`, `<Scripts>`, `<AuthKitProvider>`, `<ConvexProviderWithAuth>`, `<RegistryProvider>`, `<HydrationBoundary>`. Entire app is a React tree. |
| **Route definitions** | Every route file uses `createFileRoute('/...')` and exports a `Route` with `component: SomeComponent` and often `loader`. Components are React function components. |

So: **routing, layout, and provider tree are 100% React**. Removing React would require a non-React router and a different root shell (e.g. custom client entry that mounts Effuse-only roots per route).

---

## 2. Route components (React as shell + state)

Each route that shows UI is a **React component** that:

- Calls React hooks (`useState`, `useEffect`, `useCallback`, `useMemo`, `useRef`).
- Uses router hooks: `Route.useLoaderData()`, `useRouter()`, `useRouterState()`.
- Renders a **single mount point** (e.g. `<EffuseMount run={...} deps={...} />`) and passes “what to render” and “when to re-run” into Effuse. The actual DOM is produced by Effuse (templates + `DomService.render`), not by React JSX.

**By file:**

| File | React role |
|------|------------|
| **autopilot.tsx** | Heavy: ~30+ `useState`/`useMemo`/`useCallback`/`useEffect`/`useRef`; `useAtomValue`/`useAtomSet` for session, sidebar, chat snapshot; builds `renderInput` and passes it into `runAutopilotRoute` via `EffuseMount`. React owns: when to run Effuse, what data to pass, and all local UI state (blueprint, controls, etc.). |
| **_marketing.tsx** | Light: `useRouterState`, `useRef` for stable SSR header HTML; renders layout divs + `<Outlet />` + `EffuseMount` for header and HUD. |
| **_marketing.index.tsx** | Light: `useMemo` for home SSR HTML; redirect logic; renders `EffuseMount` with home template. |
| **_marketing.login.tsx** | Medium: `useState` (step, email, code, isBusy, errorText), `useMemo` (model), `useRef` (SSR, ezRegistry); renders `EffuseMount` for login page. |
| **modules.tsx, signatures.tsx, tools.tsx** | Same pattern: `useAtomValue(PageDataAtom)`, `useRef` for SSR HTML; render `EffuseMount` with Effuse template. |
| **__root.tsx** | Root layout: `useEffect` (auth fetch, Convex token), `useAtomValue(SessionAtom)`; renders full provider tree and `<Outlet />`. |

So: **React is the “controller” for every page**: it holds state, runs loaders, and decides when to call Effuse with what data. It does not render the main DOM for those pages; Effuse does.

---

## 3. EffuseMount (React as mount + effect runner)

**File:** `components/EffuseMount.tsx`

- **React:** `useRef` (mount node, run/hydrate/onCleanup refs, ezRegistry, didSkipInitialRender, ezMounted, isUnmounting), `useEffect` (link interception, Ez runtime mount, main run/hydrate effect keyed by `deps`), `useMemo` (stable innerHTML for SSR).
- **Behavior:** Renders a single `<div ref={ref} />` (or with `dangerouslySetInnerHTML` when `ssrHtml` is provided). When `deps` change, runs `run(container)` (an Effect program) which updates the DOM inside that div. So **React owns:** the mount point, when to run the Effuse program, and stable SSR HTML; **Effuse owns:** the DOM inside the div.

To remove React here you’d need another way to: create the mount node, run the Effect when “deps” change, and optionally set initial HTML (e.g. from a non-React client entry).

---

## 4. State: @effect-atom + React

**Packages:** `@effect-atom/atom`, `@effect-atom/atom-react`

- **Atoms** (defined in `effect/atoms/`): `SessionAtom`, `AutopilotSidebarCollapsedAtom`, `AutopilotSidebarUserMenuOpenAtom`, `ChatSnapshotAtom(chatId)`, `AutopilotChatIsAtBottomAtom(chatId)`, `ModulesPageDataAtom`, `ToolsPageDataAtom`, `SignaturesPageDataAtom`, etc. These are Effect-based reactive state.
- **React bindings:** `useAtomValue`, `useAtomSet`, `RegistryProvider`, `HydrationBoundary`, `scheduleTask`. Used in `__root.tsx`, `autopilot.tsx`, `modules.tsx`, `signatures.tsx`, `tools.tsx`, `AutopilotSidebar.tsx`.

So: **global and route-scoped state live in Effect atoms, but “subscribe and trigger re-renders” is done via React hooks.** To remove React you’d need another subscription mechanism (e.g. Effuse or raw Effect subscriptions) that drives “when to re-run” the Effuse program instead of React’s effect + deps.

---

## 5. Other React usage

| Location | Usage |
|----------|--------|
| **useAuthFromWorkOS.tsx** | `useCallback`, `useMemo` for auth token / session helpers. |
| **PostHogLoader.tsx** | `useEffect` to inject script. |
| **KranoxFrame.tsx** | `useMemo` for clip paths; accepts `ReactNode` children. |
| **HatcheryButton.tsx** | Typed as React component (`ReactNode`, etc.); used by Effuse templates via `effuse-ui`. |
| **Router/link interception** | In `EffuseMount`, `useEffect` + `container.addEventListener('click', ...)` to intercept `<a href="...">` and call `router.navigate()`. So **SPA navigation is driven by React effect + TanStack Router.** |

---

## 6. What does *not* use React for rendering

- **Effuse templates** (`effuse-pages/*.ts`): Pure Effect + `html` template literals; no JSX, no React.
- **Actual DOM updates** for autopilot, marketing header, home, login, modules/signatures/tools: Done by `DomService.render` / `swap` inside Effect programs, not by React render.

So: **rendering of the main content is already non-React (Effuse).** React is used for: (1) app shell and routing, (2) when to run Effuse and with what data (deps + renderInput), (3) state (atoms) and subscribing to it (hooks), (4) one-off effects (auth, PostHog, link interception).

---

## 7. Summary table

| Concern | Implemented with |
|--------|-------------------|
| App entry, router, root layout | React (TanStack Start + React Router) |
| Route components | React (one component per route; they call Effuse) |
| When to run Effuse / what data | React (`deps`, `renderInput`, refs) |
| Global/route state | Effect atoms + React hooks (`useAtomValue`, `useAtomSet`) |
| DOM for route content | Effuse (Effect + templates + DomService) |
| Event handling in Effuse DOM | Effuse Ez runtime (data-ez) + registry (handlers still call React setters) |

---

## 8. Refactor directions to remove React

1. **Replace router:** Use a non-React router (or custom history + path → handler map) and a single client entry that mounts one root container and runs one “route” Effect based on path.
2. **Replace “when to run” and “what data”:** Instead of React `deps` and `renderInput`, drive updates from atom subscriptions (or other Effect subscriptions) that re-run the Effuse route program when relevant state changes.
3. **Replace EffuseMount:** A small “runner” that: creates a DOM node, runs the route’s Effect program with current state, and re-runs when subscribed state changes — without React.
4. **Keep Effuse + atoms:** Effuse and Effect atoms can stay; only the React bindings (hooks, provider, HydrationBoundary) need to be replaced by a non-React subscription and mount strategy.

The sidebar user menu not opening is likely either: (1) the user-menu atom not being writable so `setUserMenuOpen` doesn’t update, or (2) the Ez handler not firing / not being connected to the same registry. Making the atom explicitly writable and ensuring the Ez handler runs (and re-runs the Effuse program with new `sidebarKey`) addresses the menu bug within the current React shell.
