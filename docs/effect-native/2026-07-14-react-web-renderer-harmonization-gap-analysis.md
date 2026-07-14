# Effect Native + React web renderer harmonization gap analysis

- Date: 2026-07-14
- OpenAgents snapshot: `53c33ff8cfa1f8b2ce5f2c2b7adaf7461e44cf2e`
- Effect Native snapshot: `ec04d1a066d6f3ed0c67735ba451cfc90a343aa8`
  (`effect-native/v39`)
- T3 Code reference: `c1ec1915fc16f3dc1ec5d47d9a97f6210a574526`
- Class: source-grounded architecture gap analysis
- Status: recommendation plus Desktop R2 projection foundation; not broad route-conversion authority

## Executive decision

**Do not ditch Effect Native for web. Do not make the current React wrapper
around the imperative DOM renderer the destination either.**

The recommended steady state is:

1. Effect Native remains the application, state, component, token, intent,
   and portable view contract.
2. React becomes an implementation technology inside the DOM renderer, just
   as React already is inside the React Native renderer.
3. TanStack Start remains the web serving, routing, SSR, hydration, and route
   host.
4. React libraries such as Base UI, a virtualizer, Lexical, xterm, or a diff
   renderer enter only as renderer-private implementations or closed typed
   `Host` drivers. They do not become arbitrary `ReactNode`, callback, JSX,
   or `className` escape hatches in the Effect Native catalog.
5. The existing direct-DOM renderer remains in place during the pilot. After
   parity, accessibility, bundle, startup, and long-feed gates, converge on
   one public `@effect-native/render-dom` implementation instead of funding
   two permanent DOM renderers.

In one sentence: **keep Effect Native above React; put React underneath the
Effect Native DOM contract; wrap the old direct-DOM renderer only while
migrating.**

## Implementation status — Desktop R2 projection foundation landed 2026-07-14

The first executable rung remains intact, and the R2 projection foundation now
exists upstream at Effect Native commit
`ec04d1a066d6f3ed0c67735ba451cfc90a343aa8` and is consumed by OpenAgents
Desktop:

- `@effect-native/render-dom/react` is an optional-peer subpath with
  `EffectNativeReactDomSurface` and `makeReactDomRenderer`;
- one explicit whole-surface selector chooses `react` or `compatibility` for a
  surface lifetime; Desktop remains explicitly on compatibility until its full
  scoped node set has declared React lowerings;
- React mode opens the Effect stream once outside React effects and exposes a
  referentially stable, monotonic snapshot through `useSyncExternalStore`;
- that same Scope-owned adapter is now value-generic (while defaulting to
  `View`), so the scoped Desktop React shell can consume its authoritative
  Effect state directly without adding a React-owned store;
- the initial React kernel lowers Stack, Text, Button, Card, Spacer, and Divider
  to ordinary semantic elements with stable keys, closed a11y projection,
  canonical styles, and exact existing intent dispatch;
- unsupported nodes fail into a public error-boundary state instead of silently
  nesting or switching renderers;
- the mount adapter waits until the nested renderer's first View commit, so
  Desktop does not remove its boot frame or begin background hydration early;
- Scope close idempotently unmounts React and the nested renderer;
- Desktop now bundles the renderer with Vite, the React plugin, and Tailwind
  CSS 4, with semantic Tailwind roles mapped to canonical `--en-*` variables;
- React/React DOM are deduped to one app-owned `19.2.7` pair and production
  mode is selected at build time without injecting Node's `process` into the
  sandbox;
- the Electron boundary continues to forbid Node, tokens, generic host
  authority, React application state, JSX in portable modules, Tailwind class
  strings in Views, and parallel router/store/schema/theme/icon systems; and
- the full 1,350-test Desktop suite, built Electron reload smoke, and startup
  benchmark pass. The measured median was 694 ms to `shellMounted`, under the
  existing 2,500 ms budget.

This is the **R2 foundation**, not the final catalog-wide destination. React
does not yet lower each catalog node natively, Base UI is not yet wired, and there is no SSR/hydration,
Lexical, LegendList, Pierre, or xterm migration in this change. Keeping those
dependencies out until a real catalog/Host implementation exists avoids
turning a stack audit into unused supply-chain surface.

This does not authorize broad web product work. The Sol roadmap still closes
broad route conversion, landing, Forum expansion, portal, CRM, sales, and
outbound product work unless separately reauthorized. This document defines
the renderer seam to use when an authorized retained surface needs it.

