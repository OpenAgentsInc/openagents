# Cursor vs. VS Code dependencies: what’s inherited vs. added

This analysis compares:
- VS Code repo manifest: `/Users/christopherdavid/code/vscode/package.json`
- Cursor app bundle manifest and runtime modules: `/Applications/Cursor.app/Contents/Resources/app/package.json` and its `node_modules/` (scanned up to depth 3)

Key point: Cursor’s app `package.json` contains no dependency lists (it’s a product manifest). The actual runtime set is what’s bundled under `node_modules/` in the app. The VS Code repo manifest includes both runtime deps and extensive dev/build/test deps that will not appear in a packaged app.

## Method
- Collected VS Code dependency names: all keys from `dependencies` + `devDependencies` (149 names).
- Collected Cursor runtime module names by scanning `node_modules/**/package.json` up to depth 3 (280 names).
- Computed set relations:
  - Common (present in both sets): 42
  - Cursor‑only (present in Cursor bundle, not listed in VS Code manifest): 238
  - VS Code‑only (present in VS Code manifest, not in Cursor app bundle): 107

Caveat: “Cursor‑only” includes many transitive runtime modules that VS Code does use but does not list as top‑level deps; conversely, “VS Code‑only” contains build/test tooling that is not expected in a packaged runtime.

## Common runtime (inherited from VS Code)
Representative modules that appear in both:
- Terminal and UI internals: `node-pty`, `@xterm/*`, `vscode-textmate`, `vscode-oniguruma`, `vscode-regexpp`
- Networking/utilities: `http-proxy-agent`, `https-proxy-agent`, `undici`, `open`, `minimist`, `tslib`
- Platform/native helpers: `native-keymap`, `native-watchdog`, `native-is-elevated`, `v8-inspect-profiler`
- Archiving/parsing: `yauzl`, `yazl`
- VS Code platform packages: `@vscode/deviceid`, `@vscode/iconv-lite-umd`, `@vscode/policy-watcher`, `@vscode/proxy-agent`, `@vscode/spdlog`, `@vscode/sqlite3`, `@vscode/sudo-prompt`, `@vscode/tree-sitter-wasm`, `@vscode/vscode-languagedetection`

Conclusion: Cursor inherits the core VS Code runtime stack (terminal, grammar/tokenization, sqlite-backed storage, platform helpers).

## Cursor additions (not listed in VS Code manifest)
Distinct runtime modules present in the Cursor app bundle that are not declared in the VS Code repo manifest. Highlights:
- Sentry SDKs and internals
  - `@sentry/browser`, `@sentry/electron`, `@sentry/node`, `@sentry/node-core`, `@sentry/core`, and `@sentry-internal/*`
- OpenTelemetry core + broad instrumentation set
  - `@opentelemetry/api`, `core`, `resources`, `sdk-trace-base`, `semantic-conventions`
  - Instrumentations: `express`, `http`, `undici`, `pg`, `mongodb`, `mysql`, `mysql2`, `graphql`, `koa`, `knex`, `ioredis`, `redis-4`, `tedious`, `amqplib`, `kafkajs`, `fs`, `dataloader`, `hapi`, `connect`, `mongoose`, `lru-memoizer`, `generic-pool`, etc.
- Cursor‑specific native and utilities
  - `cursor-proclist` (native addon for process listing)
  - `chrome-remote-interface`
  - `windows-foreground-love` (Windows focus helper)
- UI / runtime libraries not typical in upstream VS Code
  - `solid-js` and subpackages (`solid-js/web`, `store`, etc.)
- Telemetry service client variant
  - `tas-client-umd` (VS Code repo references `tas-client`) 

Interpretation: Cursor adds production observability (Sentry) and pervasive OpenTelemetry instrumentation, a custom native module, and uses SolidJS in parts of its UI/runtime. The UMD telemetry client suggests customized build/runtime integration.

## VS Code‑only (not in Cursor app bundle)
Mostly dev/build/test tooling and repo‑only utilities that do not ship in a packaged runtime. Examples:
- Build/test: `electron`, `@playwright/test`, `mocha`, `gulp*`, `copy-webpack-plugin`, `css-loader`, `asar`, `eslint*`, `istanbul-*`, `husky`, `innosetup`
- Type definitions: `@types/*`
- VS Code dev utilities: `@vscode/test-cli`, `@vscode/test-electron`, `@vscode/test-web`, `@vscode/l10n-dev`, `@vscode/telemetry-extractor`, `@vscode/v8-heap-parser`, `@vscode/ripgrep`
- Misc dev libs: `glob`, `file-loader`, `ansi-colors`, `katex`, etc.

Interpretation: Absence in Cursor’s bundle is expected; these are used for building, packaging, tests, or developer tooling in the repo, not at runtime.

## Cursor’s package.json vs. VS Code’s
- Cursor app `package.json` (in the bundle) contains only product metadata (name, version, main, repo URL) and no dep lists.
- VS Code repo `package.json` enumerates both runtime and extensive dev dependencies for building VS Code from source.

Thus, to understand Cursor’s runtime, the authoritative source is the app bundle’s `node_modules` inventory (see `node-modules-in-app-bundle.md`).

## Counts and files
- VS Code manifest dep names: 149
- Cursor bundled module names (depth≤3): 280
- Common: 42
- Cursor‑only: 238 (includes many transitive runtime deps and Cursor‑specific additions)
- VS Code‑only: 107 (build/test/dev artifacts)

Supporting inventories:
- `docs/re/cursor/node-modules-in-app-bundle.md` — list of top‑level runtime modules (subset)
- `docs/re/cursor/local-cursor-system-survey.md` — overall local install survey

If you want, I can export the full raw sets used for this comparison into the repo (common, cursor‑only, and vscode‑only) for future reference.

