# AFS-00 contract freeze, migration map, and baseline

Date: 2026-07-20

Status: implementation record for work packet AFS-00. This record is evidence.
It is not release authority. It is not a product promise.

Audience: human and agent.

Authority: the plan
`docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md` owns the
names, the bounds, and the rules. This record states what AFS-00 delivered.

## 1. What AFS-00 delivered

AFS-00 freezes the shared contract and the regression baseline for the Apple FM
router to full-agent-system program. AFS-00 changes no product behavior.

AFS-00 delivered these items:

1. The new root package graph and its manifests.
2. The frozen turn, provider, route, context, artifact, and presentation
   schemas in `packages/agent-runtime-schema/`.
3. The import-boundary check for every new root package.
4. The baseline fixtures and the regression tests.
5. The recorded Apple FM version-drift finding.
6. This migration map.

## 2. New package graph

AFS-00 adds five root packages. Each package has a manifest, a `tsconfig.json`,
a `README.md`, and subpath exports. Each package typechecks.

| Package | Root export | Extra exports | Role |
| --- | --- | --- | --- |
| `@openagentsinc/ide-runtime` | `.` | none | Reserved portable IDE schemas (AFS-05). |
| `@openagentsinc/agent-turn-runtime` | `.` | none | Turn policy and turn state machines. |
| `@openagentsinc/agent-turn-store` | `.` | none | Driver-neutral turn journal. |
| `@openagentsinc/apple-fm-runtime` | `.` | `./node`, `./testing` | Apple FM provider and wire-version source. |
| `@openagentsinc/agent-surface` | `.` | none | UI-neutral surface projectors. |

The dependency direction is a directed acyclic graph. `agent-turn-runtime` must
not import `apple-fm-runtime`. Apple FM implements the provider port instead.
`agent-turn-store` depends on `agent-turn-runtime`. The turn kernel does not
depend on the store.

## 3. Frozen schemas

AFS-00 adds six focused modules to `packages/agent-runtime-schema/src/`. The
package re-exports the modules from its public index.

| Module | Frozen contract |
| --- | --- |
| `turn.ts` | Turn intent, task class, lifecycle and terminal states, refusal reasons, usage truth, references, and the size and retained-state bounds. |
| `provider.ts` | Provider candidate vocabulary, provider descriptor, readiness, data destination, cost class, and the typed candidate set. |
| `route.ts` | Route recommendation, owner-bound candidate set, turn disclosure, and the route decision. |
| `context.ts` | The cross-surface work context envelope and its references. |
| `artifact.ts` | Released artifact reference, evidence reference, and the turn receipt. |
| `presentation.ts` | The safe turn projection and the eight-concept stage vocabulary. |

Each module names one immutable schema literal version. A change is compatible
inside one version only when it adds an optional field or widens an
unconstrained value. A new union case, a removed field, a newly required field,
or a tighter bound needs a new version literal.

### 3.1 Frozen bounds

| Bound | Value | Source |
| --- | --- | --- |
| Maximum turn input characters | 4000 | Apple FM IPC start-turn schema. |
| Maximum renderer-prepared input characters | 3900 | Renderer prompt cap. |
| Maximum turn output characters | 8192 | Apple FM result text bound. |
| Maximum context characters | 64000 | Provider-switch history budget. |
| Maximum turn event text characters | 32000 | Local turn journal text limit. |
| Maximum retained turn records | 128 | Local turn journal record limit. |
| Maximum assistant segments | 256 | Local turn journal segment limit. |
| Maximum context history messages | 32 | Provider-switch history messages. |
| Maximum blocker references | 8 | Apple FM IPC blocker-ref cap. |
| Maximum owner-bound candidates | 5 | Owner-bound provider set size. |

### 3.2 Owner-bound candidate set

The owner-bound provider candidate vocabulary is `apple_fm`, `codex`, `claude`,
`grok_acp`, and `cursor_acp`. Apple FM is a local advisory inference lane. Apple
FM is never an unrestricted provider. Policy selects only inside this set. A
recommendation cannot add a candidate to the set.

### 3.3 Selected and effective route data

The route decision records the selected lane, the effective lane, the admitted
candidate set, the policy artifact reference, the context manifest reference,
the disclosure, the decision reason, and every refused or skipped lane. A closed
decision has no selected lane and records the fail-closed reason.

### 3.4 Data-destination disclosures

The data destination is `on_device_local`, `remote_provider`, or
`openagents_managed_remote`. Apple FM keeps input on the device. A remote
provider discloses a remote destination. Local failure must not change a local
destination to a remote destination without a new admitted route.

### 3.5 Refusal and terminal states

The lifecycle states are `accepted`, `routing`, `dispatching`, `streaming`,
`completed`, `refused`, `failed`, and `cancelled`. The terminal states are
`completed`, `refused`, `failed`, and `cancelled`. The refusal reasons include
`route_closed_no_candidate`, `provider_unavailable`, `unsupported_platform`,
`not_ready`, `helper_missing`, `malformed_output`, `oversized_output`,
`empty_output`, `action_claim_rejected`, and `decode_failed`. A decode failure
never dispatches. An action-claim output never dispatches.

### 3.6 The eight distinct concepts

AFS-00 freezes the eight concepts as one typed vocabulary. Each concept is
distinct. A later concept never stands in for an earlier concept.

| Concept | Meaning |
| --- | --- |
| recommendation | An advisory model signal. It has no authority. |
| decision | A host-derived admitted route. Only the host creates it. |
| action | A real effect performed by an existing host service. |
| card | A bounded UI projection. It is display only. |
| evidence | A recorded lifecycle, decision, output, check, or receipt reference. |
| acceptance | An owner or admitted-policy disposition. It is not release. |
| delivery | A completed handoff of an accepted change or result. |
| release | An evidence-gated product transition. Only a release gate makes it. |