## What exists now

The original July 8 dossier described a small future catalog. The current
implementation is already a substantial multi-renderer framework:

| Layer / surface | Current implementation | React today? | Consequence |
| --- | --- | --- | --- |
| `@effect-native/core` | 7,323-line React-free package; 79 Schema-backed component tags; typed keys, accessibility, interactions, bindings, styles, hosts, intents, and `ViewProgram` | no dependency | Keep it as the portable authority |
| `@effect-native/tokens` | 1,140-line semantic token, theme, spacing, type, control, motion, breakpoint, and matrix system | no dependency | Project it into every renderer; do not replace it with Tailwind strings |
| `@effect-native/render-dom` | Direct DOM renderer plus optional `./react` surface with one Effect-backed external store, a foundation ordinary-element kernel, error boundary, and explicit whole-surface backend selector | optional peer at `./react` | Downstream packets can expand React coverage without inventing state or command authority |
| `@effect-native/render-rn` | 6,331-line lowering from the same `View` union to React Native elements, plus a hook-backed surface | yes, internally | This already proves React can render Effect Native without becoming its authoring model |
| OpenAgents web | TanStack Start/Router, React 19, React DOM, Tailwind 4; retained Effect Native routes mount the direct DOM renderer from `useEffect` | React owns the route shell only | Effect Native content is an opaque client-only island |
| OpenAgents Desktop | Electron plus the shared React-owned Effect Native DOM surface; Vite/React/Tailwind renderer build; Effect Native application/state/intents unchanged | renderer host plus declared foundation lowerings | R2 keeps the complete shell explicitly on compatibility while downstream packets expand React coverage |
| OpenAgents mobile | Expo/React Native host plus `createEffectNativeSurface`; screen programs stay in Effect Native | yes, as renderer/host | This is the working architectural precedent |

The current vendor manifest pins core, tokens, direct DOM/React host, and React
Native at one upstream commit and catalog version. The React host is a subpath
of the DOM renderer, so no second vendored package or catalog version was
introduced.

### Current web path

Retained Effect Native route components currently do this:

```text
TanStack route
  -> React component renders <div ref>
  -> useEffect creates an Effect Scope
  -> makeDomRenderer().mount(div, program.viewStream, report)
  -> imperative DOM renderer owns everything below the div
```

This is a valid migration seam. It keeps React out of portable screen
programs, but it is not first-class React support:

- server rendering emits the host element, not the Effect Native content;
- the tests explicitly assert that legal and landing content is absent from
  the server HTML;
- React error boundaries, Suspense, contexts, portals, and developer tooling
  cannot see inside the subtree;
- every route repeats Scope, subscription, disposal, and error-swallowing
  boilerplate;
- TanStack navigation is translated back into browser-global navigation;
- React and the direct DOM renderer each own a lifecycle and reconciliation
  model in the same route; and
- browser behavior already solved by mature React primitives must be
  reimplemented in the direct DOM renderer.

### Current React Native path

The React Native renderer has the shape web should copy conceptually:

```text
Effect state/services
  -> ViewProgram<View>
  -> createEffectNativeSurface
  -> React elements
  -> React Native host components / Fabric
```

`render-rn` injects React and React Native dependencies, lowers portable View
nodes with `createElement`, subscribes to the Effect stream in a surface
component, and disposes the subscription on unmount. Mobile app code binds the
host dependencies once. Pure screen programs do not import React.

That distinction is the whole answer to “does supporting React mean leaving
Effect Native?” It does not. React can be the renderer without being the
application contract.

## Decision criteria

The choice must preserve all of these properties:

1. one serializable, Schema-checked component and intent grammar;
2. one semantic token and theme authority;
3. Effect ownership of services, errors, resource scopes, concurrency, and
   durable/domain state;
4. native specialization without independent product component trees;
5. real web SSR, hydration, semantic HTML, keyboard behavior, and routing;
6. access to mature React accessibility and workbench libraries without
   admitting arbitrary JSX into the portable contract;
7. observable renderer failures and exact disposal under Strict Mode and
   concurrent rendering;
8. cross-renderer conformance that does not require three hand-maintained
   implementations of every semantic rule; and
9. a migration that does not destabilize the shipping Electron or mobile
   clients.

## Options considered

