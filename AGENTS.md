# Repository Guidelines

## Project Structure & Module Organization
- Mobile app: `expo/` (Expo Router in `expo/app/`, assets in `expo/assets/`).
- Rust (planned): `crates/` (e.g., `crates/codex-bridge/`; workspace config will be added as crates mature).
- Docs: `docs/` (build/run notes in `docs/logs/`).

## Build, Test, and Development Commands
- Install deps: `cd expo && bun install` (aka `bun i`).
- Run locally (Metro): `bun run start`.
  - Platform targets: `bun run ios`, `bun run android`, `bun run web`.
- Type-check TypeScript: `bun run typecheck`.
- Lint TypeScript/TSX: `bun run lint`.
- iOS production build: `bun run build:ios:prod`.
- Submit iOS build: `bun run submit:ios`.
- Run bridge (Rust): from repo root `cargo run -p codex-bridge -- --bind 0.0.0.0:8787`.

## Coding Style & Naming Conventions
- Language: TypeScript (strict mode) for app code.
- Linting: ESLint with `eslint-config-expo` (see `expo/eslint.config.js`). Fix warnings before PRs.
- Indentation: 2 spaces; keep imports sorted logically (react/external → internal).
- Files: kebab-case for components (e.g., `themed-text.tsx`); Expo Router uses `index.tsx`, `_layout.tsx`.
- Rust (when added): crates kebab-case; modules snake_case; prefer `clippy` defaults.

## Rust Workspace & Dependencies
- Workspace: root `Cargo.toml` manages members (e.g., `crates/codex-bridge`). This is intentional so Rust builds run from the repo root.
- Lockfile: `Cargo.lock` lives at the workspace root by Cargo design. It will be regenerated at the root whenever building a workspace; do not move it into `crates/`.
- Dependencies: always use `cargo add` to modify dependencies; do not edit `Cargo.toml` by hand (except workspace structure/members).
- Versions: when adding, do not pass version constraints (`@x.y`, `@*`, or explicit ranges). Let `cargo add` select and pin the latest compatible version.

## JS/Expo Dependencies
- Use Expo-aware installs so versions match the SDK:
  - React Native libs: `cd expo && bunx expo install <package>` (no versions).
  - Generic libs: `cd expo && bun add <package>` (no versions).
- Do not hand-pin versions in `package.json`. Let the installer choose compatible versions and commit the updated `bun.lock`.
- If Expo warns about mismatches, run: `cd expo && bunx expo install` to align to expected versions.

## OTA Updates (iOS)
- EAS Update is configured with channel `v0.1.0` (see `expo/eas.json`). Runtime version comes from remote app version.
- Scripts (see `expo/package.json`):
  - `update:ios`: `eas update --channel v0.1.0 --environment production --platform ios --message`
  - `update:android`: same for Android; `update:both`: both platforms.
- To publish an iOS OTA update:
  - `cd expo && bun run update:ios -- "<short message>"`
  - Keep messages concise (what changed). Assets are deduped; make sure fonts/images are committed.
  - When bumping native runtime (breaking changes), increment the app version in `expo/app.json` and coordinate store builds; otherwise OTA won’t apply across runtimes.

### OTA Policy
- Default to iOS-only updates. Do not run Android or both-platform updates unless the user explicitly requests Android or both.
- When the user says “push an OTA update” without specifying a platform, run: `cd expo && bun run update:ios -- "<short message>"`.
- Avoid `update:both` and `update:android` unless specifically directed; Android builds/updates are not in scope right now.

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

## Agent Workflow Requirements (Expo TypeScript)
- If you touch TypeScript/TSX under `expo/`, run the type checker via `bun run typecheck`.
- Do not finish work until the typecheck passes with no warnings.

## Security & Configuration Tips
- Do not commit credentials or secrets. Use EAS for managed credentials (see `expo/eas.json`).
- iOS bundle identifier: `com.openagents.app` (see `expo/app.json`).
- Review assets and third‑party licenses before release.
