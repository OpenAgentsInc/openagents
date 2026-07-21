import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";
import { audioBehaviorContracts } from "./audio";

/**
 * Pending owner contracts for the greenfield OpenAgents mobile/desktop apps.
 * These live in the shared registry until each new app exists and can own an
 * enforced registry plus executable identity, security, and cross-device
 * oracles.
 */
export const openAgentsAppsContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    ...audioBehaviorContracts,
    {
      authorityBoundary:
        "This freezes the IDE-00 project, worktree, file, document, capability, navigation, and generation identities plus their executable fencing and shipped Files regressions. It does not claim Monaco, LSP, debugging, collaboration, Zed quality, Cursor parity, or packaged release acceptance; widgets, helpers, Git, terminals, and future native code remain projections or bounded capabilities beneath the Effect-owned graph.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_project_generation_fencing.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/project-contract.ts",
        "apps/openagents-desktop/src/ide/project-service.ts",
        "apps/openagents-desktop/src/ide/project-contract.test.ts",
        "apps/openagents-desktop/src/ide/project-service.test.ts",
        "apps/openagents-desktop/scripts/check-ide-boundaries.ts",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-00-baseline.json",
        "docs/ide/ROADMAP.md",
        "github:OpenAgentsInc/openagents#9015",
      ],
      oracles: [
        {
          description:
            "Decodes the schema-first graph; rejects invalid branded refs and invalid attachment/document/language/Git/index/placement generations; proves same-path worktrees remain isolated, attachment replacement clears stale document/navigation state, explicit stop closes access, and a late capability result cannot regain authority.",
          id: "openagents_desktop.ide_project_generation_fencing.graph",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/project-service.test.ts",
        },
        {
          description:
            "The shipped Files corpus proves editor-first system and tree opens, stable selection across refresh/rename/delete, dirty close, save/save-all, external conflicts, recovery, grant revocation, bounded paging/search/cancellation, Git review, typed terminal teardown, and the ordinary session/thread state surviving Files entry and exit.",
          id: "openagents_desktop.ide_project_generation_fencing.files_regression",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/renderer/workspace-editor.test.ts",
        },
        {
          description:
            "The complete shell journey proves the Files command enters/exits without replacing the selected thread/session, Finder/system-open reaches the editor-first route, tree selection survives refresh and rename, revocation closes stale document authority, and the loaded Pierre tree remains a projection of canonical refs.",
          id: "openagents_desktop.ide_project_generation_fencing.shell_journey",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
        },
        {
          description:
            "Mechanically rejects hand-mirrored boundary declarations, unchecked IDE authority casts, widget/native schema authority, and missing Effect service/layer/function/error/decode/finalizer primitives.",
          id: "openagents_desktop.ide_project_generation_fencing.architecture",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/check-ide-boundaries.ts",
        },
      ],
      productArea: "Desktop IDE authority foundation",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "The Desktop IDE uses one Effect-owned, schema-first, generation-fenced project graph across every open origin and lifecycle; editor entry preserves the current thread/session, equal relative paths in separate worktrees never alias, save/conflict/recovery and revocation stay honest, stopped or superseded capability work cannot regain authority, and the existing Files journey remains a permanent regression fixture.",
      surface: "openagents-desktop",
      verification:
        "The normal Desktop and behavior-contract sweeps run the project graph/service tests and the established workspace/editor/shell/browser/search/Git/terminal/recovery suites; check:ide-boundaries enforces the architecture mechanically, and ide-baseline emits public-safe p50/p95/p99 receipts with explicit unmeasured states.",
    },
    {
      authorityBoundary:
        "This contract owns exact agent attachment, context disclosure, proposal lifecycle, canonical apply/undo, backlinks, and host-observed post-image evidence inside the existing Effect IDE graph. A harness may propose but never writes files; the renderer, Monaco, Pierre, and native helpers remain projections without workspace authority. A harness completion or provider claim never becomes diagnostics, tests, Git delivery, verification, acceptance, or release evidence. ProductSpec/Desktop/Cursor AssuranceSpec criteria remain proposed until independent owner review.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_agent_native_code_graph.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/agent-code-contract.ts",
        "apps/openagents-desktop/src/ide/agent-code-service.ts",
        "apps/openagents-desktop/src/ide/agent-code-host.ts",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-agent-code.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-acceptance.json",
        "docs/ide/2026-07-19-ide-08-agent-native-code-graph.md",
        "github:OpenAgentsInc/openagents#9036",
      ],
      oracles: [
        {
          description:
            "Schema/service/host tests prove exact attachment and manifest accounting, semantic-retrieval-off utility, single/mixed/partial proposal decisions, stale/dirty/secret/private/binary/too-large/refused bases, compensating rollback, checkpoint undo, current/historical/unavailable backlinks, restart recovery, corrupt-state fencing, late-generation refusal, and public-safe receipts.",
          id: "openagents_desktop.ide_agent_native_code_graph.contract_service",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/agent-code-service.test.ts",
        },
        {
          description:
            "The production React host renders every included and omitted context item plus effective runtime policy, and projects exact-base agent proposals through the existing Pierre adapter with keyboard-operable per-operation accept/reject, canonical apply, checkpoint undo, evidence, lineage, and backlinks.",
          id: "openagents_desktop.ide_agent_native_code_graph.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/react-agent-code.test.tsx",
        },
        {
          description:
            "The real packaged darwin-arm64 application performs diagnostic → context disclosure → exact proposal → Pierre review → keyboard apply → host evidence → backlink → keyboard undo, while withholding the root and recording the exact artifact digest, screenshot, trace, and candidate SHA.",
          id: "openagents_desktop.ide_agent_native_code_graph.packaged",
          kind: "script",
          mode: "e2e",
          ref: "apps/openagents-desktop/scripts/ide-agent-code-packaged-journey.ts",
        },
        {
          description:
            "The benchmark and deterministic acceptance evaluator freeze p50/p95/p99 rows, fault/resource/offline corpora, architecture custody, public-material scans, artifact ancestry, reviewer non-overridability, rollback target, claimed platform, and all remaining IDE-09+ gaps.",
          id: "openagents_desktop.ide_agent_native_code_graph.acceptance",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-agent-code-acceptance.ts",
        },
      ],
      productArea: "Desktop IDE agent-native code graph",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "A coding session attaches to one exact project/worktree generation, discloses every included and omitted runtime context item, admits only version-bound proposals, reviews them in the shared Pierre Changes plane, applies/refuses/undoes through canonical workspace authority, maintains code↔turn backlinks, and reports host-observed post-image evidence separately from agent completion.",
      surface: "openagents-desktop",
      verification:
        "verify:ide-08 typechecks, runs targeted agent/context/proposal/review/host/accessibility/behavior tests, enforces Effect/widget authority boundaries, emits p50/p95/p99 and resource receipts, packages Desktop, drives the exact LaunchServices journey, and runs a deterministic public-safe acceptance oracle. The full normal Desktop sweep remains mandatory before landing.",
    },
    {
      authorityBoundary:
        "This admits exact Monaco and Pierre Diffs artifacts plus an owned local theme registry and a first-party public-Monaco Vim contract. Khala editor is the fixed default and Tokyo Night the retained fallback. It does not make a widget canonical for files, grants, Git, processes, approvals, persistence, proposals, or receipts; it does not ship the opt-in admission fixture on the ordinary renderer path; and it does not admit remote or executable themes.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_package_admission.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/package-admission.ts",
        "apps/openagents-desktop/src/ide/package-admission.test.ts",
        "apps/openagents-desktop/src/ide/pierre-diffs-adapter.tsx",
        "apps/openagents-desktop/src/ide/tokyo-night-theme.ts",
        "apps/openagents-desktop/src/ide/khala-editor-theme.ts",
        "apps/openagents-desktop/src/ide/desktop-editor-themes.ts",
        "apps/openagents-desktop/src/ide/vim-mode-contract.ts",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-package-spike.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-package-audit.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-typescript-index.json",
        "docs/ide/2026-07-19-ide-01-package-admission.md",
        "github:OpenAgentsInc/openagents#9016",
      ],
      oracles: [
        {
          description:
            "Decodes every package/Vim/theme/projection decision, proves exact production pins, rejects both evaluated third-party Vim adapters, keeps authority fields outside the Pierre projection, checks Khala-default/Tokyo-fallback accessibility and provenance, and validates all generated receipts.",
          id: "openagents_desktop.ide_package_admission.contract",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/package-admission.test.ts",
        },
        {
          description:
            "Builds an isolated ESM graph and runs real Electron against development and ASAR layouts for three create/dispose cycles plus an injected worker failure; all Monaco language workers and the Pierre worker stay offline on the private scheme and teardown reaches zero tracked workers/models.",
          id: "openagents_desktop.ide_package_admission.package_smoke",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-package-spike-smoke.ts",
        },
        {
          description:
            "Measures normal boot, lazy fixture, source-map, package, and worker bytes; proves the fixture graph is opt-in, ordinary boot has no editor package markers, CSP has worker-src self and no unsafe-eval, and package/module chunks remain attributable.",
          id: "openagents_desktop.ide_package_admission.bundle_audit",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-package-audit.ts",
        },
        {
          description:
            "Measures a TypeScript path index on 10,000 files across initial scan, indexed query, 1,000-event churn, and real watcher latency, enforcing explicit p95 budgets and recording why speculative Rust placement is rejected until evidence crosses a written gate.",
          id: "openagents_desktop.ide_package_admission.typescript_index",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-typescript-index-benchmark.ts",
        },
      ],
      productArea: "Desktop IDE package and runtime admission",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "The Desktop IDE admits exact, attributable Monaco and Pierre Diffs packages only behind replaceable projection adapters; workers load offline under restrictive CSP in development and ASAR layouts, disposal and failure paths leak no tracked runtime state, Tokyo Night projects from one owned accessible token source, Vim is an off-by-default complete first-party contract, and TypeScript remains the project-index placement until measured evidence justifies Rust.",
      surface: "openagents-desktop",
      verification:
        "The Desktop test sweep runs the admission contract. ide:package-spike, ide:package-audit, and ide:typescript-index-benchmark regenerate schema-decoded public-safe receipts; the ordinary build excludes the opt-in package fixture and its large attribution-only source-map/catalog closure.",
    },
    {
      authorityBoundary:
        "This freezes the complete Explorer index, reconciliation, interaction, and typed operation path. The workspace/project services remain the only root, grant, filesystem, watcher, mutation, and persistence authority; Pierre receives bounded relative nodes and emits typed intent only. It does not claim Monaco, LSP, a remote index, recursive delete, arbitrary symlink traversal, or a Rust project database.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_complete_path_index.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/path-index-contract.ts",
        "apps/openagents-desktop/src/ide/path-index-service.ts",
        "apps/openagents-desktop/src/ide/path-index-service.test.ts",
        "apps/openagents-desktop/src/renderer/ide/pierre-tree-adapter.tsx",
        "apps/openagents-desktop/src/renderer/workspace-browser.ts",
        "apps/openagents-desktop/tests/workspace-service.test.ts",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-02-path-index.json",
        "docs/ide/2026-07-19-ide-02-complete-explorer.md",
        "github:OpenAgentsInc/openagents#9017",
      ],
      oracles: [
        {
          description:
            "Proves chunked complete/lazy scans, explicit partial/ready states, exact multi-root/worktree isolation, stable identity through expected-version rename, stale badge rejection, truthful filtering, incremental watcher reconciliation, overflow rescan, cancellation fencing, and scoped zero-resource teardown.",
          id: "openagents_desktop.ide_complete_path_index.service",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/path-index-service.test.ts",
        },
        {
          description:
            "Proves the production Files driver builds Pierre input from the complete index, preserves the legacy tree/search/editor loop, reconciles watchers outside mounted pages, and dispatches mutations only through typed expected-version bridge requests.",
          id: "openagents_desktop.ide_complete_path_index.files_driver",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/renderer/workspace-browser.test.ts",
        },
        {
          description:
            "Proves move/copy/duplicate/create/rename/delete/reveal enforce relative-root, revision, ignore, collision, non-recursive delete, permission, symlink, and revocation policy without leaking the selected absolute root.",
          id: "openagents_desktop.ide_complete_path_index.workspace_authority",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/tests/workspace-service.test.ts",
        },
        {
          description:
            "Measures large-index initial and cached projection traversal plus incremental update latency at p50/p95/p99, node/byte/handle budgets, deterministic disposal, and TypeScript-versus-Rust placement evidence.",
          id: "openagents_desktop.ide_complete_path_index.scale",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-path-index-benchmark.ts",
        },
      ],
      productArea: "Desktop IDE Explorer",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "The Desktop Explorer is fed by one complete Effect-owned generation-fenced path index rather than mounted pages; large and equal-path worktrees stay isolated, incomplete/degraded states are explicit, pointer/keyboard focus and reveal survive reconciliation, badges reject stale generations, every file action is an expected-version typed command, and Pierre has zero filesystem or mutation authority.",
      surface: "openagents-desktop",
      verification:
        "The normal Desktop and behavior-contract sweeps run the index/service, Files driver, Pierre adapter, workspace authority, command, shell, accessibility, and teardown regressions; ide:path-index-benchmark regenerates the public-safe scale receipt, and check:ide-boundaries enforces schema-first Effect authority and widget isolation.",
    },
    {
      authorityBoundary:
        "This makes Monaco the production editing mechanic behind one schema-first, Effect-owned document state. The current owner-selected Khala editor projection is the mounted default and Tokyo Night remains a built-in fallback; neither permits arbitrary theme installation. Monaco receives only opaque document/view refs, relative display labels, bounded text/options, and typed callbacks; it receives no root, filesystem grant, preload bridge, Git/process/network/persistence authority, or canonical success claim. Vim is a first-party controller over public Monaco commands, not a second document model. The legacy CodeEditor stub remains only in the explicitly named compatibility renderer and tests. This does not claim debugging, collaboration, remote editing, or arbitrary theme installation.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_monaco_document_runtime.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/monaco-document-contract.ts",
        "apps/openagents-desktop/src/ide/editor-runtime-entry.ts",
        "apps/openagents-desktop/src/ide/khala-editor-theme.ts",
        "apps/openagents-desktop/src/ide/desktop-editor-themes.ts",
        "apps/openagents-desktop/src/ide/monaco-runtime-loader.ts",
        "apps/openagents-desktop/src/renderer/monaco-editor-host.tsx",
        "apps/openagents-desktop/src/renderer/workspace-editor.ts",
        "apps/openagents-desktop/src/renderer/react-workspace-surfaces.tsx",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-monaco.json",
        "docs/ide/2026-07-19-ide-03-monaco-vim-tokyo-night.md",
        "github:OpenAgentsInc/openagents#9018",
      ],
      oracles: [
        {
          description:
            "Decodes opaque document/generation/sequence/view identities, edit/selection/save/close events, Vim projection, attach inputs, and resource receipts; proves equal relative paths under different grants never alias and mechanically pins the lazy offline private-scheme island with no host authority.",
          id: "openagents_desktop.ide_monaco_document_runtime.contract",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/monaco-document-contract.test.ts",
        },
        {
          description:
            "Proves document identity survives rename/Save As, generations reject stale events, monotonic edits recover explicit sequence gaps from complete snapshots, recovery v2/v3 migrates to the v4 workbench snapshot, dirty/save/conflict/undo/find remain canonical, and Vim/split/tabs/groups preserve drafts and durable settings.",
          id: "openagents_desktop.ide_monaco_document_runtime.state",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/renderer/workspace-editor.test.ts",
        },
        {
          description:
            "The production React Files surface contains Monaco hosts and no legacy textarea, exposes keyboard-named wrap/minimap/split/Vim/save controls, and routes tree/file opens and editor commands through the same typed intent registry.",
          id: "openagents_desktop.ide_monaco_document_runtime.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
        },
        {
          description:
            "Builds the production editor island, audits ordinary boot isolation and fixed private-scheme assets, then runs the packaged Electron Files journey through a real Monaco edit, Vim toggle, split view, recovery reload, screenshot, CSP/offline check, and zero-resource teardown.",
          id: "openagents_desktop.ide_monaco_document_runtime.packaged",
          kind: "script",
          mode: "e2e",
          ref: "apps/openagents-desktop/scripts/ide-monaco-packaged-journey.ts",
        },
        {
          description:
            "Measures schema/reducer open, incremental edit, gap resync, multi-tab, recovery encode/decode, and teardown paths at p50/p95/p99 with explicit TypeScript placement and memory/asset receipts.",
          id: "openagents_desktop.ide_monaco_document_runtime.scale",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-monaco-benchmark.ts",
        },
      ],
      productArea: "Desktop IDE editor",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "The production Desktop editor uses lazy offline Monaco instead of a textarea while Effect-owned schema-derived document state remains canonical; opaque grant-scoped identity survives path changes, generations and sequences fence stale events, edits/save-all/conflicts/recovery/split views stay coherent, Vim is built in and durable but off by default, and the Khala editor projection is installed before the native window, workbench, Monaco, Pierre, and terminal paint while Tokyo Night remains a validated fallback.",
      surface: "openagents-desktop",
      verification:
        "The normal Desktop and behavior-contract sweeps run the contract/state/DOM/accessibility/preferences/command/boundary suites. verify:ide-03 rebuilds and audits the lazy production island, emits a schema-decoded public-safe benchmark receipt, executes the packaged Electron journey, and requires all Monaco models, views, workers, and listeners to reach zero after disposal.",
    },
    {
      authorityBoundary:
        "This contract owns daily workbench projection and orchestration only. The path index remains the source of admitted project/root/worktree identity; the document reducer remains canonical for bytes, dirty state, and recovery; the workspace service remains the only filesystem authority; Monaco receives validated display/options data only; and Pierre receives a bounded projection plus typed commands. Symbol results are populated only by IDE-06 exact-generation receipts, Khala editor is the fixed default with Tokyo Night retained as fallback, and neither React nor these workbench schemas acquire roots, filesystem callbacks, process authority, arbitrary theme input, or arbitrary Monaco contribution access.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_daily_workbench.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/workbench-contract.ts",
        "apps/openagents-desktop/src/desktop-command-contract.ts",
        "apps/openagents-desktop/src/renderer/workspace-editor.ts",
        "apps/openagents-desktop/src/renderer/react-workspace-surfaces.tsx",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-04-workbench.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-04-packaged-workbench.json",
        "docs/ide/2026-07-19-ide-04-daily-workbench.md",
        "github:OpenAgentsInc/openagents#9019",
      ],
      oracles: [
        {
          description:
            "Decodes and exercises exact worktree/document navigation, stale/unavailable history, bounded deterministic Quick Open, honest Outline/breadcrumbs, two-layer settings precedence, schema-valid import/export, and allowlisted Monaco option projection.",
          id: "openagents_desktop.ide_daily_workbench.contract",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/workbench-contract.test.ts",
        },
        {
          description:
            "Proves preview/pinned/reordered/dirty-guarded tabs, shared-model split groups, closed-tab stack, recovery v4 migration, external rename/conflict behavior, and settings-to-document coherence through the Effect-owned editor reducer and intent handlers.",
          id: "openagents_desktop.ide_daily_workbench.state",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/renderer/workspace-editor.test.ts",
        },
        {
          description:
            "Exercises the one stable Desktop command registry, durable binding store, visible exact-conflict removal, platform/context/source metadata, Vim precedence labels, typed Pierre file commands, and keyboard/screen-reader workbench controls.",
          id: "openagents_desktop.ide_daily_workbench.commands_dom",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
        },
        {
          description:
            "Measures Quick Open over 50,000 admitted relative paths and 10,000 bounded navigation pushes at p50/p95/p99, and extends the packaged LaunchServices journey through Quick Open, preview-to-pin, split, recovery v4, root withholding, and teardown.",
          id: "openagents_desktop.ide_daily_workbench.scale_packaged",
          kind: "script",
          mode: "e2e",
          ref: "apps/openagents-desktop/scripts/ide-workbench-benchmark.ts",
        },
      ],
      productArea: "Desktop IDE daily workbench",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "Desktop provides one schema-first daily IDE workbench: exact project/root/worktree/document navigation and explicit stale history; bounded Quick Open, search-origin reuse, breadcrumbs, and honest symbol placeholders; preview/pinned tabs, dirty close operations, closed-tab recovery, shared-document split groups, and restart restore; one stable command and durable keybinding graph with visible source/context/platform/Vim precedence; complete typed grant-scoped Explorer operations; and bounded default/user/workspace editor settings projected through an allowlist into Monaco with Khala editor fixed as default and Tokyo Night retained as fallback.",
      surface: "openagents-desktop",
      verification:
        "verify:ide-04 builds Desktop, emits the workbench benchmark, typechecks, runs contract/editor/browser/Pierre/command/binding/DOM/accessibility/behavior tests, and checks IDE boundaries. The packaged LaunchServices journey additionally proves Quick Open, preview/pin, split, recovery v4, fixed private-scheme Monaco, and root withholding in the signed-shape application artifact.",
    },
    {
      authorityBoundary:
        "This contract owns review source identity, lifecycle, version fencing, bounded disclosure, and projection into @pierre/diffs. Git review remains read-only; IDE-08 owns real agent proposal generation and application. Pierre receives patch/display data and emits bounded intent only: it never receives a root, grant, preload bridge, Git/filesystem/process function, document mutation, policy, approval, persistence, or durable-success authority. Accept/reject/apply/undo dispositions are typed canonical commands that still require the owning document/workspace/checkpoint/proposal service to revalidate and execute.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_versioned_pierre_review.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/project-contract.ts",
        "apps/openagents-desktop/src/ide/review-contract.ts",
        "apps/openagents-desktop/src/ide/pierre-diffs-adapter.tsx",
        "apps/openagents-desktop/src/renderer/react-review.tsx",
        "apps/openagents-desktop/src/renderer/react-workspace-surfaces.tsx",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-05-review.json",
        "docs/ide/2026-07-19-ide-05-versioned-pierre-review.md",
        "github:OpenAgentsInc/openagents#9020",
      ],
      oracles: [
        {
          description:
            "Decodes eight distinct HEAD/index/worktree, saved/draft, conflict, checkpoint, proposal, and candidate schema variants with exact refs, generations, content safety, lifecycle, and allowed actions; proves stale and unavailable refusals plus exact-generation canonical command routing.",
          id: "openagents_desktop.ide_versioned_pierre_review.contract",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/review-contract.test.ts",
        },
        {
          description:
            "Exercises exact host repository/status fences, binary/secret/large/invalid-path refusals, stale refresh, selected bounded composer disclosure, and monotonic renderer snapshot replacement through the Git contract/host/panel corpus.",
          id: "openagents_desktop.ide_versioned_pierre_review.git_fencing",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/git-review-corpus.node.test.ts",
        },
        {
          description:
            "Mounts the production review workspace through the owned Pierre adapter with explicit base/target/source labels, unified/split controls, context bounds, line selection, annotations, open-in-editor, disclosure actions, non-color semantics, the Khala editor default plus registered Tokyo Night fallback, and no custom React hunk renderer.",
          id: "openagents_desktop.ide_versioned_pierre_review.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/react-primitive-adapters.test.tsx",
        },
        {
          description:
            "Projects and parses every source class plus 500 files at p50/p95/p99, proves 99 superseded updates cannot commit, repeats 200 open/close cycles with workers disabled and zero listener/worker residue, and records an offline schema-decoded receipt.",
          id: "openagents_desktop.ide_versioned_pierre_review.scale",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-review-benchmark.ts",
        },
      ],
      productArea: "Desktop IDE review",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "Desktop renders every admitted comparison through one app-owned @pierre/diffs adapter over a schema-first versioned source: Git HEAD/index/worktree, saved/draft, external conflict, checkpoint/current, exact-base agent proposal, and candidate A/B remain semantically distinct; explicit project/root/worktree/file/document refs, base/target versions and generations, encoding/EOL/content-safety state, origin, actions, and ready/stale/unavailable lifecycle prevent text-label authority; selected bounded context and annotations are keyboard/screen-reader legible; and any mutating action revalidates exact generations before dispatching a canonical typed command, while stale or unavailable bases refuse.",
      surface: "openagents-desktop",
      verification:
        "verify:ide-05 builds Desktop, emits and decodes the 500-file benchmark/teardown receipt, typechecks, runs review/project/Git/host/refusal/Pierre/DOM/accessibility/behavior/boundary suites, and checks IDE boundaries. The packaged offline journey renders the eight-source fixture corpus from the ASAR with the owned local theme registry and no remote theme/code dependency before recording source-class and teardown evidence.",
    },
    {
      authorityBoundary:
        "This owns two explicit language tiers: lazy packaged Monaco workers provide document-local mechanics inside the replaceable editor island, while one Effect-owned project-local TypeScript service owns project intelligence, lifecycle, cancellation, restart, and exact evidence. The renderer receives only schema-decoded relative paths, opaque refs, generations, status, bounded items, and receipts; it never receives the root, process/worker handles, or language-service authority. Unsupported documents and chat-only sessions start no project language process. This does not admit cloud intelligence, embeddings, DAP, tasks, tests, or AI-generated edits.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_generation_safe_language.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/language-contract.ts",
        "apps/openagents-desktop/src/ide/language-service.ts",
        "apps/openagents-desktop/src/ide/language-utility-worker.ts",
        "apps/openagents-desktop/src/ide/language-workbench-contract.ts",
        "apps/openagents-desktop/src/renderer/react-workspace-surfaces.tsx",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-06-language.json",
        "docs/ide/2026-07-19-ide-06-generation-safe-language.md",
        "github:OpenAgentsInc/openagents#9021",
      ],
      oracles: [
        {
          description:
            "Decodes every first-corpus capability, tagged service/result/item/failure state, exact project/root/worktree/attachment/language/document/service generation, document-local versus project-local evidence tier, and root-redacted renderer projection.",
          id: "openagents_desktop.ide_generation_safe_language.contract",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/language-contract.test.ts",
        },
        {
          description:
            "Proves lazy startup, persistent project service, supersession, timeout, malformed response, stale-result stripping, supervised crash/restart generation advance, stop, and zero pending resources through typed Effect errors and service state.",
          id: "openagents_desktop.ide_generation_safe_language.service",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/language-service.test.ts",
        },
        {
          description:
            "Runs the real bundled TypeScript 6.0.3 utility over 151 files, records first/warm diagnostics and document-symbol p50/p95/p99, schedules 100 superseding requests with exactly one commit, kills and recovers the provider at a new service generation, and stops with zero workers or pending requests.",
          id: "openagents_desktop.ide_generation_safe_language.scale_restart",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-language-benchmark.ts",
        },
        {
          description:
            "The production workbench projects one exact receipt set into Monaco diagnostics/semantic styling/inlays/folds, Problems, Outline, breadcrumbs, definitions, references, rename/format/code-action previews, and canonical edit dispatch while explicitly labeling both language tiers and refusing older model versions.",
          id: "openagents_desktop.ide_generation_safe_language.workbench",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/ide/language-workbench-contract.test.ts",
        },
      ],
      productArea: "Desktop IDE language intelligence",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "Desktop has generation-safe local language intelligence with two visible tiers: lazy document-local Monaco JSON/CSS/HTML/TypeScript workers and a separately supervised Effect-owned project TypeScript service. The project tier preserves persistent language state, exact provider/version/placement and generation evidence, bounded queues/timeouts/cancellation/restart/teardown, the complete first capability corpus, shared Problems/Outline/breadcrumb/navigation/editor projections, and stale-result refusal; unsupported documents and non-IDE sessions pay zero project-process cost.",
      surface: "openagents-desktop",
      verification:
        "verify:ide-06 rebuilds both worker tiers, emits the schema-decoded real-worker benchmark and restart receipt, typechecks, runs language/service/workbench/Monaco/editor/Electron/accessibility/behavior suites, and checks schema-first Effect and widget authority boundaries.",
    },
    {
      authorityBoundary:
        "This is a release-evidence contract, not a new project, document, Git, language, editor, review, process, persistence, approval, or claim authority. It admits one exact packaged macOS arm64 artifact only after schema decoding, child-evidence ancestry, frozen budget comparison, chat-only zero-cost inspection, the existing Effect boundary oracle, rollback corpus, public-safety checks, and a deterministic non-overridable evaluator all pass. Every untested platform and every IDE-08+ capability remains explicitly unavailable.",
      blockerRefs: [],
      contractId: "openagents_desktop.ide_basic_ide_acceptance.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/ide/basic-ide-acceptance-contract.ts",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-acceptance.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-packaged-basic-ide.json",
        "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-chat-only.json",
        "docs/ide/2026-07-19-ide-07-basic-ide-acceptance.md",
        "github:OpenAgentsInc/openagents#9022",
      ],
      oracles: [
        {
          description:
            "Schema-decodes the exact artifact/SHA, seven child packets, all fifteen journey classes, frozen p50/p95/p99 budgets, one claimed target, five unavailable targets, architecture/custody checks, rollback target, later gaps, and non-overridable evaluator disposition.",
          id: "openagents_desktop.ide_basic_ide_acceptance.contract",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/ide/basic-ide-acceptance-contract.test.ts",
        },
        {
          description:
            "Launches the real packaged application through LaunchServices with a supported file and proves Explorer, Tokyo Night Monaco, quick open, edit/recovery, Vim, split groups, Pierre review, both language tiers, Problems, Outline, offline private assets, root withholding, and zero Monaco resources after close.",
          id: "openagents_desktop.ide_basic_ide_acceptance.packaged",
          kind: "script",
          mode: "e2e",
          ref: "apps/openagents-desktop/scripts/ide-monaco-packaged-journey.ts",
        },
        {
          description:
            "Runs seven cold packaged chat-only launches and rejects any Monaco/Pierre/language/index surface, editor asset request, renderer worker, root leak, or surviving app process while recording shell-ready p50/p95/p99.",
          id: "openagents_desktop.ide_basic_ide_acceptance.chat_only",
          kind: "script",
          mode: "e2e",
          ref: "apps/openagents-desktop/scripts/ide-chat-only-packaged-journey.ts",
        },
        {
          description:
            "Independently recomputes the packaged app tree digest, checks receipt/SHA agreement and closed child issues, freezes IDE-00 comparison envelopes plus packet-owned budgets, reruns the authority oracle, rejects public-root leakage, and emits the only basic-IDE claim receipt.",
          id: "openagents_desktop.ide_basic_ide_acceptance.release_oracle",
          kind: "script",
          mode: "headless",
          ref: "apps/openagents-desktop/scripts/ide-basic-ide-acceptance.ts",
        },
      ],
      productArea: "Desktop IDE release acceptance",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-19",
      },
      state: "enforced",
      statement:
        "The exact evaluated macOS arm64 artifact satisfies the daily-use OpenAgents basic IDE rung across Finder open, complete Explorer, Monaco editing/recovery, search/navigation, versioned Pierre review, generation-safe language intelligence, built-in Vim, keyboard/accessibility, offline failure truth, resource disposal, rollback, and chat-only zero-cost gates. This admits only OpenAgents basic IDE; IDE-08+ and every unevaluated platform remain visible gaps, and issue closure does not imply Zed quality, full IDE, Cursor parity, or epic owner acceptance.",
      surface: "openagents-desktop",
      verification:
        "verify:ide-07 runs the complete Desktop test corpus, Effect boundary oracle, and deterministic artifact/evidence/budget evaluator after the exact packaged editor and seven-launch chat-only journeys have produced SHA-bound receipts.",
    },
    {
      authorityBoundary:
        "This binds the visible activity indicator to the renderer's existing working phase and valid browser animation state. It does not redefine turn, queue, retry, or completion authority, and reduced-motion preference intentionally replaces motion with a stable visible indicator.",
      blockerRefs: [],
      contractId: "openagents_desktop.working_indicator_continuous_motion.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/renderer/react-timeline.tsx",
        "packages/ui/src/desktop-workbench.css",
        "apps/openagents-desktop/tests/working-indicator-motion.e2e.test.ts",
        "github:OpenAgentsInc/openagents#8930",
      ],
      oracles: [
        {
          description:
            "Real Chromium DOM/WAAPI oracle: while the working indicator is mounted, all three bars have the named animation, remain in running play state, and advance between two samples; under prefers-reduced-motion they remain visibly mounted while animation is absent.",
          id: "openagents_desktop.working_indicator.motion_runtime",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-desktop/tests/working-indicator-motion.e2e.test.ts",
        },
      ],
      productArea: "Desktop conversation activity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-16",
      },
      state: "enforced",
      statement: "[working] bars stopped animating — fix it, in ~/work/openagents",
      surface: "openagents-desktop",
      verification:
        "The normal Desktop test sweep runs working-indicator-motion.e2e.test.ts against the production shared CSS in real Chromium, asserting accessible working state, three live advancing animations, and the explicit reduced-motion exception.",
    },
    {
      authorityBoundary:
        "This selects the product shell and host; it does not authorize release before #8574's signing, security, migration, and clean-machine gates pass.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8574"],
      contractId: "openagents_apps.greenfield_desktop_electron.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
        "docs/sol/issues/app-desktop.md",
      ],
      oracles: [
        {
          description:
            "Planned scaffold and security oracle proving Electron, Effect Native, and no legacy Electrobun app import.",
          id: "openagents_desktop.greenfield_electron.planned",
          kind: "planned",
          mode: "headless",
          ref: "github:OpenAgentsInc/openagents#8574",
        },
      ],
      productArea: "desktop application architecture",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        "Deprecate the Khala Code Desktop electrobun app and mobile app. I want a new OpenAgents desktop app to be Electron.",
      surface: "openagents-desktop",
      verification:
        "Pending #8574: verify the new app root uses Electron + Effect Native, passes the secure IPC oracle, and does not import or release the deprecated Electrobun client.",
    },
    {
      authorityBoundary:
        "Template selection does not authorize copying unsafe defaults, retaining a second UI architecture, or publishing against the template owner's update repository.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8574"],
      contractId: "openagents_apps.desktop_starting_template.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/LuanRoger/electron-shadcn",
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
        "docs/sol/issues/app-desktop.md",
      ],
      oracles: [
        {
          description:
            "Planned provenance oracle for the pinned template commit plus required security and Effect Native adaptations.",
          id: "openagents_desktop.template_provenance.planned",
          kind: "planned",
          mode: "headless",
          ref: "github:OpenAgentsInc/openagents#8574",
        },
      ],
      productArea: "desktop scaffold provenance",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        "Update anything re desktop relevant to reference https://github.com/LuanRoger/electron-shadcn as the starting template the desktop app must use.",
      surface: "openagents-desktop",
      verification:
        "Pending #8574: the new app records its imported electron-shadcn commit and MIT attribution, retains the Forge/Vite/fuse/test bootstrap, removes the template updater/publisher wiring, asserts nodeIntegration=false and sandbox=true, verifies packaged fuses, and mechanically replaces starter application semantics with Effect Native/Effect Schema.",
    },
    {
      authorityBoundary:
        "This fixes app identity and icon selection; it does not claim repository proof of the existing store records or authorize upload before owner/store verification.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8597"],
      contractId: "openagents_apps.greenfield_mobile_identity.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
        "docs/sol/issues/app-mobile.md",
        "apps/openagents-mobile/assets/images/icon.png",
      ],
      oracles: [
        {
          description:
            "Planned app-config and asset-digest oracle for name, iOS/Android identifiers, and copied icon.",
          id: "openagents_mobile.identity_icon.planned",
          kind: "planned",
          mode: "headless",
          ref: "github:OpenAgentsInc/openagents#8597",
        },
      ],
      productArea: "mobile application identity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        'the mobile app -- which should be also built from scratch -- must use the existing app identifier "com.openagents.app" (it\'s called "OpenAgents") and that should use the same app icon Khala Code mobile now does.',
      surface: "openagents-mobile",
      verification:
        "Pending #8597: assert display name OpenAgents, iOS bundle identifier and Android application ID com.openagents.app, and copied icon SHA-256 0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce.",
    },
    {
      authorityBoundary:
        "Capability folding preserves typed authority boundaries; Sarah does not inherit provider credentials, payment authority, or raw private worker events.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8566",
        "github:OpenAgentsInc/openagents#8574",
        "github:OpenAgentsInc/openagents#8597",
      ],
      contractId: "openagents_apps.sarah_first_khala_capabilities.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/MASTER_ROADMAP.md",
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
      ],
      oracles: [
        {
          description: "Planned capability-disposition and cross-device Sarah/FleetRun oracle.",
          id: "openagents_apps.sarah_khala_folding.planned",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8566",
        },
      ],
      productArea: "Sarah-first product consolidation",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "retired",
      statement: "All Khala Code ideas are to be folded into the Sarah-first OpenAgents app.",
      surface: "openagents-mobile-and-desktop",
      verification:
        "Retired by the 2026-07-10 owner decision that removed Sarah as a product surface. The preserved capability-disposition requirement continues under openagents_apps.desktop_runtime_and_early_mobile_sync.v1 and MASTER_ROADMAP R0–R7.",
    },
    {
      authorityBoundary:
        "This fixes Desktop process/data boundaries and makes early mobile continuation part of the first real conversation exit. It does not authorize renderer-held credentials, mobile local-filesystem or shell authority, a second Pylon/run universe, optimistic completion claims, or release before R7.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8574", "github:OpenAgentsInc/openagents#8597"],
      contractId: "openagents_apps.desktop_runtime_and_early_mobile_sync.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-10-openagents-desktop-product-architecture.md",
        "docs/sol/2026-07-10-r1-r2-identity-sync-contract.md",
        "docs/sol/MASTER_ROADMAP.md",
        "docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md",
        "docs/sol/issues/native-streamed-conversation-handoff.md",
        "packages/khala-sync-server/src/runtime-mutators.test.ts",
        "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
        "apps/openagents-mobile/tests/mobile-conversation.test.ts",
      ],
      oracles: [
        {
          description:
            "Planned cross-client oracle: a tokenless Desktop renderer drives one real streamed durable thread through the host-owned runtime gateway; mobile observes matching thread/message refs, versions, phases, and terminal outcome, submits one safe follow-up or interrupt, and both clients reconcile across restart, revocation, cursor gap, duplicate delivery, and a lost acknowledgement without invented completion.",
          id: "openagents_apps.desktop_runtime_mobile_sync.planned",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8574",
        },
      ],
      productArea: "Desktop runtime architecture and cross-device continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "pending",
      statement:
        "Let's get the desktop architecture dialed in solidly in place, with mobile sync working soon in that process but otherwise plan to get your planned openagents product adaptation working fastest.",
      surface: "openagents-mobile-and-desktop",
      verification:
        "The deterministic #8676 slice now enforces exact durable runtime→agent-run binding, protocol-v6 tokenless Desktop projection, same-thread mobile start/follow-up/interrupt, restart reconstruction, and revoke-without-replay. This program contract remains pending until the public-safe live receipt proves one named isolated provider account in built Electron and one physical mobile continuation.",
    },
    {
      authorityBoundary:
        "Remote-first binds durable session identity and fenced checkpoint/rehydrate movement. It does not promise transparent migration of process memory, PTYs, sockets, provider hidden state, raw host paths, or credentials, and it does not upload a local-only session until the owner explicitly adopts it.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8566",
        "github:OpenAgentsInc/openagents#8574",
        "github:OpenAgentsInc/openagents#8597",
        "github:OpenAgentsInc/openagents#8746",
        "github:OpenAgentsInc/openagents#8748",
        "github:OpenAgentsInc/openagents#8749",
        "github:OpenAgentsInc/openagents#8753",
      ],
      contractId: "openagents_apps.remote_first_portable_sessions.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/MASTER_ROADMAP.md",
        "docs/sol/issues/native-streamed-conversation-handoff.md",
      ],
      oracles: [
        {
          description:
            "Planned cross-host oracle: quiesce and checkpoint one durable session, fence its source attachment, rehydrate it on a compatible local or remote target under the same session/thread/run/WorkContext refs, and prove one live generation, exact repository post-image, fresh target grants, source cleanup, and idempotent failure/failback outcomes.",
          id: "openagents_apps.remote_first_portable_sessions.planned",
          kind: "planned",
          mode: "e2e",
          ref: "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        },
      ],
      productArea: "portable coding-session authority",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "pending",
      statement:
        "Remote-first, not local-first. Sessions can be stopped on any machine and moved to any other, local or remote. i.e. handoff to cloud.",
      surface: "openagents-mobile-desktop-pylon-cloud",
      verification:
        "PORT-00 #8745 freezes the executable schema/model boundary. PORT-01–PORT-08 #8746–#8753 remain pending for durable authority and real local-to-managed-to-owner-remote acceptance.",
    },
    {
      authorityBoundary:
        "This enforced contract freezes only the portable-session vocabulary, schemas, cross-record invariants, command parity, and real-host journey falsifiers. It grants no persistence, dispatch, broker redemption, target compatibility, movement, mobile control, or product acceptance authority.",
      blockerRefs: [],
      contractId: "openagents_apps.portable_session_contract_freeze.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/portable-session-contract/src/index.ts",
        "packages/portable-session-contract/src/model.ts",
        "packages/portable-session-contract/src/journeys.ts",
        "packages/portable-session-contract/src/portable-session-contract.test.ts",
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "github:OpenAgentsInc/openagents#8745",
      ],
      oracles: [
        {
          description:
            "Decodes the versioned public-safe schemas and rejects host-derived identity, graph flattening/leakage, two live attachments, incomplete descendant fencing, stale commands, secret/process checkpoint state, and silent target changes; also freezes the real-host journey and its first-paint/action-parity falsifiers.",
          id: "openagents_apps.portable_session_contract_freeze",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/portable-session-contract/src/portable-session-contract.test.ts",
        },
      ],
      productArea: "portable coding-session contract and invariant boundary",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "enforced",
      statement:
        "Portable coding sessions use owner-minted host-independent identity, a canonical nested graph with independent cursors, graph-wide generation fencing, secret-free content-addressed checkpoints, provider-neutral targets, target-scoped capability refs, shared typed movement commands, detail-independent first paint, and identical pointer/tap/key action semantics.",
      surface: "openagents-mobile-desktop-pylon-cloud",
      verification:
        "pnpm exec vp test --cwd packages/portable-session-contract and pnpm exec vp test --cwd packages/behavior-contracts run the executable PORT-00 contract/model and registry coverage oracles.",
    },
    {
      authorityBoundary:
        "The target contract authorizes only owner-scoped execution through declared capabilities and isolation. It does not make an owner's homelab public capacity, let clients call vendor APIs, accept an unaudited provider, or silently substitute provider, custody, account, region, data posture, or isolation rung.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8547",
        "github:OpenAgentsInc/openagents#8636",
        "github:OpenAgentsInc/openagents#8749",
        "github:OpenAgentsInc/openagents#8750",
      ],
      contractId: "openagents_cloud.user_or_managed_execution_targets.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/issues/fc-cloud-codex.md",
        "docs/sol/issues/fc-4-hybrid-cloud.md",
        "docs/cloud/ARCHITECTURE.md",
      ],
      oracles: [
        {
          description:
            "Planned target-adapter oracle: enroll and revoke an owner-managed remote node, select OpenAgents-managed capacity, and exercise one separately audited managed-provider adapter behind identical lifecycle/capability/checkpoint/preview/cleanup receipts without exposing vendor APIs or topology to either client.",
          id: "openagents_cloud.user_or_managed_targets.planned",
          kind: "planned",
          mode: "e2e",
          ref: "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        },
      ],
      productArea: "owner-managed and managed-cloud execution targets",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "pending",
      statement: "Remote sessions on my own cloud (my homelab) OR a managed cloud (e.g. Daytona)",
      surface: "openagents-mobile-desktop-cloud",
      verification:
        "Pending #8547/#8636 plus bounded target-adapter leaves: prove a real owner-managed node, the accepted Agent Computer path, and one audited managed-provider adapter through the provider-neutral contract with explicit fallback history and no silent isolation downgrade.",
    },
    {
      authorityBoundary:
        "The broker grants least-privilege capability access to one owner/session/attachment/target/tool/TTL scope. It is not a generic secret tunnel, does not place raw secrets in clients or checkpoints, and does not let a moved session reuse the source attachment's credential material.",
      blockerRefs: [],
      contractId: "openagents_cloud.brokered_session_secrets.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/2026-07-13-port-02-target-scoped-capability-broker-receipt.md",
        "docs/cloud/INVARIANTS.md",
        "docs/ops/2026-07-13-portable-capability-broker-runbook.md",
        "packages/portable-session-contract/src/capability-broker.ts",
      ],
      oracles: [
        {
          description:
            "Executable broker oracle: issue, redeem, renew, revoke, reissue, release, and wipe provider/SCM/tool/API leases across owner-local and accepted managed adapters; reject replay, expiry, outage, denial, and cleanup faults; scan every exported surface for raw material.",
          id: "openagents_cloud.brokered_session_secrets.test",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/portable-session-contract/src/capability-broker.test.ts",
        },
      ],
      productArea: "cross-target secret capability broker",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "enforced",
      statement: "Secrets access via a broker (i.e. gondolin or agyn style)",
      surface: "openagents-pylon-cloud-workrooms",
      verification:
        "PORT-02 enforces target-scoped short-lived leases and injected JIT materialization across owner-local and accepted OpenAgents-managed adapters, including reauthorization, revocation-during-move, lost-ACK replay, expiry, cleanup, outage/denial, and forbidden-material scans. PORT-03 separately proves a real process/session move.",
    },
    {
      authorityBoundary:
        "Mobile receives owner-scoped session, target, capability, freshness, isolation, and command projections only. Voice is an explicit ASR/TTS/barge-in modality over the normal typed policy/approval/outcome path; it does not grant host paths, credentials, vendor APIs, ambient capture, raw-audio retention by default, or voice-only authority, and it does not revive Sarah/avatar/video.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8566",
        "github:OpenAgentsInc/openagents#8597",
        "github:OpenAgentsInc/openagents#8751",
        "github:OpenAgentsInc/openagents#8752",
        "github:OpenAgentsInc/openagents#8753",
      ],
      contractId: "openagents_mobile.any_host_session_voice.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md",
        "docs/sol/MASTER_ROADMAP.md",
      ],
      oracles: [
        {
          description:
            "Planned physical-device oracle: list and access every authorized adopted session across enrolled host classes, use visible persona-neutral voice for one follow-up or interrupt, request one stop/checkpoint/move/resume transition, reconcile a lost acknowledgement, and prove text fallback, ordinary approvals, no raw-audio retention, and no client secret/vendor authority.",
          id: "openagents_mobile.any_host_session_voice.planned",
          kind: "planned",
          mode: "e2e",
          ref: "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        },
      ],
      productArea: "mobile any-host session access and conversational voice",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "pending",
      statement:
        "Mobile client which can access any session on any host, with conversational voice",
      surface: "openagents-mobile",
      verification:
        "Pending bounded #8597/#8566 leaves: pass host/session-directory, typed movement, microphone lifecycle, ASR transcript, TTS, barge-in, approval, reconnect, privacy, and physical iOS/Android acceptance oracles against owner-managed and managed targets.",
    },
    {
      authorityBoundary:
        "The verified native session authorizes only the server-derived owner's personal Sync scope. Owner refs, credentials, database handles, transport/session objects, and raw rows remain host-only; authenticated replication substrate does not imply conversation projection, command acceptance, execution, or completion.",
      blockerRefs: [],
      contractId: "openagents_mobile.sync.host_owned_expo_sqlite.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/sync/mobile-sync-host.ts",
        "packages/khala-sync-client/src/expo-sqlite-store.ts",
        "docs/sol/issues/mobile-sync-host.md",
        "docs/sol/issues/native-authenticated-sync-hosts.md",
        "github:OpenAgentsInc/openagents#8657",
      ],
      oracles: [
        {
          description:
            "Proves restart-stable write-once installation identity, authorized personal-scope selection, dynamic token lookup, bounded live/freshness projection, native Expo composition outside the view program, and session-before-store close; the package adapter separately proves durable queue persistence, transaction rollback, and initialization cleanup.",
          id: "openagents_mobile.sync.host_owned_expo_sqlite",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-sync-host.test.ts",
        },
      ],
      productArea: "mobile cross-device continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile owns one private Expo SQLite cache through the shared Khala Sync store core and, only after native-session verification, composes the shared production transport on exactly the server-derived owner's personal scope. It re-reads rotated access custody host-side and closes session-before-store on OTA reload/unmount.",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test apps/openagents-mobile/tests/mobile-sync-host.test.ts plus the khala-sync-client Expo adapter suite prove the authenticated host/storage boundary; mobile OTA and Home tests prove close-before-reload ordering without credential projection.",
    },
    {
      authorityBoundary:
        "The Expo host selects confirmed account-linked Sync or the existing public-local conversation before mounting one Effect Native Home program. The modes are never merged. Runtime commands carry exact confirmed refs through the shared client contract and never imply provider acceptance or completion. Owner refs, credentials, store/session/transport objects, raw rows/provider events, and optimistic completion remain outside view state; denial or sign-out revokes queued hosted commands and clears account-linked projections.",
      blockerRefs: [],
      contractId: "openagents_mobile.chat.authoritative_sync_mode.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
        "apps/openagents-mobile/src/screens/home-core.ts",
        "apps/openagents-mobile/src/app.tsx",
        "packages/khala-sync-client/src/runtime.ts",
        "packages/khala-sync-client/src/session.ts",
        "docs/sol/issues/native-streamed-conversation-handoff.md",
        "docs/sol/issues/mobile-visible-sync-conversation.md",
        "github:OpenAgentsInc/openagents#8671",
      ],
      oracles: [
        {
          description:
            "Proves bounded live-vs-local selection, confirmed startup reconstruction, stable create/append refs, exact-ref start/follow-up/interrupt through the shared runtime contract, confirmed terminal observation, and pending-reconcile timeout honesty.",
          id: "openagents_mobile.chat.authoritative_sync_adapter",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-conversation.test.ts",
        },
        {
          description:
            "Proves confirmed refs/versions enter the existing Effect Native Home/thread surface, optimistic rows are visibly pending and replaced only by confirmed state, failures remove drafts, and denial clears account-linked projections.",
          id: "openagents_mobile.chat.authoritative_sync_home",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
        },
      ],
      productArea: "mobile cross-device conversation continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile uses confirmed canonical chat_thread/chat_message plus bounded agent-run timeline projections for its visible Home conversation when verified personal Sync is live. Create, append, same-run follow-up, new start, and exact-run interrupt remain visibly pending until exact stable refs and a later confirmed outcome reconcile; unavailable or timed-out work never appears completed.",
      surface: "openagents-mobile",
      verification:
        "The mobile conversation adapter and authoritative Home tests run in the normal mobile sweep; mobile typecheck plus behavior-contract coverage guard the host/view boundary.",
    },
    {
      authorityBoundary:
        "SecureStore custody protects credential material but does not prove the credential is current, assign identity authority to the client, authorize Sync rows or commands, or make cached state live.",
      blockerRefs: [],
      contractId: "openagents_mobile.session.secure_store_custody.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/auth/native-session-vault.ts",
        "docs/sol/issues/mobile-session-vault.md",
        "github:OpenAgentsInc/openagents#8658",
      ],
      oracles: [
        {
          description:
            "Proves one versioned device-only SecureStore record, exact keychain service/options, schema and epoch validation, malformed-record purge, idempotent clear, bounded recovery classification, and public-safe storage failures.",
          id: "openagents_mobile.session.secure_store_custody",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/native-session-vault.test.ts",
        },
      ],
      productArea: "mobile native session custody",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile keeps native access and refresh tokens in a versioned device-only SecureStore record, purges invalid records, and projects only credential-present-unverified until server validation.",
      surface: "openagents-mobile",
      verification:
        "The native-session-vault and Home view-program tests prove custody, fail-closed recovery, and the no-credential view boundary; mobile typecheck and behavior-contract coverage gate the integration.",
    },
    {
      authorityBoundary:
        "Server verification establishes only a native OpenAgents session. It does not make Khala Sync live, authorize cached rows, create a device_session, execute a command, or expose replacement tokens to Effect Native.",
      blockerRefs: [],
      contractId: "openagents_mobile.session.recovered_validation_rotation.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/auth/mobile-session.ts",
        "apps/openagents-mobile/src/auth/native-session-recovery.ts",
        "docs/sol/issues/mobile-session-recovery.md",
        "github:OpenAgentsInc/openagents#8659",
      ],
      oracles: [
        {
          description:
            "The mobile recovery test proves verification, rotation rewrite, denial and identity-mismatch purge, unavailable retention, and bounded tokenless state.",
          id: "openagents_mobile.session.recovered_validation_rotation",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/native-session-recovery.test.ts",
        },
        {
          description:
            "The Worker boundary test proves only a bounded refresh header on the exact native session GET reaches the existing OpenAuth verifier; other routes and malformed values cannot trigger rotation.",
          id: "openagents_api.session.native_refresh_boundary",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/auth/mobile-session.test.ts",
        },
      ],
      productArea: "mobile native session recovery",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile validates recovered credentials through the native session boundary, persists bounded OpenAuth rotation, purges denial or owner mismatch, and never equates session readiness with live Sync.",
      surface: "openagents-mobile-and-api",
      verification:
        "Worker mobile-session tests plus mobile native-session-recovery and Home tests enforce both sides; API/mobile typechecks and behavior-contract coverage gate the integration.",
    },
    {
      authorityBoundary:
        "A verified native OpenAgents session does not make Khala Sync live, authorize cached rows or commands, create a device_session, or prove physical-device acceptance.",
      blockerRefs: [],
      contractId: "openagents_mobile.session.pkce_sign_in_sign_out.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/auth/native-session-pkce.ts",
        "docs/sol/issues/mobile-session-pkce.md",
        "github:OpenAgentsInc/openagents#8660",
      ],
      oracles: [
        {
          description:
            "Proves the exact public client/provider/S256/canonical redirect, one imperative state-validating request, ephemeral prompt, code exchange, server-derived owner verification, immediate rotation, bounded results, and revocation-before-clear sign-out.",
          id: "openagents_mobile.session.pkce_sign_in_sign_out",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/native-session-pkce.test.ts",
        },
        {
          description:
            "Proves the Effect Native surface renders session entry/exit from honest phases and routes both through typed intents to host-owned session actions.",
          id: "openagents_mobile.session.typed_intents",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/home-shell-core.test.ts",
        },
      ],
      productArea: "mobile native session entry and exit",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile signs in through one state-validating GitHub authorization-code and S256 PKCE request using openagents://auth, verifies the server owner before custody, and revokes both credentials before local sign-out.",
      surface: "openagents-mobile",
      verification:
        "The native PKCE and Home view-program suites enforce the credential and typed-intent boundaries; mobile typecheck and behavior-contract coverage gate the integration.",
    },
    {
      authorityBoundary:
        "This registers only the Desktop public-client authorization redirect policy. It does not launch a browser, accept a callback, exchange a code, authenticate the renderer, make Sync live, or freeze package identity.",
      blockerRefs: [],
      contractId: "openagents_desktop.session.loopback_pkce_policy.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/auth/mobile-session.ts",
        "docs/sol/issues/desktop-session-loopback-policy.md",
        "github:OpenAgentsInc/openagents#8663",
      ],
      oracles: [
        {
          description:
            "Proves the distinct Desktop public client accepts only literal IPv4 loopback, a required ephemeral port, exact callback path, GitHub code + S256, and no userinfo/query/fragment while preserving web/mobile redirect behavior.",
          id: "openagents_desktop.session.loopback_pkce_policy",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/auth/mobile-session.test.ts",
        },
      ],
      productArea: "Desktop native OpenAuth entry",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents Desktop uses the distinct public client openagents-desktop with an RFC 8252 literal-loopback callback and GitHub authorization-code + S256 PKCE only; it never claims the mobile custom scheme.",
      surface: "openagents-desktop-and-api",
      verification:
        "The Worker native-session policy suite and API typecheck enforce the registered redirect boundary; behavior-contract validation gates its evidence record.",
    },
    {
      authorityBoundary:
        "This binds sheet-dismissal authority to user intents only; it does not authorize StoreKit purchase flows or change how/when the shell opens the sheet.",
      blockerRefs: [],
      contractId: "openagents_mobile.minerals_sheet_user_dismiss_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/home-core.ts",
        "github:OpenAgentsInc/openagents#8648",
      ],
      oracles: [
        {
          description:
            "Drives the real Home view program: with the Buy Minerals sheet open, the AskVideoEnded playback event (playToEnd/loop boundary) and the AskVideoDismissed user video-tap both end the takeover while the sheet stays open; only MineralsSheetDismissed (Not now) or MineralPackSelected closes it.",
          id: "openagents_mobile.minerals_sheet.user_dismiss_only",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/home-shell-core.test.ts",
        },
      ],
      productArea: "mobile minerals purchase sheet",
      source: {
        channel: "owner-testflight-feedback",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "The Buy Minerals Liquid Glass sheet auto-dismisses when the background reply video ends/loops. Wrong. The sheet must stay open until the USER dismisses it (selecting a price pack or Not now).",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test apps/openagents-mobile/tests/home-shell-core.test.ts proves the sheet survives video-ended and video-tap-dismiss events and closes only on the user's pack-selection or Not-now intents; the simulator pixel proof on #8648 shows the sheet still open past the video loop boundary.",
    },
    {
      authorityBoundary:
        "This binds the text-first conversation floor only; voice/avatar tiers follow #8610 capacity policy, account linking unlocks operator posture only through server-owned policy, and the bundled demo video is ambient presentation — never conversation evidence.",
      blockerRefs: [],
      contractId: "openagents_mobile.sarah_text_surface.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/sarah-core.ts",
        "apps/openagents-mobile/src/sarah/sarah-client.ts",
        "github:OpenAgentsInc/openagents#8649",
      ],
      oracles: [
        {
          description:
            "Drives the real Home view program with a deterministic turn client and the real render-rn lowering: typed turn round-trips (submit -> user + thinking -> done reply), typed SSE transcript/card events with bounded dedupe and typed reconnect phases, honest typed degradation on turn/session failure with the composer alive, turn-bootstrap session adoption, persisted-session restore marking continuity, and the SSE frame parser contract.",
          id: "openagents_mobile.sarah_text_surface.view_program",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/sarah-surface.test.ts",
        },
      ],
      productArea: "mobile Sarah conversation surface",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "retired",
      statement:
        "The owner wants Sarah consumable in OpenAgents mobile with the native glass shell as soon as possible. V1 is the text availability floor over the same /sarah contracts as web: prospect/authenticated session, bounded SSE transcript, composer turns, and typed cards inside the GL-2 shell.",
      surface: "openagents-mobile",
      verification:
        "Retired by the 2026-07-10 surface-removal decision. The 2026-07-18 reboot is a different authenticated owner-orchestrator contract: it preserves neither the public /sarah endpoint nor the prospect/SSE/avatar state model.",
    },
    {
      authorityBoundary:
        "Sarah is an owner-authenticated principal projected into the existing mobile conversation system. Read access is owner-scoped and redacted; mutations require an exact admitted capability, root and Sarah authority grants, runtime gates, and receipts. Raw secrets, custody, legal/employment commitments, destructive customer-data actions, invariant weakening, self-amplification, unsupported claims, and stable releases without current direction remain reserved.",
      blockerRefs: [],
      contractId: "openagents_mobile.sarah_owner_orchestrator.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/sarah/src/index.ts",
        "packages/authority/src/index.ts",
        "apps/openagents.com/workers/api/src/sarah-owner-routes.ts",
        "apps/openagents.com/workers/api/src/sarah-business-context.ts",
        "apps/openagents-mobile/src/screens/home-core.ts",
        "apps/openagents-mobile/tests/sarah-owner-orchestrator.test.ts",
      ],
      oracles: [
        {
          description:
            "Proves the authenticated route returns one opaque stable owner thread with durable cited memory and the admitted authority revision.",
          id: "openagents_mobile.sarah_owner_orchestrator.route",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/sarah-owner-routes.test.ts",
        },
        {
          description:
            "Drives the existing Effect Native conversation program, proving Sarah is pinned in the drawer, identified as owner orchestrator, and her ordinary messages are forced through hosted Khala without a second persona state model.",
          id: "openagents_mobile.sarah_owner_orchestrator.view_program",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/sarah-owner-orchestrator.test.ts",
        },
      ],
      productArea: "owner orchestration and business continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-18",
      },
      state: "enforced",
      statement:
        "Create an authority delegation to Sarah. Bring her back so I have a single point of contact with full persistent memory, tie her into the OpenAgents mobile app, and let me ask for release status, who is saying what, and the state of the business. Sarah is the main decision maker and orchestrator under delegated authority.",
      surface: "openagents-mobile",
      verification:
        "The Worker route and mobile Effect Native tests prove the stable owner-private thread, authority projection, conversation-first UI, and hosted runtime lane. Package tests prove fail-closed authority resolution and citation-bound business context.",
    },
    {
      authorityBoundary:
        "Owner scoping binds the Worker portal API (/api/portal/*): engagement reads resolve only through the caller's verified session identity, and admin creation/binding/seeding stays behind the operator bearer token. This contract does not authorize any client-facing engagement-id lookup route.",
      blockerRefs: [],
      contractId: "openagents_web.portal_owner_scoped_engagement.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/portal-routes.ts",
        "apps/openagents.com/workers/api/migrations/0315_portal_engagements_and_content_items.sql",
        "github:OpenAgentsInc/openagents#8652",
      ],
      oracles: [
        {
          description:
            "Route-level isolation proof against the real 0315 migration schema: a second client (different user id, same or different email) reads engagement:null, cannot decide the first client's content item (404, no existence leak, item stays draft), and a bound client_user_id is authoritative over any email match.",
          id: "openagents_web.portal_owner_scoping.routes",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/portal-routes.test.ts",
        },
        {
          description:
            "The /portal Effect Native surface is login-gated: logged-out renders only the login gate (never engagement content), and the surface offers no foreign-engagement lookup — it can only fetch the caller's own engagement.",
          id: "openagents_web.portal_owner_scoping.surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
        },
      ],
      productArea: "client portal engagement access",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "Clients see only their own engagement. Owner-scoped fail-closed: a client can NEVER read another engagement.",
      surface: "openagents-web",
      verification:
        "pnpm --dir apps/openagents.com/workers/api run test -- src/portal-routes.test.ts proves cross-client isolation against the real migration schema; pnpm --dir apps/openagents.com/apps/start run test -- src/routes/-portal.test.tsx proves the login gate and own-engagement-only surface.",
    },
    {
      authorityBoundary:
        "Receipts bind the decision write only: a decision receipt does not mark content as published, does not authorize publishing automation, and never flips after minting (idempotent repeats return the same receipt; opposite decisions are refused).",
      blockerRefs: [],
      contractId: "openagents_web.portal_decision_receipts.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/portal-store.ts",
        "github:OpenAgentsInc/openagents#8652",
      ],
      oracles: [
        {
          description:
            "Store + route proof: approve and reject each mint an immutable portal_content_decision:<id> receipt with decided_at, idempotent same-decision repeats return the identical receipt, and flipping a decided item is refused with a typed 422.",
          id: "openagents_web.portal_decision_receipts.routes",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/portal-routes.test.ts",
        },
        {
          description:
            "Surface proof: approve/reject dispatch typed intents, the optimistic card state commits on success with the minted receipt ref rendered inline, and a failed decision rolls the item back to draft.",
          id: "openagents_web.portal_decision_receipts.surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
        },
      ],
      productArea: "client portal content decisions",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement: "Decisions always produce receipts.",
      surface: "openagents-web",
      verification:
        "pnpm --dir apps/openagents.com/workers/api run test -- src/portal-routes.test.ts proves receipt minting, idempotency, and immutability; pnpm --dir apps/openagents.com/apps/start run test -- src/routes/-portal.test.tsx proves the rendered receipt ref and optimistic rollback.",
    },
    {
      authorityBoundary:
        "Presentation-only guarantee over the authenticated /portal empty state: it names the caller's own session identity (email, else provider login, else an honest fallback) and links the existing /logout route. It grants no engagement access, adds no lookup route, and never renders anyone else's identity.",
      blockerRefs: [],
      contractId: "openagents_web.portal_empty_state_account_identity.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/apps/start/src/routes/-portal-core.ts",
        "github:OpenAgentsInc/openagents#8652",
      ],
      oracles: [
        {
          description:
            "View + DOM proof: the authenticated empty state renders 'Signed in as <session email>' (login fallback, honest no-email fallback — never blank), the different-email guidance, and a 'Sign out / switch account' affordance targeting /logout.",
          id: "openagents_web.portal_empty_state_identity.surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
        },
      ],
      productArea: "client portal engagement access",
      source: {
        channel: "session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "Owner, 2026-07-10, after seeing only 'Your setup is being prepared' on /portal while logged in with no engagement, no account context, and no way to log in or switch: \"it will [go out] when it actually works... theres something horribly missing about your QA process that you would put this in front of me as ready for testing.\" The authenticated empty state must always show WHICH account/email the caller is signed in as, say that an engagement set up under a different email is the likely cause, and offer a sign-out/switch-account affordance.",
      surface: "openagents-web",
      verification:
        "pnpm --dir apps/openagents.com/apps/start run test -- src/routes/-portal.test.tsx proves the signed-in identity line, the fallback chain, the different-email guidance, and the /logout affordance on the empty state; the #8652 reopen receipts carry the deployed browser screenshots (logged out, logged in without engagement, logged in with engagement).",
    },
    {
      authorityBoundary:
        "Re-evaluation only reads confirmed personal-scope rows once the scope reports the live phase; it never fabricates a conversation, never creates or duplicates a thread, and does not make cached or pre-live state authoritative. The account control is a phase-derived affordance, not an authorization decision.",
      blockerRefs: [],
      contractId: "openagents_mobile.chat.post_auth_live_upgrade.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/app.tsx",
        "apps/openagents-mobile/src/conversation/mobile-experience-reconciler.ts",
        "apps/openagents-mobile/src/screens/home-core.ts",
        "github:OpenAgentsInc/openagents#8676",
        "github:OpenAgentsInc/openagents#8689",
        "github:OpenAgentsInc/openagents#8677",
      ],
      oracles: [
        {
          description:
            "Proves the pre-live read stays local, a live scope upgrades the selection to sync exactly once (authority sync, 'OpenAgents' pill, 'Continue conversation' composer) with no duplicate conversation, the genuine local fallback is preserved when the scope never becomes live, and a closed reconciler never upgrades.",
          id: "openagents_mobile.chat.post_auth_live_upgrade.reconciler",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-experience-reconciler.test.ts",
        },
        {
          description:
            "Proves every confirmed post-authentication phase (session_ready, bootstrapping, catching_up, live, must_refetch, stale) renders 'Sign out', genuinely unauthenticated phases render 'Link OpenAgents account', and an in-flight authenticating step renders neither.",
          id: "openagents_mobile.chat.post_auth_live_upgrade.account_control",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-account-control.test.ts",
        },
      ],
      productArea: "mobile cross-device conversation continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "enforced",
      statement:
        "Owner, 2026-07-11, after seeing the mobile app stuck on the local 'Message Khala' Khala surface with a 'Link OpenAgents account' button while the OpenAgents status surface already read 'Sync live': the visible conversation authority must re-evaluate when the verified personal scope reaches the live phase — upgrading the Khala surface from the pre-live local fallback to the confirmed sync conversation (title 'OpenAgents', 'Continue conversation' composer) exactly once and without inventing or duplicating a conversation — while a scope that never becomes live stays local; and the OpenAgents account control must read 'Sign out' for every confirmed post-authentication phase (session_ready, bootstrapping, catching_up, live, must_refetch, stale) and read 'Link OpenAgents account' only for genuinely unauthenticated phases.",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test --cwd apps/openagents-mobile runs the reconciler and account-control oracles in the normal mobile sweep; mobile typecheck plus behavior-contract coverage guard the phase-to-authority and phase-to-account-control boundaries.",
    },
    {
      authorityBoundary:
        "This binds the lightning-bolt Full Auto entry point, its compact one-mission default, collapsed Advanced configuration, and persistent run monitor. It does not itself define the run's lifecycle state machine (see full_auto_play_pause_stop_lifecycle.v1), and it grants no fleet scheduling, release, or public-claim authority.",
      blockerRefs: [],
      contractId: "openagents_desktop.full_auto_dedicated_launcher.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "specs/desktop/full-auto.product-spec.md",
        "docs/fable/2026-07-17-full-auto-implementation-audit.md",
        "github:OpenAgentsInc/openagents#8968",
        "github:OpenAgentsInc/openagents#8974",
      ],
      oracles: [
        {
          description:
            "Rev 13 real-DOM oracles prove the left rail renders a dedicated Full Auto action beside New session; the default form requires only objective plus host-resolved workspace, deterministically infers title/done condition, defaults to Codex then Claude, and keeps every other field in a closed Advanced disclosure. The persistent monitor lists every active run and dispatches open/stop by exact runRef. Start routes through the same startFullAutoRunAction as the authenticated HTTP/CLI/MCP surface, including workspace/provider admission and the eight-active-run bound.",
          id: "openagents_desktop.full_auto_dedicated_launcher.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
        },
        {
          description:
            "Real-Chromium/Electron e2e visual smoke across the launch/running/paused/stalled/terminal states and supported viewport/theme combinations remains a residual for a follow-up issue; the DOM-level oracle above is the currently enforced, currently running proof.",
          id: "openagents_desktop.full_auto_dedicated_launcher.e2e_residual",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8974",
        },
      ],
      productArea: "Desktop Full Auto launch surface",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-18",
      },
      state: "enforced",
      statement:
        "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
      surface: "openagents-desktop",
      verification:
        "The normal Desktop sweep runs react-full-auto-surface.test.tsx and full-auto-workspace.test.ts, proving the compact launcher, inferred contract, collapsed Advanced fields, Codex-to-Claude default, multi-run monitor, and runRef-scoped Stop. The Electron React smoke drives a real start/turn/stop lifecycle; Playwright-rendered review captures verify the compact and expanded layouts.",
    },
    {
      authorityBoundary:
        "This binds only the visible run view while a Full Auto run is active: pinned objective/workspace, explicit lifecycle state, an inspectable per-turn transcript, and the absence of the ordinary chat composer. It does not grant live token-streaming, steering, or any release/public-claim authority, and it does not itself define Play/Pause/Stop transition legality (see full_auto_play_pause_stop_lifecycle.v1).",
      blockerRefs: [],
      contractId: "openagents_desktop.full_auto_read_only_run_view.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "specs/desktop/full-auto.product-spec.md",
        "docs/fable/2026-07-17-full-auto-implementation-audit.md",
        "github:OpenAgentsInc/openagents#8968",
        "github:OpenAgentsInc/openagents#8974",
      ],
      oracles: [
        {
          description:
            "FA-UX-01 (#8974) landed: real-DOM component oracles prove that once a run Starts, the main canvas renders a dedicated read-only run view (pinned objective/workspace/provider/cap, an explicit lifecycle state across all ten named states -- never a generic failure banner -- and an inspectable per-turn transcript) and that the ordinary chat composer, its retired Full Auto toggle, and its manual-send fencing are absent while the run is active (react-full-auto-surface.test.tsx: 'the ordinary chat composer is genuinely absent from the run view'; full-auto-workspace.test.ts's per-state rendering sweep; react-composer.test.tsx's FA-AC-56 retirement tests).",
          id: "openagents_desktop.full_auto_read_only_run_view.dom",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx",
        },
        {
          description:
            "Real-Chromium/Electron e2e visual smoke across the launch/running/paused/stalled/terminal states and supported viewport/theme combinations remains a residual for a follow-up issue; the DOM-level oracle above is the currently enforced, currently running proof.",
          id: "openagents_desktop.full_auto_read_only_run_view.e2e_residual",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8974",
        },
      ],
      productArea: "Desktop Full Auto run view",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-17",
      },
      state: "enforced",
      statement:
        "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
      surface: "openagents-desktop",
      verification:
        "FA-UX-01 (#8974) landed: the read-only run view renders explicit lifecycle state across Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached (never a generic 'Turn failed' banner, closing the 2026-07-17 audit's observability gap), and the ordinary composer, toggle, and badge (FA-AC-01/19/21 in the ProductSpec) are retired from the chat surface (FA-AC-55/FA-AC-56), proven in the normal Desktop test sweep. A real-Chromium/Electron e2e visual smoke is a residual, tracked as a planned oracle above.",
    },
    {
      authorityBoundary:
        "This binds each runRef-scoped lifecycle state machine and its Play/Pause/Stop transition legality and attribution. Rev 13 admits up to eight independently active run/thread identities, but grants no fleet allocation, provider selection outside an owner-admitted ordered rotation policy, or mid-run steering; it does not itself verify that a run's stated done condition was actually satisfied.",
      blockerRefs: [],
      contractId: "openagents_desktop.full_auto_play_pause_stop_lifecycle.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "specs/desktop/full-auto.product-spec.md",
        "docs/fable/2026-07-17-full-auto-implementation-audit.md",
        "github:OpenAgentsInc/openagents#8968",
        "github:OpenAgentsInc/openagents#8969",
        "github:OpenAgentsInc/openagents#8974",
      ],
      oracles: [
        {
          description:
            "Unit coverage exercises the full Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached state machine, its exhaustive legal-transition matrix, up to eight independently active runs with a typed no-side-effect capacity refusal, rerun/new-generation semantics, and additive legacy migration. Pause/Resume/Stop semantics and actor/timestamp/typed-reason attribution remain exact per runRef.",
          id: "openagents_desktop.full_auto_play_pause_stop_lifecycle.run_model",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
        },
        {
          description:
            'FA-UX-01 (#8974) landed: the read-only run view\'s Pause/Resume/Stop/Retry-now controls are wired to `full-auto-run-actions.ts` -- the SAME shared action functions the opt-in HTTP control server uses (actor:"owner_ui" vs actor:"control_api") -- via a dedicated renderer IPC bridge (full-auto-run-ipc-contract.ts, main.ts). Per-state control visibility (Pause only while Running, Resume only while Paused, Retry now only while Stalled with a recoverable cause, Stop present on every non-terminal state and absent once terminal) and click-to-intent wiring are proven end to end through the real Effect intent registry and a rendered DOM.',
          id: "openagents_desktop.full_auto_play_pause_stop_lifecycle.ui_wiring",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents-desktop/src/renderer/full-auto-workspace.test.ts",
        },
      ],
      productArea: "Desktop Full Auto run lifecycle",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-17",
      },
      state: "enforced",
      statement:
        "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
      surface: "openagents-desktop",
      verification:
        "FA-RUN-01 (#8969) landed the durable FullAutoRun lifecycle state machine (FA-AC-43/FA-AC-44/FA-AC-45 in the ProductSpec), its control-API Pause/Resume/Stop routes, and the legacy-registry migration, unit- and route-tested (apps/openagents-desktop/tests/full-auto-run-registry.test.ts, apps/openagents-desktop/src/full-auto-run-control-server.test.ts). FA-UX-01 (#8974) landed wiring those exact typed transitions into the read-only run view's Pause/Resume/Stop/Retry-now controls through a dedicated renderer IPC bridge sharing the same action functions as the control API (apps/openagents-desktop/src/full-auto-run-actions.ts) -- an illegal transition is refused by the shared registry.transition function, never silently coerced by the UI.",
    },
    {
      authorityBoundary:
        "This binds the bind-time readiness projection only. It records which routing candidates were ready and shows every Full-Auto-eligible lane plus an advisory-only Apple FM marker. It grants no action authority, does not itself bind a routing policy (validateFullAutoRoutingPolicy remains the fail-closed bind gate), and gives Apple FM no action lane.",
      blockerRefs: [],
      contractId: "openagents_desktop.full_auto_readiness_gated_routing.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/full-auto-readiness.ts",
        "apps/openagents-desktop/src/renderer/boot-sequence.ts",
        "apps/openagents-desktop/src/full-auto-routing.ts",
        "docs/fable/2026-07-20-full-auto-first-verifiable-mode.md",
        "github:OpenAgentsInc/openagents#9111",
      ],
      oracles: [
        {
          description:
            "Unit coverage proves the bind-time readiness snapshot is projected from the SAME lane gate validateFullAutoRoutingPolicy uses (allReady agrees with the fail-closed bind decision), evaluates every candidate without a first-refusal short-circuit, maps unknown/unadmitted/ineligible lanes to typed reasons, reads a still-probing lane as checking rather than unavailable, and reconciles the scan set: every Full-Auto-eligible action lane (including Cursor, which the boot scan omits) plus Apple FM as an advisory-only entry with no action lane. The snapshot decodes against its schema and is an optional additive field on the run report.",
          id: "openagents_desktop.full_auto_readiness_gated_routing.projection",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/src/full-auto-readiness.test.ts",
        },
      ],
      productArea: "Desktop Full Auto readiness-gated routing",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-20",
      },
      state: "enforced",
      statement:
        "Full Auto run start reads provider readiness from the same lane truth the BOOT SEQUENCE renders: the routing policy's per-candidate readiness is projected and recorded on the run report, a candidate whose lane failed its probe is a visible typed refusal rather than a silent drop, and the lane scan shows every Full-Auto-eligible lane plus an advisory-only Apple FM marker.",
      surface: "openagents-desktop",
      verification:
        "FAV-01 (#9111) landed full-auto-readiness.ts: projectFullAutoReadinessSnapshot and projectFullAutoLaneScan project readiness from the shared FullAutoRoutingLaneGate, the run report carries an optional readinessSnapshot field, and the projections are unit-tested (apps/openagents-desktop/src/full-auto-readiness.test.ts) including agreement with validateFullAutoRoutingPolicy and the scan/lane reconciliation.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-20.1",
};