| Option | Short-term cost | React ecosystem / SSR | Effect Native portability | Long-term cost | Verdict |
| --- | --- | --- | --- | --- | --- |
| Replace Effect Native web screens with direct React/JSX/Tailwind | low for a new web-only feature | excellent | lost; web becomes a second component/state/token system | high cross-platform drift and duplicate behavior | reject |
| Keep React route shells mounting the direct DOM renderer | already shipped | poor; subtree stays opaque and client-only | preserved | repeated glue plus two browser lifecycles | migration bridge only |
| Add arbitrary React components through a generic `Host`/`ReactNode` escape | low initially | excellent inside each island | nominal only; the escape becomes a second UI grammar | unbounded host, state, a11y, and theme drift | reject |
| Add a first-class React-backed Effect Native DOM renderer | medium | excellent after SSR/lifecycle work | preserved | medium, if shared lowering and one final DOM target are enforced | **recommend** |
| Keep independent direct-DOM and React-DOM renderers forever | high | excellent on React path | preserved in theory | permanent double DOM conformance burden | fallback only if measured Desktop constraints require it |
| Reuse `react-native-web` as the DOM renderer | medium | weaker web semantics/ecosystem fit | preserved | RN-shaped web compromises and another abstraction layer | do not prefer |
| Build a custom React reconciler/host config | very high | little benefit over ordinary React DOM | preserved | owns unstable internals needlessly | reject |

### Why direct React product UI is the wrong trade

T3 Code shows the upside: React 19, Base UI, TanStack Router, Tailwind,
Lexical, xterm, Pierre, virtualization, lazy loading, and React Compiler make a
capable workbench possible quickly. It also shows the bill: web and mobile own
separate component trees and theme vocabularies, while complex composer,
Markdown, diff, terminal, and feed behavior is implemented twice.

OpenAgents should adapt the implementation ecosystem, not T3's duplicated
application grammar. Replacing Effect Native on web would also discard the
current Desktop/mobile sharing, typed intent boundary, server/agent-safe View
data, conformance hooks, and renderer independence.

### Why the current wrapper is not enough

Wrapping the direct DOM renderer answers “can React host Effect Native?” It
does not answer “can Effect Native use React's renderer ecosystem?” The old
renderer still creates elements, manages portals, performs keyed child
reconciliation, restores focus and scroll, injects CSS, virtualizes lists,
and owns event listeners. React merely lends it a rectangle.

The destination must instead lower `View` nodes to React elements so React is
actually the browser renderer.

## Target architecture

```text
OpenAgents Effect application
  services / Layers / Scope / projections / commands / domain state
                              |
                              v
Effect Native core
  Schema View + NodeKey + typed styles + a11y + intents + Host contracts
                              |
                    renderer-neutral resolution
                              |
              +---------------+----------------+
              |                                |
              v                                v
      DOM target                         React Native target
  @effect-native/render-dom             @effect-native/render-rn
      |               |                         |
      |               +-- React surface         +-- React surface
      |                   for TanStack/SSR           for Expo/Fabric
      +-- owned mount API
          for Electron/non-React hosts

Renderer-private implementations
  semantic HTML / Base UI / virtualizer / typed editor, terminal, diff hosts
```

The public target should remain **DOM**, not “React” as a product concept. A
temporary `render-react-dom` package or experimental entry point is reasonable
during parity work, but the steady state should expose one
`@effect-native/render-dom` contract:

- a React component surface for TanStack Start composition and hydration; and
- a `makeDomRenderer`-compatible mount API that can create and own a React
  root for Electron or other non-React hosts.

If Desktop bundle/startup evidence rejects React, retain the direct renderer
as an explicitly funded low-dependency backend. Do not drift into that cost by
accident.

## Authority and state boundary

React support needs an explicit ownership ledger. “No React state” is too
coarse: renderers need ephemeral state. “React can own state” is too loose: it
creates a second application.

| State class | Owner | Examples | Persistence |
| --- | --- | --- | --- |
| Durable/domain truth | Effect services and owning server/projection | threads, turns, command lifecycle, approvals, receipts, user/account facts | owning service/database; never React |
| Application/session state | Effect Native program / typed Effect service | selected durable entity, loading/error phase, form model, command acknowledgement, navigation intent | explicit Effect lifetime and policy |
| Portable controlled UI value | Effect Native `View` props + intents | text value, selected tab, modal open state when product-significant, toggle value | follows program state |
| Renderer mechanics | React component or renderer store | focus, selection, IME composition, measured geometry, pointer capture, animation phase, virtualizer window, portal bookkeeping | memory-only; reset by keyed lifecycle |
| URL/address state | TanStack Router adapter | current route, params, search, history transition | browser/router; projected to typed navigation inputs |
| Foreign host state | Scope-owned typed driver | terminal emulator buffer, editor model, canvas instance, media element | declared per host; emits typed events |

