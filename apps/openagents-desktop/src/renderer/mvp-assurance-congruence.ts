import { mvpDockSurfaces } from "./mvp-visible-surfaces.ts"

export type MvpAssuranceCoverageItem = Readonly<{
  surfaceId: string
  interaction: string
  criterionRefs: ReadonlyArray<string>
  assuranceItemRefs: ReadonlyArray<string>
  contractRefs: ReadonlyArray<string>
  oracleRefs: ReadonlyArray<string>
}>

/**
 * UX-5 (#8791): the complete and minimal proof map for the expected-working
 * MVP dock. A row is an assurance item, not a feature wish: every row must
 * cite a ProductSpec criterion, AssuranceSpec obligation, enforced behavior
 * contract, and executable oracle. The congruence oracle below rejects both
 * an uncovered allowlisted surface and coverage for a non-MVP surface.
 */
export const mvpAssuranceCoverageMatrix: ReadonlyArray<MvpAssuranceCoverageItem> = [
  {
    surfaceId: "workspace-new-chat",
    interaction: "Open a new chat and type immediately without a click",
    criterionRefs: ["CW-AC-10"],
    assuranceItemRefs: ["AO-CW-AC-10-01"],
    contractRefs: ["openagents_desktop.composer.focused_on_open.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/composer-focus.test.ts", "apps/openagents-desktop/src/main.ts"],
  },
  {
    surfaceId: "workspace-chat",
    interaction: "Search the full coding-history catalog by session title or workspace",
    criterionRefs: ["CW-AC-10"],
    assuranceItemRefs: ["AO-CW-AC-10-01"],
    contractRefs: ["openagents_desktop.history.session_search_filters.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/shell.test.ts", "apps/openagents-desktop/tests/history-catalog-scale.test.ts"],
  },
  {
    surfaceId: "workspace-chat",
    interaction: "Disclose scanning, paged, and complete history scope truthfully",
    criterionRefs: ["CW-AC-10"],
    assuranceItemRefs: ["AO-CW-AC-10-01"],
    contractRefs: ["openagents_desktop.history.sidebar_header_truthful_scope.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/shell.test.ts", "apps/openagents-desktop/tests/history-catalog-scale.test.ts"],
  },
  {
    surfaceId: "workspace-chat",
    interaction: "Send, stop, steer, queue, approve, and resume on the typed causal timeline",
    criterionRefs: ["CW-AC-10", "CW-AC-11", "CW-AC-13"],
    assuranceItemRefs: ["AO-CW-AC-10-01", "AO-CW-AC-11-01", "AO-CW-AC-13-01"],
    contractRefs: ["openagents_desktop.mvp.visible_surface_allowlist.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/runtime-conversation.test.ts", "apps/openagents-desktop/tests/native-conversation-continuation.e2e.test.ts"],
  },
  {
    surfaceId: "workspace-product-spec",
    interaction: "Create or open a validator-clean ProductSpec and expose exact validation failures",
    criterionRefs: ["CW-AC-04", "CW-AC-05", "CW-AC-06", "CW-AC-07", "CW-AC-08", "CW-AC-09"],
    assuranceItemRefs: ["AO-CW-AC-04-01", "AO-CW-AC-05-01", "AO-CW-AC-06-01", "AO-CW-AC-07-01", "AO-CW-AC-08-01", "AO-CW-AC-09-01"],
    contractRefs: ["openagents_desktop.mvp.visible_surface_allowlist.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/product-spec-workspace.test.ts", "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"],
  },
  {
    surfaceId: "workspace-assurance-spec",
    interaction: "Inspect source-driven assurance obligations without execution or verification authority",
    criterionRefs: ["CW-AC-04", "CW-AC-07"],
    assuranceItemRefs: ["AO-CW-AC-04-01", "AO-CW-AC-07-01"],
    contractRefs: ["openagents_desktop.assurance_spec.document_visualization.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts"],
  },
  {
    surfaceId: "workspace-home",
    interaction: "Grant one repository and retain stable work-context identity",
    criterionRefs: ["CW-AC-03"],
    assuranceItemRefs: ["AO-CW-AC-03-01"],
    contractRefs: ["openagents_desktop.mvp.visible_surface_allowlist.v1"],
    oracleRefs: ["apps/openagents-desktop/tests/local-first-identity.e2e.test.ts", "apps/openagents-desktop/src/mvp-assurance-criteria.test.ts"],
  },
  {
    surfaceId: "shell-settings-toggle",
    interaction: "Open and scroll Settings without opening Command-K",
    criterionRefs: ["CW-AC-12"],
    assuranceItemRefs: ["AO-CW-AC-12-01"],
    contractRefs: ["openagents_desktop.settings.reachable_workspace.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/shell.test.ts", "apps/openagents-desktop/src/renderer/design-conformance.test.ts"],
  },
  {
    surfaceId: "shell-settings-toggle",
    interaction: "Report the current Codex session and runtime/update/diagnostic truth without account linking",
    criterionRefs: ["CW-AC-01", "CW-AC-02", "CW-AC-17", "CW-AC-18"],
    assuranceItemRefs: ["AO-CW-AC-01-01", "AO-CW-AC-02-01", "AO-CW-AC-17-01", "AO-CW-AC-18-01"],
    contractRefs: ["openagents_desktop.mvp.uses_logged_in_codex_session.v1"],
    oracleRefs: ["apps/openagents-desktop/src/renderer/settings.test.ts", "apps/openagents-desktop/src/codex-local-runtime.test.ts"],
  },
]

