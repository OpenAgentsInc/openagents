# Node, pnpm, and Vite Plus VP-2 Node-runtime receipt

- Class: receipt
- Date: 2026-07-14
- Status: complete and published on `main`; package-manager, test-runner, and
  production-image authority remain explicitly assigned to VP-3 through VP-6
- Dispatch: no; use [#8796](https://github.com/OpenAgentsInc/openagents/issues/8796)
- Parent: [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777)
- Owner: Sol runtime and toolchain conversion
- Implementation: `a34a4da943`
- Reference runtime: Node `24.13.1`

## Result

Retained TypeScript services and executable packages now have a stock-Node
runtime path before the workspace changes package-manager or Vite Plus
authority. The implementation replaces direct Bun globals in retained source
entrypoints with three narrow owned seams:

- `@openagentsinc/runtime-platform` owns file I/O, process spawning, standard
  input, executable lookup, zstd, build output, and HTTP/WebSocket lifecycle;
- `@openagentsinc/postgres-runtime` owns the structural SQL client contract on
  `postgres.js`; and
- `@openagentsinc/sqlite-runtime` promotes its `node:sqlite` implementation,
  statement cache, nested savepoints, typed failures, and conformance suite.

The runtime platform is intentionally smaller than Bun. It implements only
the behavior used by retained callers. Node server readiness is explicit,
ephemeral port selection is observed after bind, streamed files remain
streams, subprocess output remains byte-safe, WebSocket upgrade data remains
typed, idle timeouts are applied to HTTP and upgraded sockets, and graceful
shutdown has an awaitable boundary.

`Runtime.build` is backed by esbuild during this bridge phase. It preserves
the used disk-output, in-memory-output, naming, plugin, external, define,
source-map, target, and format contracts. ESM Node bundles receive a scoped
`createRequire` bridge so CommonJS dependencies such as `ws` still execute in
the compiled artifact. Vite Plus/tsdown becomes the final package/build
authority in VP-4; this adapter is a runtime migration seam, not a competing
permanent toolchain.

## Retained service proof

The Live Hub is bundled from its real production entrypoint and imported from
the compiled ESM artifact under stock Node. The smoke starts the real HTTP/
WebSocket server on an ephemeral port, awaits bind readiness, requests the
real `/health` route, observes the exact empty-hub projection, and performs
graceful teardown. This caught two failures that ambient Bun types did not:

- a symlinked macOS `/tmp` entry path made the first Node `isMain` check false;
  the seam now compares canonical real paths; and
- dynamically bundled CommonJS WebSocket code required an ESM-local
  `createRequire` bridge.

Aiur and the audio edge use the Node `ws` client for authenticated upstream
upgrade headers. Passing a second argument to the browser-shaped global
`WebSocket` would not have preserved those headers under Node. Disk-backed OTA
and Forum/Portal assets now pass actual readable streams to `Response`, and
the trace-archive upload supplies a real stream body with Node's duplex
contract.

## Public executable artifacts

One shared tsdown catalog builds the seven JavaScript CLI surfaces:

1. `@openagentsinc/khala`
2. `@openagentsinc/agent-readiness`
3. `@openagentsinc/product-spec`
4. `@openagentsinc/assurance-spec`
5. `@openagentsinc/qa-runner`
6. `@openagentsinc/pylon-runtime`
7. `@openagentsinc/pylon`

Each staged package contains compiled ESM and declarations, a Node shebang,
no raw TypeScript runtime entry, no `workspace:` version, and no Bun runtime
reference. The smoke hides Bun from `PATH` and executes every JavaScript bin
with `--help`. Internal implementation packages are bundled. The two public
type dependencies that remain in Pylon Runtime declarations are projected at
their concrete public package versions rather than leaking workspace ranges.
The native Pylon Foundation bridge remains its native executable and is not
misrepresented as JavaScript.

This phase proves the CLI entrypoints and their retained library entries. The
atomic VP-4 manifest rewrite remains responsible for replacing every source
export in the final published package manifests without contracting a public
API.

## Exact verification

The exact reference-runtime command is:

```text
NODE_OPTIONS=--max-old-space-size=8192 npx -y node@24.13.1 --test \
  packages/runtime-platform/src/runtime-platform.node-suite.ts \
  packages/sqlite-runtime/src/node-database.node-suite.ts \
  scripts/public-cli-artifacts.node.test.mjs \
  scripts/vp2-node-runtime-guard.test.mjs \
  scripts/vp2-retained-service.node.test.mjs
```

Result: 19 tests passed, zero failed. Coverage includes seven runtime-platform
tests, nine SQLite tests, one installed-CLI artifact test, the static guard
fixture, and one compiled retained-service smoke. The standalone QA artifact
was also built and its real `--help` command exited zero under Node 24.13.1.
The explicit heap ceiling accommodates the concurrently expanded QA/assurance
workspace during declaration bundling; the first post-rebase default-heap run
reached Node's 4 GiB ceiling while compiling the staged assurance artifact.

Focused TypeScript checks pass for the new runtime, PostgreSQL, and SQLite
packages and for the retained service/CLI consumers changed by the phase. The
VP-2 static guard reports zero retained-source violations. It rejects Bun
globals, Bun module imports, and Bun shebangs in production source while
naming the temporary `sqlite-runtime` Bun implementation as the sole
comparison oracle.

## Named remaining work

This receipt does not falsely claim the later phases:

- VP-3 converts `bun:test`, conformance/testkit imports, mocks, timers, and
  Effect TSGo/Vite Plus test identity;
- VP-4 atomically replaces Bun workspace scripts and `bun.lock` with pnpm and
  Vite Plus authority;
- VP-5 replaces Bun production images and release/deploy host machinery; and
- VP-6 deletes the SQLite comparison oracle, Bun types, stale examples, and
  every remaining supported Bun reference.

The phase boundary is deliberate: production code is Node-capable before the
old workspace toolchain is removed, so the atomic VP-4 change has a useful
rollback point without creating a permanent dual-runtime policy.