Rules:

- No Zustand, React Query, Context, or hook state may become a second owner of
  domain data already modeled by Effect.
- Renderer-local state is allowed only when it is mechanical, non-authority,
  and reset/disposed by `NodeKey` and Effect Scope.
- A controlled input may buffer composition locally, but commits through a
  typed intent and reconciles to the portable value.
- React callbacks never enter the Schema tree. The renderer creates callbacks
  that report named intents.
- React context may carry renderer services such as theme, portal root,
  navigation, or host registry; it does not replace Effect Layers.

## Gap ledger

### G0 — policy precision

**Current:** the dossier says “leave React,” “no hooks,” and “DOM no React” in
places. The current RN renderer legitimately uses React and hooks internally,
and TanStack Start is the retained web host.

**Required:** make the boundary precise:

> Product views, behavior, state authority, tokens, and intents are authored
> in Effect Native. React, React Native, TanStack, and browser component
> libraries may implement a renderer or host shell. They are not the portable
> application contract.

The EN-9 import rule should therefore ban React from portable ViewProgram and
domain-authoring modules, not from reviewed renderer and host modules.

### G1 — a React-compatible View source

**Current:** `ViewProgram` remains a push-only `Stream<View>`, while the DOM
renderer now adapts it once per Scope into a stable synchronous snapshot store.
React 19 consumes that store with `useSyncExternalStore`; Strict Mode listener
replay does not reopen the upstream Effect stream.

**Required:** add a renderer-neutral source/store seam with:

- `getSnapshot(): VersionedView`;
- `getServerSnapshot(): VersionedView`;
- `subscribe(listener): () => void`;
- an explicit initial/server View;
- monotonic revision or stable digest;
- structural sharing by keyed subtree where practical; and
- Scope-owned interruption and failure reporting.

The React surface should consume this through `useSyncExternalStore` or an
equivalent React 19-safe adapter. It must not create a second product store.

One possible shape, for discussion rather than as a frozen API:

```ts
interface ViewSource {
  readonly getSnapshot: () => VersionedView
  readonly getServerSnapshot: () => VersionedView
  readonly subscribe: (notify: () => void) => () => void
}

interface EffectNativeRootProps {
  readonly source: ViewSource
  readonly report: IntentReporter
  readonly theme: Theme
  readonly hosts?: ReadonlyArray<ReactDomHostDriver>
}
```

### G2 — first-class React DOM surface

**Current:** `renderReactDomView` provides the declared foundation subset and
the owned React surface provides loading, failed, incompatible, recovery, and
teardown behavior. Catalog-wide lowering, portals, host services, and SSR are
still open.

**Required:** provide:

- a pure `renderReactDomView(view, context)` lowering;
- `<EffectNativeRoot>` for composition inside an existing React tree;
- server rendering and hydration entry points;
- an owned-root mount adapter compatible with `RendererAdapter`;
- injected React/ReactDOM dependencies or peers, not dependencies in core;
- stable `NodeKey -> key` identity;
- renderer status/error callbacks; and
- portal, navigation, clipboard, viewport, reduced-motion, and host services.

Do not use a custom React reconciler. Produce ordinary React elements and let
React DOM own the browser commit.

### G3 — renderer-neutral lowering and catalog drift

**Current:** core has 79 component tags. Direct DOM and RN each have a roughly
6,000-line renderer with large exhaustive switches and private semantic/style
helpers. Adding a third permanent hand-authored switch would triple drift.

**Required:** before full catalog parity, extract or generate:

- component capability metadata;
- resolved appearance and interaction state;
- accessibility/ARIA intent;
- typed style-to-web declarations;
- theme/custom-property serialization;
- shared `data-en-*` test/debug attributes;
- controlled/uncontrolled behavior matrices; and
- exhaustive fixture generation from `componentTags`.

Platform-neutral semantics belong in core or a renderer-neutral internal
package. Browser-only mechanics belong in one web lowering kernel used during
the direct-to-React migration. Catalog bumps must fail when any supported
renderer lacks an explicit supported, delegated-host, degraded, or unavailable
disposition.

