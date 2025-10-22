# Repository Guidelines

## Project Structure & Module Organization
- Mobile app: `expo/` (Expo Router in `expo/app/`, assets in `expo/assets/`).
- Rust (planned): `crates/` (e.g., `crates/codex-bridge/`; workspace config will be added as crates mature).
- Docs: `docs/` (build/run notes in `docs/logs/`).

## Build, Test, and Development Commands
- Install deps: `cd expo && bun install` (or `npm install`).
- Run locally (Metro): `bun run start` (or `npm run start`).
  - Platform targets: `bun run ios`, `bun run android`, `bun run web`.
- Lint TypeScript/TSX: `bun run lint`.
- iOS production build: `bun run build:ios:prod`.
- Submit iOS build: `bun run submit:ios`.

## Coding Style & Naming Conventions
- Language: TypeScript (strict mode) for app code.
- Linting: ESLint with `eslint-config-expo` (see `expo/eslint.config.js`). Fix warnings before PRs.
- Indentation: 2 spaces; keep imports sorted logically (react/external → internal).
- Files: kebab-case for components (e.g., `themed-text.tsx`); Expo Router uses `index.tsx`, `_layout.tsx`.
- Rust (when added): crates kebab-case; modules snake_case; prefer `clippy` defaults.

## Testing Guidelines
- Currently no test suite. When adding tests:
  - Frameworks: Jest + `@testing-library/react-native`.
  - Naming: colocate as `*.test.tsx`/`*.test.ts` next to source.
  - Targets: aim for 80%+ statement coverage on new code.
  - Run with a `test` script (to be added in `expo/package.json`).

## Commit & Pull Request Guidelines
- Commits: imperative, concise subject (≤50 chars), e.g., "Add splash screen asset"; include a brief body when needed.
- PRs: clear description, link issues, note scope, and include screenshots/GIFs for UI changes.
- Checks: lint passes, app boots via `bun run start`, docs updated when behavior changes.

## Security & Configuration Tips
- Do not commit credentials or secrets. Use EAS for managed credentials (see `expo/eas.json`).
- iOS bundle identifier: `com.openagents.app` (see `expo/app.json`).
- Review assets and third‑party licenses before release.
