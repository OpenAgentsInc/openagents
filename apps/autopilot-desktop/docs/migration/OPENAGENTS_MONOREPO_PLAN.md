# OpenAgents Monorepo Integration Plan (Autopilot Desktop)

Goal: Move this Autopilot Tauri app into the OpenAgents monorepo as
`apps/autopilot-desktop`, rename the Rust crate to `autopilot-desktop`,
and wire it into the OpenAgents workspace without breaking builds.

## Scope

- Move the entire repo into OpenAgents under `apps/autopilot-desktop/`.
- Rename the Tauri crate to avoid conflict with `crates/autopilot`.
- Use OpenAgents workspace dependencies for shared crates.
- Keep JS tooling local to the app (no root JS workspace for now).
- Remove nested Cargo.lock and rely on the workspace lockfile.

## Preconditions

- OpenAgents repo is cloned at `~/code/openagents`.
- Autopilot repo is clean and up to date.

## Step-by-step

1. Create target directory
   - `~/code/openagents/apps/autopilot-desktop/`

2. Move repo contents
   - Move all files and directories from this repo into
     `apps/autopilot-desktop/`.
   - Do not move `.git`.

3. Rename the Rust crate
   - Update `apps/autopilot-desktop/src-tauri/Cargo.toml`:
     - `name = "autopilot-desktop"`
     - Ensure `package` metadata still matches app name.

4. Wire into OpenAgents workspace
   - Add `apps/autopilot-desktop/src-tauri` to
     `~/code/openagents/Cargo.toml` `workspace.members`.

5. Switch path deps to workspace deps
   - In `apps/autopilot-desktop/src-tauri/Cargo.toml`, change any
     `path = "../../openagents/..."` entries to `workspace = true`.
   - Ensure OpenAgents workspace dependencies list includes those crates.

6. Remove nested Cargo.lock
   - Delete `apps/autopilot-desktop/src-tauri/Cargo.lock` if present.

7. Update repo-relative paths
   - Update any scripts/docs that assume the old repo root layout
     (for example, scripts that refer to `src-tauri` or `ai-server`
     relative to the old root).

8. Validate basic commands
   - `cargo check -p autopilot-desktop` (from OpenAgents root).
   - `bun install` and `bun run dev` (from apps/autopilot-desktop).

## Notes

- If we later need a JS workspace, add a root `package.json` or
  `pnpm-workspace.yaml` and hoist dependencies; not required now.
- Keep any app-specific docs under `apps/autopilot-desktop/docs/`.
