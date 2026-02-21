# Rust Migration Execution Control Plane

Status: Active
Last updated: 2026-02-21
Owner: `@AtlantisPleb` (interim DRI across all lanes until delegated)

## Board

- GitHub Project: https://github.com/orgs/OpenAgentsInc/projects/12
- Project name: `Rust Migration Execution`
- Scope: all `OA-RUST-*` issues

## Swimlanes (`Migration Status`)

Use the custom project field `Migration Status` with these lanes:

1. `Backlog`: scoped but not dependency-ready.
2. `Ready`: dependency-ready and owner-confirmed.
3. `In Progress`: active implementation with an assigned DRI.
4. `Blocked`: waiting on unresolved dependency/risk.
5. `Verification`: implementation landed; validation/rollout checks in progress.
6. `Done`: issue closed with comment linking commit(s)/PR(s) and verification evidence.

## Label Taxonomy

All `OA-RUST-*` issues must carry exactly one label from each set:

- `phase:*`
- `area:*`
- `owner:*`
- `risk:*`
- `deps:*`

Required baseline labels remain:

- `roadmap`
- `planning`
- `enhancement`

### `area:*` labels

- `area:program`
- `area:proto`
- `area:openagents.com`
- `area:wgpui-web`
- `area:runtime`
- `area:khala`
- `area:desktop`
- `area:ios`
- `area:client-core`
- `area:data`
- `area:infra`
- `area:testing`
- `area:docs`
- `area:release`
- `area:adr`
- `area:auth`
- `area:protocol`
- `area:security`
- `area:observability`
- `area:payments`
- `area:services`
- `area:packages`
- `area:ci`
- `area:sync-docs`
- `area:build`
- `area:onyx`

### `owner:*` labels

- `owner:openagents.com`
- `owner:runtime`
- `owner:khala`
- `owner:desktop`
- `owner:ios`
- `owner:infra`
- `owner:contracts-docs`

### `risk:*` labels

- `risk:low`
- `risk:medium`
- `risk:high`
- `risk:critical`

### `deps:*` labels

- `deps:none`
- `deps:single`
- `deps:multi`
- `deps:external`

### `phase:*` labels

- `phase:01-foundation`
- `phase:02-proto`
- `phase:03-control-service`
- `phase:04-wgpui-web-foundation`
- `phase:05-runtime-rust`
- `phase:06-khala`
- `phase:07-desktop-consolidation`
- `phase:08-ios-shared-core`
- `phase:09-web-cutover`
- `phase:10-data-reliability`
- `phase:11-adrs`
- `phase:12-hardening`
- `phase:13-closure`

## Area Owner Map

Current interim DRI mapping:

1. `openagents.com` -> `@AtlantisPleb`
2. `runtime` -> `@AtlantisPleb`
3. `khala` -> `@AtlantisPleb`
4. `desktop` -> `@AtlantisPleb`
5. `ios` -> `@AtlantisPleb`
6. `infra` -> `@AtlantisPleb`
7. `contracts/docs` -> `@AtlantisPleb`

When dedicated owners are assigned, update this map and keep `owner:*` labels aligned.

## Triage Rules

1. New `OA-RUST-*` issue creation requires phase/area/owner/risk/deps labels at creation time.
2. `Ready` requires:
   - all blocking dependencies closed or explicitly waived,
   - DRI confirmed in issue body metadata,
   - acceptance criteria present.
3. Move to `Blocked` immediately when a dependency or external gate is not satisfiable.
4. Move to `Verification` after implementation lands and before issue closure.
5. `Done` requires:
   - issue comment with commit/PR links,
   - verification notes,
   - issue closure.

## Reporting Conventions

1. Weekly migration report snapshots:
   - total by `Migration Status`,
   - blocked list (`Migration Status=Blocked`),
   - high/critical risk items due in current phase.
2. Daily execution updates for active lane:
   - issues moved to `In Progress`,
   - issues moved to `Verification`,
   - issues closed (`Done`).
3. Dependency watch:
   - filter by `deps:multi` and `risk:high`/`risk:critical`.

## Verification Commands

```bash
gh project item-list 12 --owner OpenAgentsInc --limit 500
gh label list --repo OpenAgentsInc/openagents
gh issue list --repo OpenAgentsInc/openagents --search "OA-RUST-" --state open --limit 200
```