## 4. Import-boundary checks

AFS-00 adds `scripts/check-afs-boundaries.ts` and its test. The check mirrors
`apps/openagents-desktop/scripts/check-ide-boundaries.ts`. The check runs in the
normal test sweep and through `pnpm run check:afs-boundaries`.

The check enforces these rules for every new root package:

1. A root-core package must not import an app or nested-app code.
2. A root-core package must not import a platform API. The Apple FM `./node`
   subpath is the only place a Node API is admitted.
3. A root-core package must not import a provider SDK.
4. A root-core package must not import a SQL driver.
5. A root-core package must not import a cloud client.
6. `agent-turn-runtime` must not import `apple-fm-runtime` or `agent-turn-store`.
7. Every declared subpath export must resolve to an existing file.
8. The root export must be portable and must not resolve to a Node host file.
9. The package graph must have no import cycle.

## 5. Version-drift finding

AFS-00 records one intentional finding. The Swift `foundation-bridge` source
declares bridge version `0.1.3`. The Desktop staging pin declares `0.1.1`.
AFS-00 does not fix this drift. AFS-02 owns the fix.

`packages/apple-fm-runtime/src/index.ts` freezes the single wire-version source.
`scripts/check-afs-apple-fm-version-drift.ts` reads the two live sources and
reports the finding. AFS-02 must generate the native manifest, the Swift bridge
version, and the Desktop staging pin from the single source. AFS-02 must then
change the version-drift test to assert that the sources agree.

## 6. Baseline

AFS-00 captures the baseline with fixtures and tests.

`packages/agent-runtime-schema/src/afs-baseline-fixtures.ts` holds fixtures for a
local answer, standby, an explicit provider turn, malformed Apple FM output, a
helper failure, and an unavailable provider. Desktop, web, and mobile decode the
same safe projection fixtures to equivalent facts because the three surfaces
share one schema.

`apps/openagents-desktop/src/afs-baseline-regression.test.ts` proves two current
behaviors with existing code:

1. The current local chat does not dispatch a provider. The Apple FM local
   answer path runs through the host supervisor and its local helper session.
   The host module is structurally free of the provider-lane dispatcher.
2. The explicit provider path still works. A typed request runs through the real
   provider-lane dispatch engine to a completed journal disposition.

## 7. Migration map

This table records the disposition of each current in-scope file or tree. The
disposition is keep, extract, transition-adapter, evidence-only, or
retire-after-cutover. A transition adapter keeps a migration green. A transition
adapter is not the new authority.

| Current path | Disposition | Note |
| --- | --- | --- |
| `packages/agent-runtime-schema/` | keep | AFS-00 adds the frozen turn contract modules here. |
| `packages/provider-account-schema/` | keep | The account-reference schema authority. |
| `packages/portable-session-contract/` | keep | The placement and continuation authority. |
| `apps/openagents-desktop/src/apple-fm-host.ts` | transition-adapter | AFS-02 makes it a thin Desktop adapter over `apple-fm-runtime`. |
| `apps/openagents-desktop/src/apple-fm-native-helper.ts` | extract | AFS-02 moves helper discovery, verification, spawn, readiness, and shutdown to `apple-fm-runtime/node`. |
| `apps/openagents-desktop/src/apple-fm-contract.ts` | transition-adapter | The portable parts move to `apple-fm-runtime`. The Electron IPC contract stays in Desktop. |
| `apps/openagents-desktop/scripts/stage-target.ts` | transition-adapter | AFS-02 generates the staging version from the single wire-version source. |
| `apps/pylon/packages/runtime/src/backends/apple-fm/` | extract | AFS-02 moves the wire schemas, client, capability probe, and process control to `apple-fm-runtime`. |
| `apps/pylon/swift/foundation-bridge/` | extract | AFS-02 moves the Swift source to `apple-fm-runtime/native`. |
| `apps/openagents-desktop/src/provider-lane.ts` | transition-adapter | AFS-01 wraps the Desktop dispatch engine behind the shared turn kernel. |
| `apps/openagents-desktop/src/local-turn-journal.ts` | transition-adapter | AFS-01 wraps the local journal behind the driver-neutral store. |
| `apps/openagents-desktop/src/thread-store.ts` | transition-adapter | AFS-01 wraps the thread store behind the shared thread repository port. |
| `apps/openagents-desktop/src/renderer/shell.ts` standby path | retire-after-cutover | AFS-03 removes the `openAgentsStandby` branch and the renderer prompt builder after the rollback gate passes. |
| Desktop IDE contracts (`cursor-contract.ts`, `agent-code-contract.ts`, `project-contract.ts`) | extract | AFS-05 moves the portable IDE schemas to `ide-runtime`. |
| Historical `@openagentsinc/dse` package | evidence-only | AFS reimplements the useful DSE rules. It does not restore the old package. |
| Blueprint runtime copies in web, Pylon, and Probe | evidence-only | AFS reuses the governance patterns in provider-neutral contracts. These paths are not the target home. |

## 8. Verification

AFS-00 passes these checks:

1. The schema round-trip, invalid-input, size-bound, and compatibility tests
   pass.
2. The package-boundary, subpath-export, and import-cycle checks pass.
3. Desktop, web, and mobile decode the same safe fixtures to equivalent facts.
4. The baseline proves that current local chat does not dispatch a provider.
5. The baseline proves that the explicit provider path still works.
6. `pnpm --dir apps/openagents-desktop run typecheck` passes.
7. `pnpm --dir apps/openagents-desktop run check:ide-boundaries` passes.
8. Each new package typechecks.
