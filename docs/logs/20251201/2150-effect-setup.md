- Timestamp: 2025-12-01 21:50 (local)
- Context: Effect setup adjustments and typecheck fix.

Changes performed:
- Switched `tsconfig.json` to bundler-friendly settings (`module: preserve`, `moduleResolution: Bundler`, `noEmit`, Bun types, `typeRoots`, `baseUrl` + Electrobun path mapping) to align with Electrobun/Bun.
- Added `types/electrobun.d.ts` stub and path mapping so TypeScript ignores Electrobun’s broken upstream types while runtime still uses the real package; re-exported `electrobun/bun`.
- Silenced unused variable in `src/bun/index.ts` via `void mainWindow`.
- Verified `bun run typecheck` now passes.

Notes:
- If richer Electrobun typings are needed later, expand the stub instead of relying on `any`.
