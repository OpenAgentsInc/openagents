# Autopilot Desktop Migration (2026-01-25 14:05)

## Summary

Migrated the standalone Autopilot Tauri app into the OpenAgents monorepo
as `apps/autopilot-desktop`, and renamed the Rust crate to avoid
conflicts with `crates/autopilot`.

## What Changed

- Copied the Autopilot app into `apps/autopilot-desktop/` (excluding
  `node_modules`, `dist`, `target`, and local artifacts).
- Added `apps/autopilot-desktop/src-tauri` to the OpenAgents workspace
  members in `Cargo.toml`.
- Renamed the Tauri crate:
  - `package.name` -> `autopilot-desktop`
  - `lib.name` -> `autopilot_desktop_lib`
  - Updated `src-tauri/src/main.rs` and
    `src-tauri/src/bin/gen_types.rs` to use the new lib name.
- Switched `dsrs` and `dsrs-macros` to workspace dependencies.
- Removed the nested `src-tauri/Cargo.lock` so the workspace lockfile
  applies.
- Updated `apps/autopilot-desktop/package.json` name to
  `autopilot-desktop`.
- Wrote the migration plan to
  `apps/autopilot-desktop/docs/migration/OPENAGENTS_MONOREPO_PLAN.md`.

## Rationale

- Keeps product apps under `apps/` and core crates under `crates/`.
- Avoids naming collisions with existing crates.
- Centralizes dependency management in the OpenAgents workspace.

## Next Steps

1. Validate build and typecheck:
   - `cargo check -p autopilot-desktop` (from OpenAgents root)
   - `bun install && bun run dev` (from `apps/autopilot-desktop`)
2. Review any repo-relative paths in scripts or docs that assume the old
   Autopilot repo layout.
3. Decide whether to add a JS workspace at OpenAgents root (only if more
   JS packages will be managed across the monorepo).
4. After validation, delete the old standalone Autopilot repo
   (`~/code/autopilot`) if no longer needed.
