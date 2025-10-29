# Repository Guidelines

## Codebase Summary
- Purpose: mobile command center for coding agents. Expo app drives agent sessions over a local WebSocket bridge to the OpenAI Codex CLI; Rust service spawns/streams the CLI; docs capture JSONL schema and operational notes.
- Architecture: two layers
  - App (`expo/`): Expo Router screens — Session (live feed + input), History (in Drawer; fetched from bridge), Library (UI component samples), Settings (bridge URL + permissions). Parses Codex JSONL into typed UI rows and cards.
- Bridge (`crates/oa-bridge/`): Axum WebSocket server on `--bind` (default `0.0.0.0:8787`) that launches `codex exec --json` (auto‑adds `resume --last` when supported) and forwards stdout/stderr lines to all clients; each prompt is written to the child’s stdin then closed to signal EOF.
- Key App Modules:
  - Routing: `expo/app/` with routes: `/session`, `/session/[id]`, `/projects`, `/project/[id]`, `/library`, `/settings`; message detail at `expo/app/message/[id].tsx`.
  - Session UI: `expo/app/session/index.tsx` renders a streaming feed. Incoming lines are parsed by `expo/lib/codex-events.ts` into kinds like `md`, `reason`, `exec_begin`, `file_change`, `web_search`, `mcp_call`, `todo_list`, `cmd_item`, `err`, `turn`, `thread`, `item_lifecycle`.
  - Components: JSONL renderers in `expo/components/jsonl/*` (e.g., `MarkdownBlock`, `ReasoningHeadline`, `ExecBeginRow`, `FileChangeCard`, `CommandExecutionCard`). A `HapticTab` adds iOS haptics for the tab bar.
  - State & storage: lightweight log store in `expo/lib/log-store.ts` (AsyncStorage backed) powers History and Message detail views.
- Connection/permissions: `expo/providers/ws.tsx` manages the WebSocket connection, exposes `readOnly`, `networkEnabled`, `approvals`, and `attachPreface` toggles (persisted). The header shows a green/red dot for connection.
- Rule: No HTTP calls to the bridge. All bridge control is via WebSocket control messages (e.g., `{ "control": "run.submit", ... }`) or via Convex queries/mutations. Do not add REST endpoints.
 - For the `packages/tricoder` CLI: do not implement your own Node WebSocket server. The Rust bridge (`crates/oa-bridge`) is the single source of truth for `/ws`.
  - Theming/typography: Dark theme in `expo/constants/theme.ts`; global mono font + defaults via `expo/constants/typography.ts` (Berkeley Mono; splash hidden after fonts load).
  - OTA: `expo/hooks/use-auto-update.ts` checks for `expo-updates` when not in dev; EAS configured for channel `v0.2.0` with runtimeVersion `appVersion` in `expo/app.json` and `expo/eas.json`.
- Bridge Details:
  - Entry: `crates/oa-bridge/src/main.rs`. Dependencies: `axum` (ws), `tokio`, `clap`, `tracing`.
  - CLI flags injected (unless provided): `--dangerously-bypass-approvals-and-sandbox`, `-s danger-full-access`, and config `sandbox_permissions=["disk-full-access"]`, `sandbox_mode="danger-full-access"`, `approval_policy="never"`, plus `-m gpt-5` and `-c model_reasoning_effort="high"`.
  - Resilience: if stdin is consumed after one prompt, the bridge respawns the child process for the next message. Large `exec_command_output_delta` payloads are summarized for console logs.
  - Repo root detection: runs Codex from the repository root (heuristic checks for both `expo/` and `crates/`).
- Docs:
  - JSONL schema and mappings: `docs/exec-jsonl-schema.md`.
  - Resume behavior: `docs/exec-resume-json.md`.
  - Permissions model and recommended setups: `docs/permissions.md`.
  - Projects & Skills schema: `docs/projects-and-skills-schema.md`.
- Repository Layout:
  - `expo/`: app sources, assets, config (`app.json`, `eas.json`, `eslint.config.js`).
  - `crates/oa-bridge/`: Rust WebSocket bridge crate.
  - `docs/`: developer docs and logs.
  - Root `Cargo.toml` and `Cargo.lock`: Cargo workspace anchor (lockfile at root by design).