### G4 — React lifecycle and concurrency

**Current:** the RN surface establishes a useful precedent with hooks and
Scope-like disposal, but the new web renderer must be safe under Strict Mode,
aborted renders, transitions, and replayed rendering. Current web wrappers
often swallow mount failures.

**Required:**

- no Effect execution, host mounting, mutation, or disposal during render;
- subscribe and acquire hosts only during commit effects;
- idempotent setup/cleanup under Strict Mode double mount;
- one Effect Scope per surface instance;
- interruption of stream subscriptions and child fibers on unmount;
- host mount/update/unmount exactly once per committed lifecycle;
- no duplicate typed intent dispatch after replay; and
- a typed renderer status/error channel plus React error boundary and recovery
  UI.

The same tests may reveal lifecycle work to lift back into `render-rn`.

### G5 — SSR and hydration

**Current:** the retained web host supports SSR, but Effect Native page content
does not. `/privacy` and `/stage1` tests intentionally prove that the server
renders only the mount shim.

**Required:** define an SSR contract:

1. load or construct a public-safe initial state through Effect on the server;
2. render one deterministic initial View;
3. emit actual semantic HTML through the React DOM renderer;
4. emit stable critical/component CSS and surface-scoped theme variables;
5. serialize only the minimal redacted hydration snapshot;
6. use the identical keys, catalog version, theme, and initial snapshot on the
   client; and
7. attach subscriptions and interactive hosts only after hydration commits.

Responsive resolution is a specific blocker. Today the renderer resolves
breakpoints from a JavaScript viewport. The server cannot know the client
viewport reliably. For web, prefer emitting responsive CSS when the View shape
does not change. Where structure truly changes, define one stable server
fallback and delay the structural change until after hydration without
changing semantic order or losing focus.

Acceptance must include meaningful page content in server HTML, no hydration
warnings, no duplicate intents, and correct no-JavaScript reading for retained
legal/public content.

### G6 — styling, tokens, and Tailwind

**Current:** Effect Native correctly keeps `className` out of the public View
contract. Typed style objects and semantic tokens lower to RN styles and DOM
CSS. The direct DOM renderer currently uses a runtime atomic stylesheet and
emits theme variables on global `:root`. Start also has a Tailwind palette and
a separate route-local Effect Native marketing theme, creating multiple token
authorities.

**Required:**

- keep typed Effect Native styles and `@effect-native/tokens` canonical;
- scope theme variables to the Effect Native surface and its portal root, not
  global `:root`;
- produce deterministic static or server-extractable component CSS;
- project canonical tokens into Tailwind/CSS variables for surrounding host
  layout rather than copying values by hand;
- use internal classes, data attributes, inline variables, or Tailwind
  utilities only as renderer output/build tooling; and
- delete duplicate route themes only through separately authorized visual
  parity work.

Tailwind is therefore compatible with Effect Native as an implementation and
token-consumer tool. Tailwind class strings remain incompatible as the
portable component contract.

### G7 — accessible primitives and React libraries

**Current:** the direct DOM renderer owns modal, popover, menu, tooltip,
combobox, tabs, focus restoration, keyboard handling, and portal behavior.
React can supply mature implementations, but substituting a library does not
automatically preserve the Effect Native contract.

**Required:** evaluate Base UI or React Aria/Floating UI behind renderer-private
adapters for complex interactive primitives. For every lowering prove:

- Effect Native props fully control product-significant state;
- library callbacks translate to one named typed intent;
- roles, names, descriptions, state, and keyboard behavior match the catalog;
- focus return, escape, outside interaction, scroll locking, nesting, and
  portals work;
- reduced-motion and forced-colors behavior is explicit; and
- no library type or component leaks into core.

Simple Text, Button, Link, Image, and form controls should remain semantic
HTML unless a library materially improves behavior.

### G8 — specialist workbench hosts

**Current:** Effect Native has a closed `HostKind` union and Scope-owned host
drivers for code editor, terminal, canvas, voice input, on-device model, and
video. The DOM renderer's editor path is not yet a T3-class Lexical/editor,
xterm, Pierre, or virtualized conversation implementation.

**Required:** use one of two bounded paths:

1. implement a catalog node internally with a React library when the portable
   semantics already fit, for example a virtualizer inside `Transcript`; or
2. add or extend a closed typed host driver when the library owns a specialist
   runtime, for example terminal, editor, canvas, or media.

