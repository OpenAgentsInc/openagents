# IDE-01 package, worker, theme, Vim, and native-placement admission

Date: 2026-07-19  
Issue: [#9016](https://github.com/OpenAgentsInc/openagents/issues/9016)  
Status: implemented foundation; not a production editor or release-rung promotion

## Decision

Admit exactly `monaco-editor@0.55.1` and `@pierre/diffs@1.2.12` as
replaceable renderer projections. Reject `monaco-vim@0.4.4` and
`@replit/codemirror-vim@6.3.0`. Use an app-owned, public-Monaco
`VimModeController` contract, off by default. Own one provenance-pinned Tokyo
Night data projection. Keep the IDE-02 path index in TypeScript/Effect; there
is no measured Rust case.

The opt-in package fixture proves these decisions in real Electron development
and ASAR layouts. It is deliberately not emitted by an ordinary Desktop build
and does not replace the production textarea. IDE-02 through IDE-07 still own
the actual Explorer/editor/navigation/review/language integration and daily-use
basic-IDE gate.

## Immutable artifacts and supply-chain disposition

| Artifact                       | Decision | Immutable identity                                                                                   | License / notice                     | Direct runtime graph                                                                                                            | Installed package bytes |
| ------------------------------ | -------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- | ----------------------: |
| `monaco-editor@0.55.1`         | adopt    | upstream commit `516f350bdaf7a82f6731bd128a9ec86a6e5fa47d`; registry integrity `sha512-jz4x…AFl9A==` | MIT; package license retained        | `marked@14.0.0`, `dompurify@3.2.7`                                                                                              |              72,635,377 |
| `@pierre/diffs@1.2.12`         | adopt    | upstream commit `9466c467ae6fc03501b6bca74c12f717d70293a7`; registry integrity `sha512-pY/g…a1cLQ==` | Apache-2.0; package license retained | `@shikijs/transformers`, `diff`, `hast-util-to-html`, `lru_map`, `shiki`, `@pierre/theming`, `@pierre/theme`; React 18/19 peers |               5,232,264 |
| `monaco-vim@0.4.4`             | reject   | upstream commit `f7f085732795f58f0fee5d03a46bdc459d6c8a30`                                           | MIT                                  | not installed                                                                                                                   |               0 shipped |
| `@replit/codemirror-vim@6.3.0` | reject   | exact registry integrity is recorded in the schema decision                                          | MIT                                  | not installed                                                                                                                   |               0 shipped |

The complete machine-readable record, including publish timestamps,
compatibility fields, registry unpacked sizes, new lock nodes, maintenance,
security posture, disposal, rollback, and seven explicit no-authority flags is
`apps/openagents-desktop/src/ide/package-admission.ts`. Exact pins are in the
Desktop manifest and lockfile. No range admits a later package silently.

The focused production license inventory also resolves `@pierre/theme`,
`@shikijs/transformers`, `hast-util-to-html`, `lru_map`, `marked`, and `shiki`
as MIT; `@pierre/theming` as Apache-2.0; `diff@9.0.0` as BSD-3-Clause; and
`dompurify@3.2.7` as `(MPL-2.0 OR Apache-2.0)`. Release notice generation must
retain those transitive notices; the top-level package license alone is not a
complete inventory.

Focused public VS Code candidates are not smuggled in by Monaco. `vscode-uri`,
`vscode-jsonrpc`, `vscode-languageserver-protocol`, and
`vscode-languageserver-textdocument` are individually deferred to IDE-06 with
admission conditions. Code-OSS workbench/Explorer internals, `vs/*` private
modules, extension host, context-key universe, settings registry, and
filesystem authority remain rejected.

## Compatibility and packaging result

Both adopted artifacts pass Electron 43, Node 24, TypeScript 6, sandbox,
offline, ASAR, and source-map gates. Monaco is React-independent. Pierre's
public React surface accepts React 19. Vite 8 and restrictive CSP pass through
the owned worker/asset adapter rather than through package defaults.

The private `openagents-app:` protocol now declares worker support and serves
only normalized allowlisted renderer assets. CSP remains:

- `default-src 'none'`;
- `script-src 'self'` and `worker-src 'self'`;
- `connect-src 'none'`;
- no `unsafe-eval`;
- no remote worker, theme, font, image, or source-map URL.

The admission graph emits editor, JSON, CSS, HTML, TypeScript, and Pierre
workers as immutable local assets. The real Electron probe observes every
request through `webRequest`; all requests use `openagents-app:` in both
development and ASAR runs. The probe performs three complete mount/dispose
cycles and one injected TypeScript-worker construction failure in each layout.
Every disposal and failure reaches zero Monaco models and zero tracked workers.

Each successful cycle proves:

- four Monaco models and one view;
- editor/JSON/CSS/HTML/TypeScript worker readiness;
- unified and side-by-side Pierre diffs;
- controlled line selection, annotation, context, and Tokyo Night styling;
- a live Pierre worker pool;
- a 200-file Pierre `CodeView` review with only five viewport items mounted;
- no external request; and
- exact teardown before the next generation.

## Bundle, source-map, startup, and memory evidence

The measured local Apple Silicon candidate receipt records:

| Observation                          |            Result |
| ------------------------------------ | ----------------: |
| ordinary renderer JavaScript         |   1,890,299 bytes |
| ordinary renderer CSS                |   1,513,383 bytes |
| opt-in fixture runtime closure       |  25,559,382 bytes |
| attribution-only fixture source maps |  59,577,055 bytes |
| fixture entry JavaScript             |   4,545,528 bytes |
| six worker assets                    |  10,192,739 bytes |
| development fixture load p95         |            534 ms |
| ASAR fixture load p95                |            466 ms |
| development renderer working set p95 | 404,226,048 bytes |
| ASAR renderer working set p95        | 391,004,160 bytes |
| IDE-00 ordinary-chat first-paint p95 |         547.22 ms |
| IDE-01 ordinary-chat first-paint p95 |         588.02 ms |

The ordinary `boot.js` contains no Monaco, Pierre, worker-constructor, or
fixture marker. Its JavaScript/CSS byte shape is unchanged from the IDE-00
candidate. The 40.8 ms first-paint p95 movement is therefore recorded as noisy
whole-process A/B evidence, not falsely attributed to editor code that is
absent from the graph; both candidates remain inside the established startup
budgets. IDE-03 must remeasure the real lazy editor route and chat-only delta.

Pierre's public root export currently makes Vite emit the complete dynamic
Shiki language/theme catalog. That explains most of the 25.6 MB runtime and
59.6 MB source-map closure. It is offline and lazy in the contained spike, but
it is not acceptable to ship accidentally. The build therefore emits the
fixture only when `OPENAGENTS_DESKTOP_IDE_PACKAGE_SPIKE_BUILD=1`. IDE-05 must
either narrow the public package closure or accept a written production budget
before integrating review. Vite's generated Rolldown runtime is the sole
JavaScript asset without its own map; all dependency/module chunks and workers
are attributable.

## Monaco boundary

Production integration may use only `monaco.d.ts` and documented ESM language
worker entries. The future app-owned runtime/controller maps opaque document
refs, versions, edits, selections, commands, theme data, and worker failures.
It does not receive a root, workspace grant, save authority, conflict policy,
Git operation, approval, persistence writer, or receipt writer.

The rollback is mechanical: remove the exact dependency and fixture graph.
The current textarea/stub remains the named compatibility path until IDE-03
accepts the real controller. No Code-OSS workbench or private `vs/*` API is
part of the decision.

## Pierre boundary

`PierreDiffProjectionSchema` accepts only an opaque review ref, opaque file
ref, bounded patch, mode, context, controlled selection, and annotations.
`PierreDiffCollectionProjectionSchema` adds a bounded 1..500 projection list
and viewport height for public `CodeView` virtualization. Schema decoding
strips undeclared values. The adapters never accept an absolute root, grant,
preload bridge, Git command, apply callback, process handle, approval, or
receipt capability.

Raw `unsafeCSS`, remote theme loading, and Pierre mutation actions are not
exposed. React roots and package worker pools remain owned by the outer scope.
The bounded fallback is the existing typed hunk renderer. This admits a review
projection, not Git or document authority.

## Built-in Vim decision

Neither evaluated package passes:

- `monaco-vim@0.4.4` imports the private
  `monaco-editor/esm/vs/editor/common/commands/shiftCommand` module and its own
  README warns that Ex/search/replace extra-input paths may fail. Private
  Monaco coupling and incomplete command input are release blockers.
- `@replit/codemirror-vim@6.3.0` is a credible CodeMirror 6 engine, but cannot
  control Monaco without admitting a second editor runtime and duplicate view
  state.

The selected fallback is therefore a first-party `VimModeController` over
public Monaco commands. It has no runtime dependency and is off by default.
Its schema enumerates exactly 32 capability decisions covering Normal, Insert,
Visual/Line/Block, Replace, operator pending, motions/counts/operators/text
objects, marks/registers/repeat/search/find, join/indent/case/paste,
undo/redo, explicit system clipboard, bounded Ex/save/close translation, IME,
screen readers, keyboard layouts, multi-cursor, split scoping, focus/Escape,
restart persistence, and handler teardown. IDE-03 implements that contract;
IDE-01 does not pretend a keymap exists yet.

## Tokyo Night projection

The owned source is pinned to `tokyo-night/tokyo-night-vscode-theme` commit
`7c0f11eaef322f293621ca7befe462214b7ea468`, with MIT provenance and license
checked in under `resources/third-party/tokyo-night/`. OpenAgents imports data,
not executable theme code or VS Code contributions.

One `DesktopThemeProjectionSchema` maps the palette to Effect Native/app
chrome and focus roles, Monaco chrome/syntax/diagnostics, Pierre tree and
diff/conflict roles, terminal ANSI/cursor/selection, and Problems, Output,
debug, review/proposal, browser, and status states. Theme data is installed
before model or view construction and never recreates a model/session.

The upstream faint foreground `#787c99` was adjusted to `#8990ad`; it reaches
5.42:1 against `#1a1b26`. Primary, muted, faint, blue, red, yellow, and green
semantic foregrounds all meet at least 4.5:1 in the checked contrast fixture.
Pierre explicitly records `allowUnsafeCss: false` and
`allowRemoteTheme: false`. The deterministic visual fixture is
`apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-tokyo-night.png`.

## Effect versus Rust result

The optimized TypeScript path-index candidate was measured on 10,000 files
across 100 directories:

| Metric                      |       p50 |       p95 |       p99 | p95 gate |
| --------------------------- | --------: | --------: | --------: | -------: |
| initial scan                | 21.077 ms | 23.199 ms | 23.982 ms |   500 ms |
| indexed path query          |  0.125 ms |  0.232 ms |  0.335 ms |    10 ms |
| apply 1,000-event churn     |  0.273 ms |  0.378 ms |  0.701 ms |    25 ms |
| real filesystem-watch event | 11.990 ms | 12.448 ms | 12.453 ms |   250 ms |

All gates pass, while the IDE-00 pre-index filesystem path-search p95 remains
recorded at 237.2 ms. TypeScript/Effect is selected for IDE-02. Rust is
rejected because it would add process/FFI, serialization, packaging, crash,
cross-platform, observability, and teardown boundaries without a measured user
benefit.

Reconsider Rust only after a representative production corpus repeatedly
breaches a written CPU/memory/latency budget after TypeScript algorithm,
batching, worker, and cache improvements. Any proposal must isolate the
smallest authority-free protocol, generated schema conformance, cancellation,
absence/failure behavior, end-to-end measurements including serialization,
and a reversal threshold. Project, document, session, identity, policy,
credential, database, approval, projection, and receipt ownership never move.

## Evidence and commands

Machine-readable evidence:

- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-package-spike.json`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-package-audit.json`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-typescript-index.json`;
- `apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-startup.json`;
- behavior contract `openagents_desktop.ide_package_admission.v1`.

Reproduction:

```sh
pnpm --dir apps/openagents-desktop run ide:package-spike
pnpm --dir apps/openagents-desktop run ide:package-audit
pnpm --dir apps/openagents-desktop run ide:typescript-index-benchmark
pnpm --dir apps/openagents-desktop run typecheck
pnpm exec vp test --run --root . apps/openagents-desktop/src/ide/package-admission.test.ts
pnpm --dir apps/openagents-desktop run check:ide-boundaries
```

The issue closing receipt records the landed `main` SHA and final command
results. These observations are public-safe fixtures: no repository source,
absolute workspace root, credential, user content, or external request is
captured.

## Exit and remaining work

IDE-01 exits with exact reversible package decisions, real offline/ASAR worker
proof, a projection-only virtualized diff adapter, a complete first-party Vim
contract, one accessible Tokyo Night source, and evidence against speculative
Rust. It does not ship Monaco, Pierre review, Vim behavior, or Tokyo Night in
the production workbench. It makes IDE-02 and IDE-03 implementable without
reopening package or authority questions.
