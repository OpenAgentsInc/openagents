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
- One exact-origin custom command carries a maximum 8 KiB projection and
  sequence/acknowledged native intent. The 120 ms pull is deliberately visible:
  Native SDK 0.5.1 can target bridge responses to child WebViews but its window
  event emitter targets only the primary WebView.
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

That runs TypeScript checking, Effect Native intent/contract tests, the Vite
frontend build, Native SDK Zig tests, and the native binary build.

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

The focused headed smoke is:

```sh
native automate wait
native automate snapshot
native automate widget-click native-shell <renderer-boundary-widget-id>
native automate assert 'name="Renderer boundary".*selected' \
  'name="0 messages · revision 2"' 'dispatch_errors=0'
native automate screenshot native-shell 1
```

The deterministic Native SDK screenshot covers the retained GPU surface; on
the system WebView host the composited Effect pane is inspected in the live
window and is not included in that CPU-rendered PNG.

## Adoption rule

Native SDK's components are implementation material, behavioral specifications,
and conformance-test oracles for a future Effect Native renderer. Their Zig and
Native-markup props must not leak into the closed Effect Native view catalog.
Renderer-specific components stay behind a typed `Host` driver; unsupported
specialists keep their existing renderer implementation.