export const mvpAssuranceCongruenceViolations = (
  matrix: ReadonlyArray<MvpAssuranceCoverageItem> = mvpAssuranceCoverageMatrix,
  allowedSurfaceIds: ReadonlyArray<string> = mvpDockSurfaces.map(surface => surface.id),
): ReadonlyArray<string> => {
  const violations: Array<string> = []
  const allowed = new Set(allowedSurfaceIds)
  const covered = new Set(matrix.map(item => item.surfaceId))
  for (const surfaceId of allowed) if (!covered.has(surfaceId)) violations.push(`allowlisted surface "${surfaceId}" has no assurance item`)
  for (const surfaceId of covered) if (!allowed.has(surfaceId)) violations.push(`assurance matrix over-covers non-MVP surface "${surfaceId}"`)
  for (const [index, item] of matrix.entries()) {
    for (const [field, refs] of Object.entries({ criterionRefs: item.criterionRefs, assuranceItemRefs: item.assuranceItemRefs, contractRefs: item.contractRefs, oracleRefs: item.oracleRefs })) {
      if (refs.length === 0) violations.push(`coverage item ${index + 1} (${item.surfaceId}) has no ${field}`)
    }
  }
  return violations
}

export const renderMvpAssuranceCoverageMarkdown = (): string => {
  const rows = mvpAssuranceCoverageMatrix.map(item =>
    `| \`${item.surfaceId}\` | ${item.interaction} | ${item.criterionRefs.map(ref => `\`${ref}\``).join(", ")} | ${item.assuranceItemRefs.map(ref => `\`${ref}\``).join(", ")} | ${item.contractRefs.map(ref => `\`${ref}\``).join(", ")} | ${item.oracleRefs.map(ref => `\`${ref}\``).join("<br>")} |`,
  )
  return `# OpenAgents Desktop MVP assurance coverage matrix\n\nThis is the human-readable projection of the checked-in UX-5 congruence oracle. The authority is \`mvpDockSurfaces\`; every expected-working surface interaction maps to ProductSpec intent, an AssuranceSpec item, an enforced behavior contract, and an executable oracle. Coverage outside that allowlist and missing coverage both fail the normal Desktop sweep.\n\n| Surface | Expected-working interaction | ProductSpec criteria | Assurance items | Behavior contracts | Executable oracles |\n| --- | --- | --- | --- | --- | --- |\n${rows.join("\n")}\n\nThe matrix is intentionally minimal: Files and read-only review remain command-reachable supporting views under \`CW-AC-12\`/\`CW-AC-14\`, but UX-4 removed them from the visible dock; Fleet, Terminal, Inbox, account linking, provider selection, MCP/plugins, and Git mutation are outside the MVP and must not acquire assurance rows.\n`
}
