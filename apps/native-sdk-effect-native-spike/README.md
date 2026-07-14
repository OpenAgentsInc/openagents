# Native SDK × Effect Native spike

This bounded, non-shipping app was created with Native SDK 0.5.1's `native init`
and then adapted into a hybrid architecture proof:

- Native SDK owns the macOS window, retained GPU canvas, toolbar, rail, layout,
  built-in controls, and child WebView placement.
- A real Effect Native `ViewProgram`, typed intent registry, shared catalog, and
  DOM renderer run inside that child WebView.
- `frontend/src/native-sdk-component-adoption.ts` records which Native SDK
  components are plausible direct lowerings, owned composites, or bounded
  `Host` drivers. Effect Native remains the public authoring contract.

The proof does **not** compile Effect through Native SDK's restricted TypeScript
core compiler and does **not** replace the shipping Electron desktop host.

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
native dev .
```

In the umbrella workspace, the reference CLI can be built and used without
modifying the read-only reference checkout:

```sh
cd ../../../projects/repos/native
zig build cli
cd ../../../openagents/apps/native-sdk-effect-native-spike
../../../projects/repos/native/zig-out/bin/native dev .
```

`build.zig.zon` exact-pins the audited Native SDK commit rather than relying on
the local reference checkout's path.

## Adoption rule

Native SDK's components are implementation material, behavioral specifications,
and conformance-test oracles for a future Effect Native renderer. Their Zig and
Native-markup props must not leak into the closed Effect Native view catalog.
Renderer-specific components stay behind a typed `Host` driver; unsupported
specialists keep their existing renderer implementation.
