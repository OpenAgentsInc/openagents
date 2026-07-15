# Native SDK × Effect Native spike

This bounded, non-shipping app was created with Native SDK 0.5.1's `native init`
and then adapted into a product-shaped hybrid architecture proof:

- Native SDK owns the macOS window, retained GPU canvas, 232px session rail,
  macOS File menu and ⌘N shortcut, list/focus semantics, layout, automation,
  and child WebView placement.
- The child WebView mounts the production OpenAgents Desktop center pane through
  React 19, Tailwind, and the React-owned Effect Native DOM renderer. It imports
  the real Desktop state type, intents, handlers, view builder, CSS, and theme;
  it does not maintain a parallel transcript/composer implementation.
- Electron still composes the production sidebar and center pane. The Native
  app imports a browser-safe `renderer-portable` seam containing only the
  center pane because the retained Native rail owns that region.
- Effect owns workspace, session, transcript, composer, pending, and revision
  state. The only application substitute is a bounded in-memory `ChatHost`
  with three privacy-safe threads and no provider, credential, filesystem,
  process, Git, or durable session custody.
- The bounded native rail projection persists through native process restart
  in Web storage. A Native lifecycle command tears down and remounts the
  React/Effect renderer scope over the same state; it does not claim a
  WKWebView document reload.
- One exact-origin custom command carries a maximum 8 KiB projection and
  sequence/acknowledged native intent. The 120 ms pull is deliberately visible:
  Native SDK 0.5.1 can target bridge responses to child WebViews but its window
  event emitter targets only the primary WebView.
- Native rail actions carry real Desktop canonical command IDs into the actual
  Desktop intent registry. File → New Chat and ⌘N go further: Native produces a
  complete `openagents.desktop.deferred_command.v1`, then the child imports the
  production strict decoder and resolver before dispatching `DesktopNewChat`.
  Exact `commandId`, intent, source, and sequence evidence appears in the rail
  only after the higher-revision Desktop projection returns.
- `frontend/src/native-sdk-component-adoption.ts` records which Native SDK
  components are plausible direct lowerings, owned composites, or bounded
  `Host` drivers. Effect Native remains the public authoring contract.
- Native SDK's own supervised spawn effect starts and retains a plain ESM
  sidecar under exact Node 24.13.1. The sidecar keeps the Electron-neutral
  production Desktop runtime gateway v11, Sync host, coding catalog, SQLite
  store, private binding, and admitted workspace service live behind an
  authenticated loopback-only protocol. Chat still uses the deterministic
  fixture host.

The thread data and `ChatHost` response are deterministic fixtures and send no
provider request. The proof does **not** compile Effect through Native SDK's
restricted TypeScript core compiler, implement a native Effect Native renderer,
provide provider/session custody, replace the shipping Electron host, or fully
confirm the Native MVP AssuranceSpec. It does fully confirm CW-AC-03; 17
criteria remain open.

## Verify

From this directory:

```sh
pnpm verify
```

That runs TypeScript checking, twelve Desktop/Effect Native integration and
contract tests, a production sidecar bundle, the Vite frontend build, eleven
Native SDK Zig tests, the native binary build, and a headed production-asset
host gate. The host gate:

- refuses to share Native SDK's hard-coded automation directory with a live
  publisher;
- binds every observation to the exact spawned process id and protocol 7;
- starts the minified persistent sidecar under exact Node 24.13.1, requires the
  production Desktop runtime gateway v11, and proves distinct live sidecar
  generations plus dead-after-host teardown;
- grants a real private Git repository through Native SDK's semantic file-drop
  action, then restarts and grants a symlink alias; requires the same six
  opaque refs and exactly one production coding-catalog session;
- resolves fresh widget ids from roles and accessible names before acting;
- drives session and workspace intents through the native canvas and waits for
  higher-revision Effect projections;
- captures the retained-canvas PNG, accessibility/snapshot evidence, and a
  composited macOS window PNG containing the real Desktop center pane;
- tears down and remounts the React-owned Effect renderer, restarts the native
  process, and proves the bounded rail state survives both lifecycle events;
- invokes Native File → New Chat after restart and requires exact production
  decode/resolve/dispatch evidence with no selected fixture session;
- attests distinct initial/restart PIDs, exact Node/Zig/Native SDK identities,
  actual exit/signal/forced-kill state, and command, binary, sidecar bundle,
  frontend, source, and evidence digests in a
  schema-decoded `openagents.native-sdk.host-gate.v5` artifact;
- leaves private evidence under
  `var/native-sdk-effect-native-spike/host-smoke/` and prints
  `[native-sdk-effect-native-spike smoke] OK` only after all twelve steps.

The wrapper now installs Native SDK's production asset source explicitly, so
the built binary loads a marked
`zero://app/index.html#surface=effect-native&assurance-run=…` child URL; the
proof no longer depends on a Vite development server. Native SDK also creates a
primary asset WebView, so the frontend refuses to boot there. Only the marked,
named `effect-native-surface` child may own Desktop state, persist its bounded
run-scoped projection, publish Effect projections, or consume native intents.

To run the full window with a Native SDK CLI on `PATH`:

```sh
pnpm run build:frontend
native dev . -Dautomation=true
```

In the umbrella workspace, the reference CLI can be built and used without
modifying the read-only reference checkout:

```sh
cd ../../../projects/repos/native
zig build cli
cd ../../../openagents/apps/native-sdk-effect-native-spike
../../../projects/repos/native/zig-out/bin/native dev . -Dautomation=true
```

`build.zig.zon` exact-pins the audited Native SDK commit rather than relying on
the local reference checkout's path.

The repeatable headed smoke is:

```sh
pnpm run build:frontend
pnpm run smoke
```

The full AssuranceSpec diagnostic lane is:

```sh
pnpm --dir ../../packages/assurance-spec run assure:mvp --target=native-sdk
```

It executes the 36 Native-owned criterion candidate/falsifier units and this
headed gate, writes a private gap report, and fails without publishing while
any full MVP criterion lacks target-specific integration evidence.

The persistent grant is counted only for CW-AC-03. The current diagnostic
confirms that candidate and its falsifier while retaining 17 explicit gaps;
the host gate is ready and publication remains withheld. The next useful
targets are CW-AC-17's bounded privacy-safe diagnostics and CW-AC-14's
grant-scoped workspace tree/status/diff projection through the same sidecar.

The script drives the file protocol directly and therefore does not need a
machine-global Native SDK CLI. Native SDK's deterministic screenshot covers
the retained GPU surface; a separate window-scoped macOS capture covers the
composited Effect pane. The latter proves pixels, not DOM semantics, so a
browser/accessibility adapter is still required before MVP assurance admission.

## Adoption rule

Native SDK's components are implementation material, behavioral specifications,
and conformance-test oracles for a future Effect Native renderer. Their Zig and
Native-markup props must not leak into the closed Effect Native view catalog.
Renderer-specific components stay behind a typed `Host` driver; unsupported
specialists keep their existing renderer implementation.
