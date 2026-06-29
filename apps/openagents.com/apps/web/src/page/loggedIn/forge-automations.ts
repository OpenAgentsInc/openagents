import type { AutopilotWorkComposerDraft, AutopilotWorkSummary } from './model'

export type ForgeStageKey =
  | 'signal'
  | 'triage'
  | 'codegen'
  | 'validate'
  | 'release'
  | 'document'
  | 'monitor'
  | 'deploy'

export type ForgeAutomationMode = 'deterministic' | 'ai_assisted'

export type ForgeAutomation = Readonly<{
  branch: string
  description: string
  evidenceRefs: ReadonlyArray<string>
  id: string
  label: string
  maxSpendCents: string
  mode: ForgeAutomationMode
  objective: string
  repositoryFullName: string
  stageKey: ForgeStageKey
  stageName: string
  verificationCommand: string
}>

export const forgeAutomations: ReadonlyArray<ForgeAutomation> = [
  {
    branch: 'main',
    description:
      'Classifies inbound requests and produces the first scoped work-order candidate.',
    evidenceRefs: [
      'evidence.forge_automation.signal_classifier.source_refs',
      'receipt.forge_automation.signal_classifier.work_order_created',
    ],
    id: 'forge.automation.signal_classifier',
    label: 'Signal classifier',
    maxSpendCents: '0',
    mode: 'deterministic',
    objective:
      'Forge automation forge.automation.signal_classifier: classify inbound product signals into a public-safe work-order candidate with source refs, duplicate checks, and acceptance criteria.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'signal',
    stageName: 'Signal',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Scopes a candidate into repo, stage, blocker, and acceptance refs before launch.',
    evidenceRefs: [
      'evidence.forge_automation.triage_scope.repo_scope',
      'receipt.forge_automation.triage_scope.work_order_created',
    ],
    id: 'forge.automation.triage_scope',
    label: 'Scope triage',
    maxSpendCents: '0',
    mode: 'ai_assisted',
    objective:
      'Forge automation forge.automation.triage_scope: turn a queued product signal into a scoped OpenAgents work order with repo placement, blocker refs, and acceptance criteria.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'triage',
    stageName: 'Triage',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Runs a coding-agent work unit against the selected public repository scope.',
    evidenceRefs: [
      'evidence.forge_automation.codegen_patch.artifact_refs',
      'receipt.forge_automation.codegen_patch.work_order_created',
    ],
    id: 'forge.automation.codegen_patch',
    label: 'Patch generator',
    maxSpendCents: '0',
    mode: 'ai_assisted',
    objective:
      'Forge automation forge.automation.codegen_patch: produce a scoped code-change artifact with public-safe summary, verification plan, and artifact refs.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'codegen',
    stageName: 'Code Gen',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Runs typecheck, tests, smoke, and policy checks before review.',
    evidenceRefs: [
      'evidence.forge_automation.validate_gate.test_refs',
      'receipt.forge_automation.validate_gate.work_order_created',
    ],
    id: 'forge.automation.validate_gate',
    label: 'Validation gate',
    maxSpendCents: '0',
    mode: 'deterministic',
    objective:
      'Forge automation forge.automation.validate_gate: verify a candidate with typecheck, tests, smoke refs, and explicit blocker refs when checks fail.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'validate',
    stageName: 'Validate',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Prepares accepted delivery notes and release-safe receipt refs.',
    evidenceRefs: [
      'evidence.forge_automation.release_notes.summary_refs',
      'receipt.forge_automation.release_notes.work_order_created',
    ],
    id: 'forge.automation.release_notes',
    label: 'Release note writer',
    maxSpendCents: '0',
    mode: 'ai_assisted',
    objective:
      'Forge automation forge.automation.release_notes: prepare release notes, issue comment summary, and public-safe receipt refs for an accepted candidate.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'release',
    stageName: 'Release',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Updates runbooks, roadmap notes, and customer-safe handoff summaries.',
    evidenceRefs: [
      'evidence.forge_automation.docs_handoff.doc_refs',
      'receipt.forge_automation.docs_handoff.work_order_created',
    ],
    id: 'forge.automation.docs_handoff',
    label: 'Docs handoff',
    maxSpendCents: '0',
    mode: 'ai_assisted',
    objective:
      'Forge automation forge.automation.docs_handoff: update the relevant docs and handoff summary for a completed Forge work order without widening product claims.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'document',
    stageName: 'Document',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Turns failures, regressions, and forum signals into monitored follow-up work.',
    evidenceRefs: [
      'evidence.forge_automation.monitor_regression.signal_refs',
      'receipt.forge_automation.monitor_regression.work_order_created',
    ],
    id: 'forge.automation.monitor_regression',
    label: 'Regression monitor',
    maxSpendCents: '0',
    mode: 'deterministic',
    objective:
      'Forge automation forge.automation.monitor_regression: convert blocked, rejected, or incident signals into a follow-up work order with stale-data and blocker refs.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'monitor',
    stageName: 'Monitor',
    verificationCommand: 'bun run check:deploy',
  },
  {
    branch: 'main',
    description:
      'Runs deploy checklist and post-deploy smoke evidence for approved work.',
    evidenceRefs: [
      'evidence.forge_automation.deploy_smoke.smoke_refs',
      'receipt.forge_automation.deploy_smoke.work_order_created',
    ],
    id: 'forge.automation.deploy_smoke',
    label: 'Deploy smoke',
    maxSpendCents: '0',
    mode: 'deterministic',
    objective:
      'Forge automation forge.automation.deploy_smoke: run the deploy checklist and capture post-deploy smoke refs for an approved OpenAgents surface.',
    repositoryFullName: 'OpenAgentsInc/openagents',
    stageKey: 'deploy',
    stageName: 'Deploy',
    verificationCommand: 'bun run check:deploy',
  },
]

export const automationDraft = (
  automation: ForgeAutomation,
): AutopilotWorkComposerDraft => ({
  branch: automation.branch,
  maxSpendCents: automation.maxSpendCents,
  objective: automation.objective,
  repositoryFullName: automation.repositoryFullName,
  verificationCommand: automation.verificationCommand,
})

export const automationDraftForId = (
  automationId: string,
): AutopilotWorkComposerDraft | null => {
  const automation = forgeAutomations.find(item => item.id === automationId)

  return automation === undefined ? null : automationDraft(automation)
}

export const configuredAutomationCountForStage = (
  stageKey: ForgeStageKey,
): number =>
  forgeAutomations.filter(automation => automation.stageKey === stageKey).length

const automationMarker = (automation: ForgeAutomation): string =>
  automation.id.replaceAll('.', '_')

export const workOrderRefsForAutomation = (
  automation: ForgeAutomation,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
): ReadonlyArray<string> => {
  const marker = automationMarker(automation)

  return workOrders
    .filter(order =>
      (order.taskRefs ?? []).some(taskRef => taskRef.includes(marker)),
    )
    .map(order => order.workOrderRef)
}