Every host requires Schema-decoded serializable props, typed emitted events,
an explicit data/credential boundary, Effect Scope ownership, deterministic
mount/update/unmount, accessibility and fallback behavior, and cross-platform
disposition. Never add `component: ReactNode`, arbitrary children, callback
props, or unbounded module names to `Host`.

T3-derived mapping:

| T3 mechanism | Effect Native adaptation |
| --- | --- |
| Base UI primitives | internal web lowering for reviewed catalog primitives |
| Tailwind 4 | renderer/host build tooling fed from canonical EN tokens |
| TanStack Router | injected typed navigation adapter and route host |
| Lexical composer | renderer-private `Composer` implementation or typed rich-editor host |
| xterm | typed `terminal` host with bounded runtime bridge |
| Pierre diff | internal `DiffView` implementation or typed diff host |
| Legend List / virtualized feed | internal collection/transcript renderer after portable anchoring semantics are specified |
| Zustand | renderer-local ephemeral mechanics only; never domain authority |
| React Compiler / lazy loading | renderer build/performance detail, proven by budgets |

### G9 — routing and framework integration

**Current:** TanStack owns route matching and document metadata. Effect Native
navigation commonly falls through a DOM handler using browser globals.

**Required:** inject a typed navigation service into the React renderer:

- `Link` renders a semantic anchor with correct href for open-in-new-tab,
  copy-link, and no-JavaScript behavior;
- internal navigation reports or invokes the typed Navigate intent and maps to
  TanStack Router without putting router objects in View data;
- route params/search are decoded at the host boundary and passed into Effect
  services/program state;
- prefetch and transition behavior remain framework-private; and
- navigation failures are observable.

React route files may use JSX to establish the host, metadata, and providers.
They should not carry a second visual implementation of the same product
screen.

### G10 — focus, selection, scroll, and collections

**Current:** the direct DOM renderer contains substantial manual preservation
logic because whole-root commits and child replacement otherwise lose focus
and scroll. T3's strongest UI mechanics are anchored virtualization and
visible-content retention.

**Required:** define portable behavior before choosing a library:

- stable `NodeKey` identity across emissions;
- focus target and return semantics for overlays;
- input value, selection, IME, and submit behavior;
- vertical and horizontal scroll preservation;
- end anchoring and “user scrolled away” behavior;
- visible-content retention when earlier rows arrive;
- variable-height measurement and cache invalidation;
- keyboard navigation and active-descendant behavior; and
- bounded rich-text/diff caches.

React keys and a virtualizer help only after these semantics are explicit.

### G11 — performance and granularity

**Current:** every `SubscriptionRef` change maps through `render(state)` and
emits a resolved whole View. React can avoid DOM replacement with stable keys,
but it will still receive and traverse the tree. A high-frequency composer,
terminal, graph, or long transcript can overwhelm this shape.

**Required:**

- stable keyed structural sharing or subtree digests;
- memoized component lowerings where they are safe;
- batching aligned with React and Effect scheduling;
- renderer-local input/IME buffering with typed commits;
- virtualization for long collections;
- lazy specialist hosts and bounded caches;
- no whole-thread rebuild for cursor movement or terminal frames; and
- measured bundle, server-render, hydration, update, memory, and Electron
  startup budgets recorded before changing the default renderer.

Desktop already has explicit startup expectations and a test that excludes
React/TanStack/Tailwind dependencies. A Desktop convergence changes that
boundary and must earn it with evidence; it is not a mechanical package bump.

### G12 — failures, recovery, and developer experience

**Current:** `RendererAdapter.mount` declares no typed error, while route
wrappers can suppress Promise failures. React adds its own rendering failure
class.

**Required:**

- distinguish modeled application errors, host-driver errors, renderer
  defects, and hydration mismatch;
- report failures into Effect Native devtools/diagnostics;
- provide a catalog-authored recovery surface where possible;
- make source maps and component `NodeKey`/tag visible in diagnostics;
- surface catalog/renderer capability mismatches before runtime;
- provide one reusable host component rather than route-local Scope code; and
- preserve redaction when views or state enter diagnostics.

### G13 — source ownership and vendoring

**Current:** the monorepo vendors unbuilt TypeScript from
`OpenAgentsInc/effect-native` at one exact commit. Local-only renderer growth
would fork the framework.

**Required:**

