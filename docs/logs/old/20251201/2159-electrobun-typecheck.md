- Timestamp: 2025-12-01 22:20 (local)
- Context: Type-only Electrobun resolution and bundler-safe tsconfig splits for builds.

Changes performed:
- Added `tsconfig.typecheck.json` so type checks can include the Electrobun declaration shim while keeping the bundler-friendly `tsconfig.json` clean for runtime builds.
- Pointed the `typecheck` script at the new config, and kept `tsconfig.json` free of Electrobun path overrides so `electrobun build` resolves the real files.
- `types/electrobun.d.ts` continues to describe the API, while build-time bundling uses the actual `node_modules/electrobun` exports for runtime behavior.
- Verified `bun run typecheck` and `bun run build` both succeed on the current setup.

Notes:
- Build failures will reappear if `tsconfig.json` tries to reroute `electrobun` at runtime; keep the shim confined to the dedicated typecheck configuration.
