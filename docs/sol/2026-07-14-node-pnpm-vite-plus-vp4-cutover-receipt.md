# Node, pnpm, and Vite Plus VP-4 cutover receipt

- Class: receipt
- Date: 2026-07-14
- Status: complete
- Dispatch: no; closed evidence for #8798 and folded leaves #8772–#8774
- Parent: #8777
- Runtime: Node `24.13.1`
- Package manager: pnpm `11.10.0`
- Toolchain: Vite Plus `0.2.4`

## Result

The repository now has one T3-style JavaScript toolchain authority. A single
`pnpm-lock.yaml`, `pnpm-workspace.yaml`, `.node-version`, `.npmrc`, and root
`vite.config.ts` replace the root and nested Bun locks/configs. The canonical
root gates are `pnpm run check`, `pnpm run typecheck`, `pnpm test`,
`pnpm run build`, and `pnpm run pack`; Vite Plus owns formatting, lint,
tests, builds, packaging, and the workspace task graph.

The folded leaves landed in the same authority change:

- TC-1: root verbs delegate through `vp`/`vp run` and retain package/task
  attribution;
- TC-2: the six-rule `oxlint-plugin-openagents` is loaded from root config,
  with valid/invalid integration fixtures; and
- TC-3: staged hygiene is formatter-only while pre-push delegates to the
  heavier canonical gates.

Every supported package script, deploy helper, and hook uses Node/pnpm/`vp`.
Direct Vite build commands were replaced by `vp build`, public executable
packaging is driven through `vp pack`, and the API build now owns its web-asset
prerequisite. Recursive builds are serialized to avoid the three simultaneous
large web builds that exceeded the clean host's memory ceiling.

## Verification

- frozen pnpm install: pass across 87 workspace projects;
- `pnpm run check`: pass;
- `pnpm run typecheck`: pass across 84 tasks (Probe remains the documented
  pre-existing diagnostic-baseline boundary);
- `pnpm run build`: pass across 14 build tasks, including the Worker dry-run;
- Vite Plus plugin integration fixtures: 6/6 pass; and
- public exact-tarball offline distribution: 6/6 pass.

Rollback is the published pre-cutover VP-3 commit. No compatibility lockfile,
second package-manager authority, or GitHub-hosted Action was added.