1. write the renderer RFC and implementation in the owned Effect Native repo;
2. test it there against the catalog and existing renderers;
3. vendor the exact upstream commit and update the manifest atomically;
4. add every new vendored package/entry point to the provenance guard; and
5. convert OpenAgents consumers only after the vendored snapshot is coherent.

## Required conformance and acceptance matrix

React support is complete only when the following are executable, not prose:

| Gate | Minimum receipt |
| --- | --- |
| Catalog coverage | every v39-or-later tag has explicit React-DOM support/delegation/degradation; catalog bump fails omissions |
| Structure | shared fixtures produce intended semantic structure and stable keys across headless, direct DOM during migration, React DOM, and RN |
| Intent behavior | the same interactions emit the same typed intents and final Effect state exactly once |
| SSR/hydration | real retained content in server HTML; stable CSS/theme/keys; no hydration warning; no duplicate subscription or intent |
| Strict Mode | double-mount/replayed render tests show no Scope, listener, portal, timer, or host leak |
| Accessibility | automated axe plus keyboard-only and screen-reader task fixtures for overlays, forms, navigation, collections, transcript, and specialist hosts |
| Focus/input | focus return, selection, IME, controlled value, submit, and recovery survive updates |
| Collections | end anchor, prepend retention, variable height, horizontal/vertical scroll, and memory bounds pass |
| Styling | canonical token projection, surface/portal scoping, light/dark/system policy, reduced motion, forced colors, and no global theme collision |
| Errors | renderer/host/hydration defects are observable and recoverable; no swallowed mount rejection |
| Security | no credential or generic host authority enters React props/state; HTML/Markdown/link outputs remain sanitized |
| Performance | agreed bundle, SSR, hydration, update, long-feed memory, and Electron startup budgets pass on representative hardware |
| Import boundary | core/tokens/ViewProgram modules remain React-free; React imports stay in renderer and declared host-shell modules |
| Provenance | upstream commit, catalog version, vendored packages, and consumer lockfiles agree |

Visual snapshots are necessary but insufficient. Accessibility behavior,
state ownership, exact intent emission, cleanup, and performance need their own
oracles.

## Migration sequence

This sequence is conditional implementation guidance. It does not itself open
web product scope.

### R0 — promote the boundary

- approve the precise React-as-renderer policy in the Effect Native repo;
- name the temporary and final package/entry-point strategy;
- record state ownership and the no-arbitrary-React-host rule; and
- choose initial browser and Desktop budgets.

**Exit:** an upstream RFC/issue and boundary tests agree that Effect Native is
the contract and React is permitted only in renderer/host layers.

### R1 — stabilize today's bridge

- replace copied route `useEffect`/Scope boilerplate with one reusable,
  observable `<EffectNativeDomHost>`;
- stop swallowing renderer failures; and
- prove exact unmount/disposal.

**Exit:** current direct-DOM islands have one migration host with loading,
failure, and cleanup tests. This is not the final renderer.

### R2 — extract the web kernel and source contract

- create the versioned synchronous View source;
- extract shared web a11y, appearance, token, CSS, and data-attribute lowering;
- scope theme variables per surface/portal; and
- generate catalog capability fixtures.

**Exit:** the direct renderer still passes while pure shared lowerings and
source semantics have independent tests.

### R3 — React DOM vertical slice

- lower Stack, Text, Card, Button, Link, Image, TextField, and one controlled
  overlay through ordinary React elements;
- integrate typed navigation and error reporting; and
- add Strict Mode tests.

**Exit:** one View and intent fixture passes headless, direct DOM, React DOM,
and RN without a React type escaping core.

### R4 — SSR/hydration and complex primitives

- render one authorized static retained route with actual server content;
- add deterministic CSS/theme/viewport behavior;
- add Base UI or equivalent internal lowerings for overlays/combobox/menu/tabs;
  and
- prove keyboard, focus, portal, reduced-motion, and hydration behavior.

**Exit:** no empty server host, no hydration warning, and no duplicate
lifecycle or intent.

### R5 — dynamic workbench proof

- choose one authorized dynamic projection screen;
- add transcript/list anchoring, composer/input semantics, and one typed
  specialist host;
- compare memory/update latency and bundle impact to the direct renderer; and
- validate state ownership in diagnostics.

**Exit:** a representative workbench path passes long-feed, input, a11y,
error, and performance gates.

### R6 — choose the DOM implementation

- if gates pass, move the React-backed implementation behind the stable
  `@effect-native/render-dom` target;