- Development Flow:
  - App: `cd expo && bun install && bun run start` (then `bun run ios|android|web`). Type‑check with `bun run typecheck`; lint with `bun run lint`.
  - Bridge: from repo root `cargo bridge` (alias for `run -p oa-bridge -- --bind 0.0.0.0:8787`). App default WS URL is `ws://localhost:8787/ws` (configurable in Settings).
  - Agent prompts: Session screen optionally prefixes a one‑line JSON config indicating sandbox/approvals to match the bridge.
- Conventions & Policies (highlights):
  - TypeScript strict, 2‑space indent, imports grouped React/external → internal. Expo Router filename conventions.
  - Expo installs: use `bunx expo install` for RN/Expo packages; `bun add` for generic libs. Don’t pin versions manually; commit `bun.lock`.
  - OTA iOS default: prefer `bun run update:ios -- "<msg>"` unless Android explicitly requested.
  - Rust deps: add via `cargo add` without versions; let Cargo lock at root.
  - Security: no secrets; iOS bundle id `com.openagents.app`.
  - Gotchas: none specific to paths with parentheses now; the `(tabs)` group has been removed.

## Project Structure & Module Organization
- Mobile app: `expo/` (Expo Router in `expo/app/`, assets in `expo/assets/`).
- Rust (planned): `crates/` (e.g., `crates/oa-bridge/`; workspace config will be added as crates mature).
- Docs: `docs/` (build/run notes in `docs/logs/`).

## Build, Test, and Development Commands
- Install deps: `cd expo && bun install` (aka `bun i`).
- Run locally (Metro): `bun run start`.
  - Platform targets: `bun run ios`, `bun run android`, `bun run web`.
- Type-check TypeScript: `bun run typecheck`.
- Lint TypeScript/TSX: `bun run lint`.
- iOS production build: `bun run build:ios:prod`.
- Submit iOS build: `bun run submit:ios`.
- Run bridge (Rust): from repo root `cargo bridge`.

## Projects and Skills (Desktop‑side Source of Truth)

- Home folder: `OPENAGENTS_HOME` (defaults to `~/.openagents`).
  - Projects dir: `~/.openagents/projects/`
  - Skills dir: `~/.openagents/skills/`
- Format: Markdown with YAML frontmatter (see `docs/projects-and-skills-schema.md`).
  - Project files: `{id}.project.md`
  - Skill files: `{id}.skill.md`
- Validation: frontmatter is validated against JSON Schemas on the bridge.
  - Schemas: `crates/oa-bridge/schemas/project.schema.json` and `skill.schema.json`.
  - The bridge rejects invalid saves and skips invalid files when listing.
- WebSocket controls (Projects):
  - List: `{ "control": "projects" }` → `{ type: "bridge.projects", items }`
  - Save: `{ "control": "project.save", project }` (writes `{id}.project.md`)
  - Delete: `{ "control": "project.delete", id }`
- App behavior:
  - On mount, the app fetches projects over WS and seeds the local store for instant rehydrate; the Projects screen and drawer read from this store.

### Add a Project (example)
1. Create file `~/.openagents/projects/tricoder.project.md` with:
```
---
name: Tricoder
workingDir: /Users/you/code/openagents
repo:
  provider: github
  remote: OpenAgentsInc/openagents
  url: https://github.com/OpenAgentsInc/openagents
  branch: main
instructions: |
  A mobile command center for coding agents. Manage and talk to your coding
  agents on the go. Fully open source.
---

# Overview
...free‑form Markdown body...
```
2. Start the bridge: `cargo bridge`
3. Open the app — projects load via WS.

### Validate a Project
- The bridge auto‑validates on save/list. To check manually:
  - Ensure required fields: `name`, `workingDir`.
  - Optional fields may be present (see schema docs). If a save fails or an item doesn't show up in the list, validate frontmatter against the JSON schema (`crates/oa-bridge/schemas/project.schema.json`).

## Coding Style & Naming Conventions
- Language: TypeScript (strict mode) for app code.
- Linting: ESLint with `eslint-config-expo` (see `expo/eslint.config.js`). Fix warnings before PRs.
- Indentation: 2 spaces; keep imports sorted logically (react/external → internal).
- Files: kebab-case for components (e.g., `themed-text.tsx`); Expo Router uses `index.tsx`, `_layout.tsx`.
- Rust (when added): crates kebab-case; modules snake_case; prefer `clippy` defaults.

