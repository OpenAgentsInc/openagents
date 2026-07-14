# Native SDK × Effect Native spike

This bounded, non-shipping app was created with Native SDK 0.5.1's `native init`
and then adapted into a product-shaped hybrid architecture proof:

- Native SDK owns the macOS window, retained GPU canvas, 232px session rail,
  list/focus semantics, layout, automation, and child WebView placement.
- A real Effect Native `ViewProgram`, typed intent registry, shared catalog, and
  DOM renderer run the fixture transcript and MVP Codex composer inside that
  child WebView.
- Effect owns workspace, session, transcript, composer, pending, and revision
  state. The source selection in the native mirror changes only after Effect
  returns a versioned projection; the retained control may show its bounded
  optimistic press echo while the rail says it is synchronizing.
- The bounded fixture projection persists through WebView reload and native
  process restart in Web storage. Each document boot advances the Effect
  revision and fails any in-flight fixture turn closed to stopped.
- One exact-origin custom command carries a maximum 8 KiB projection and
  sequence/acknowledged native intent. The 120 ms pull is deliberately visible:
  Native SDK 0.5.1 can target bridge responses to child WebViews but its window
  event emitter targets only the primary WebView.
- Native rail actions carry the real Desktop canonical command IDs and dispatch
  the matching production Effect intent names. The mapping imports and checks
  `apps/openagents-desktop/src/desktop-command-contract.ts`; it is not a copied
  fixture registry.
- `frontend/src/native-sdk-component-adoption.ts` records which Native SDK
  components are plausible direct lowerings, owned composites, or bounded
  `Host` drivers. Effect Native remains the public authoring contract.

The transcript is deterministic fixture data and sends no provider request.
The proof does **not** compile Effect through Native SDK's restricted TypeScript
core compiler, implement a native Effect Native renderer, or replace the
shipping Electron desktop host.

## Verify

From this directory:

```sh
pnpm verify
```

That runs TypeScript checking, ten Effect Native intent/contract tests, the
Vite frontend build, six Native SDK Zig tests, the native binary build, and a
headed production-asset host gate. The host gate:

- refuses to share Native SDK's hard-coded automation directory with a live
  publisher;
- binds every observation to the exact spawned process id and protocol 6;
- resolves fresh widget ids from roles and accessible names before acting;
- drives session and workspace intents through the native canvas and waits for
  higher-revision Effect projections;
- captures the retained-canvas PNG, accessibility/snapshot evidence, and a
  composited macOS window PNG containing the live Effect Native WebView;
- reloads the Effect WebView, restarts the native process, and proves the
  Effect-owned fixture state is restored before creating a new chat;
- attests distinct initial/restart PIDs, exact Node/Zig/Native SDK identities,
  actual exit/signal/forced-kill state, and command, binary, frontend, source,
  and evidence digests in a
  schema-decoded `openagents.native-sdk.host-gate.v3` artifact;
- leaves private evidence under
  `var/native-sdk-effect-native-spike/host-smoke/` and prints
  `[native-sdk-effect-native-spike smoke] OK` only after all nine steps.

The wrapper now installs Native SDK's production asset source explicitly, so
the built binary loads `zero://app/index.html`; the proof no longer depends on
a Vite development server. Only the named `effect-native-surface` child may
publish Effect projections or consume native intents.

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