- let TanStack use the component/hydration entry and Electron use the owned
  root mount entry;
- run Desktop startup/security/bundle gates before removing its React ban; and
- delete the duplicate direct-DOM implementation after parity.

If Desktop cannot accept React within the budgets, keep the direct backend
explicitly and fund a standing two-DOM conformance matrix. Do not claim one
renderer while maintaining two divergent semantics.

### R7 — governance

- lint portable ViewProgram/domain modules against React imports;
- lint View/Host schemas against callbacks, `ReactNode`, arbitrary components,
  and class strings;
- require catalog capability/conformance changes in the same commit as a
  catalog bump; and
- keep renderer-local state and foreign-host ownership documented.

**Exit:** React accelerates implementation without becoming a second product
architecture.

## Explicit non-goals

- no rewrite of existing mobile screens into direct React Native JSX;
- no direct React/Tailwind component system parallel to Effect Native;
- no generic React escape hatch in the serialized catalog;
- no React Server Components requirement—the first target is conventional
  deterministic SSR and hydration;
- no custom React reconciler;
- no speculative port of all T3 libraries;
- no automatic Desktop renderer swap before measured acceptance; and
- no reopening of closed web product or route-conversion scope.

## Final recommendation

React is not the alternative to Effect Native. It is a mature implementation
engine that Effect Native can target.

The React Native renderer already demonstrates the correct layering. Apply
that pattern to web: keep the Effect application and Schema View tree, add a
first-class React DOM lowering and surface, integrate TanStack/SSR and mature
web primitives below the typed boundary, then converge back to one public DOM
renderer after evidence. Preserve the current direct DOM renderer during the
pilot, not as an unquestioned permanent duplicate.

This captures T3's renderer velocity while refusing its two-design-system
cost. Ditching Effect Native would throw away the differentiating contract;
wrapping the existing renderer forever would throw away most of React's value.

## Primary evidence map

| Concern | Source |
| --- | --- |
| Catalog, styles, hosts, ViewProgram, renderer adapter | [`effect-native-core/src/index.ts`](../../apps/openagents.com/packages/effect-native-core/src/index.ts) |
| Canonical tokens/theme | [`effect-native-tokens/src/index.ts`](../../apps/openagents.com/packages/effect-native-tokens/src/index.ts) |
| Direct DOM reconciliation, CSS, focus/scroll, mount | [`effect-native-render-dom/src/index.ts`](../../apps/openagents.com/packages/effect-native-render-dom/src/index.ts) |
| React Native lowering and surface | [`effect-native-render-rn/src/index.ts`](../../apps/openagents.com/packages/effect-native-render-rn/src/index.ts) |
| Vendor provenance | [`effect-native-vendor.json`](../../apps/openagents.com/packages/effect-native-vendor.json) |
| Current mobile binding | [`effect-native-host.tsx`](../../apps/openagents-mobile/src/effect-native/effect-native-host.tsx) |
| Current Desktop mount | [`boot.ts`](../../apps/openagents-desktop/src/renderer/boot.ts) |
| Current web imperative island | [`-privacy-effect-native-page.tsx`](../../apps/openagents.com/apps/start/src/routes/-privacy-effect-native-page.tsx) |
| Server-shim test | [`-privacy-effect-native.test.tsx`](../../apps/openagents.com/apps/start/src/routes/-privacy-effect-native.test.tsx) |
| T3 frontend/runtime evidence | [T3 Code teardown](../teardowns/2026-07-13-t3-code-teardown.md) |
| Cross-product adaptation | [OpenAgents adaptation analysis](../teardowns/2026-07-10-openagents-product-adaptation-analysis.md) |
| Native renderer precedent | [Native SDK / Effect Native audit](../sol/2026-07-14-vercel-native-sdk-effect-native-desktop-audit.md) |
| Current product scope | [Sol subsystem implications](../sol/SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md) |

## Evidence limitations

The original analysis inspected source, tests, package manifests, current
decisions, and the pinned T3 reference. The follow-on R1 implementation added
and tested a React-owned host, ran the complete Desktop unit/contract suite,
real built Electron smoke/reload, and a deterministic startup benchmark. It
still did not implement native React catalog lowering, SSR/hydration, Base UI,
axe/VoiceOver/NVDA, or specialist workbench libraries, nor did it profile a
representative long transcript. The decision to replace the compatibility
direct-DOM lowering remains conditional on the R2–R6 executable gates above.