## Rust Workspace & Dependencies
- Workspace: root `Cargo.toml` manages members (e.g., `crates/oa-bridge`). This is intentional so Rust builds run from the repo root.
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
- EAS Update is configured with channel `v0.2.0` (see `expo/eas.json`). Runtime version comes from remote app version.
- Scripts (see `expo/package.json`):
  - `update:ios`: `eas update --channel v0.2.0 --environment production --platform ios --message`
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
- Commit and push as you go: make small, focused commits and push immediately after each commit. Do not leave local, unpushed changes — keep the shared branch updated to minimize merge conflicts and visibility gaps.

## Multi‑Agent Git Etiquette
- Do not stage everything: avoid commands like `git add -A`, `git add .`, or `git commit -a`. Explicitly add only the files you changed for the current task.
- Never stash: do not run `git stash` (including variants like `git stash -u`). Stashing can hide or discard work in progress from other agents operating on the same branch.
- Respect concurrent work: assume other agents may be active on this branch. Do not run destructive or history‑rewriting commands (e.g., `git reset --hard`, `git clean -fdx`, force pushes, or rebases) unless explicitly instructed.
- Keep commits focused: limit diffs to the smallest set of files necessary; avoid touching unrelated files.
- Review before committing: use `git status` and `git diff --staged` to confirm only intended paths are included.
- Branching policy: do not create branches unless the user explicitly directs you to. Default to committing on `main` and opening PRs only when requested.

### Absolutely No Destructive Local Ops (must‑follow)
- Never delete untracked files or folders in the working tree (e.g., `rm -rf docs/convex/`) — even if they look unrelated. Untracked work may be in progress by another agent.
- Never reset or clean the working tree to drop local changes without explicit user approval. Do not run: `git reset --hard`, `git checkout -- <path>`, `git clean -fdx`, or tooling that implies those operations.
- Never “restore to HEAD” unrelated files to make a commit look clean. Instead, only stage the files you intentionally changed.
- If you see unrelated local changes:
  1) Leave them untouched.
  2) Stage only your intended files (path‑spec add).
  3) Ask the user before attempting any local cleanup.
- When moving docs or code across folders, prefer Git moves in a focused commit and update all references in the same commit.
- Do not add trivial provenance comments to files (e.g., `<!-- Moved from ... -->`). Use meaningful, persistent documentation and rely on Git history for move provenance.

### Staging and committing checklist
- Before commit: `git status` shows only intended files under “Changes to be committed”.
- If you modified files in `expo/`, run `cd expo && bun run typecheck` and ensure it passes.
- Do not include unrelated reformats or mass renames in the same commit as functional changes.

### Local Working Tree Safety (STRICT)
- Do not modify, revert, or delete files you did not touch for the current task — even locally. Avoid commands like `git checkout -- <path>`, `git restore --source=HEAD <path>`, or editor auto-reverts on unrelated paths.
- Never delete untracked files or directories (e.g., `rm -rf docs/...`). Untracked content may be in-progress work by another agent.
- Do not use destructive commands: `git reset --hard`, `git clean -fdx`, `git stash*` (already disallowed), or any history rewriting.
- If the working tree contains unrelated changes, proceed by staging only your intended files (explicit paths). Leave unrelated changes untouched and unmodified.
- If your work would collide with those local changes or you suspect name collisions (e.g., a new folder vs. an existing tracked file), STOP and ask for guidance instead of attempting “local cleanup”.
- Before committing, verify your staged set does not include or revert other agents’ files. Use `git status --porcelain` and `git diff --staged` to double-check.

## Agent Workflow Requirements (Expo TypeScript)
- If you touch TypeScript/TSX under `expo/`, run the type checker via `bun run typecheck`.
- Do not finish work until the typecheck passes with no warnings.

## Security & Configuration Tips
- Do not commit credentials or secrets. Use EAS for managed credentials (see `expo/eas.json`).
- iOS bundle identifier: `com.openagents.app` (see `expo/app.json`).
- Review assets and third‑party licenses before release.

## CLI (Tricoder) Notes
- Do not run `npm run dev` in `packages/tricoder`. It launches a long‑lived process and will block your shell/session.
- For local testing, prefer one of:
  - `npx tricoder` (published package)
  - `npm run build` then run `node dist/index.js`
