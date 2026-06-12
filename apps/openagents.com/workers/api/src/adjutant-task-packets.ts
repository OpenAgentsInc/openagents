import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type { AdjutantAssignment } from './adjutant-assignments'
import type { AdjutantResearchBrief } from './adjutant-research-briefs'
import { inferSiteVisualAssetRequirements } from './sites-build-validations'

export const ADJUTANT_TASK_PACKET_DIR = 'docs/autopilot-tasks'
export const ADJUTANT_TASK_PACKET_REPOSITORY = {
  name: 'autopilot-omega',
  owner: 'OpenAgentsInc',
} as const

export type AdjutantTaskPacketSiteContext = Readonly<{
  id: string
  slug: string
  title: string
}>

export type BuildAdjutantTaskPacketInput = Readonly<{
  assignment: AdjutantAssignment
  commitSha: string
  operatorNotes?: string | undefined
  researchBrief?: AdjutantResearchBrief | null | undefined
  site: AdjutantTaskPacketSiteContext | null
  taskSpecPath?: string | undefined
}>

export type AdjutantTaskPacket = Readonly<{
  commitSha: string
  markdown: string
  path: string
}>

export type AdjutantTaskPacketRefValidationInput = Readonly<{
  commitSha: string
  githubAccessToken?: string | undefined
  path: string
  repositoryName: string
  repositoryOwner: string
}>

export class AdjutantTaskPacketUnsafe extends S.TaggedErrorClass<AdjutantTaskPacketUnsafe>()(
  'AdjutantTaskPacketUnsafe',
  {
    reason: S.String,
  },
) {}

export class AdjutantTaskPacketValidationError extends S.TaggedErrorClass<AdjutantTaskPacketValidationError>()(
  'AdjutantTaskPacketValidationError',
  {
    reason: S.String,
  },
) {}

export class AdjutantTaskPacketRefMissing extends S.TaggedErrorClass<AdjutantTaskPacketRefMissing>()(
  'AdjutantTaskPacketRefMissing',
  {
    commitSha: S.String,
    path: S.String,
    reason: S.String,
  },
) {}

export class AdjutantTaskPacketRefValidationFailed extends S.TaggedErrorClass<AdjutantTaskPacketRefValidationFailed>()(
  'AdjutantTaskPacketRefValidationFailed',
  {
    reason: S.String,
  },
) {}

export type AdjutantTaskPacketError =
  | AdjutantTaskPacketRefMissing
  | AdjutantTaskPacketRefValidationFailed
  | AdjutantTaskPacketUnsafe
  | AdjutantTaskPacketValidationError

const commitShaPattern = /^[0-9a-fA-F]{7,40}$/
const taskPacketPathPattern =
  /^docs\/autopilot-tasks\/[0-9a-zA-Z][0-9a-zA-Z._-]*\.md$/

export const validAdjutantTaskPacketCommitSha = (commitSha: string): boolean =>
  commitShaPattern.test(commitSha.trim())

export const validAdjutantTaskPacketPath = (path: string): boolean =>
  taskPacketPathPattern.test(path.trim()) &&
  !path.includes('..') &&
  !path.includes('//')

export const defaultAdjutantTaskPacketPath = (
  assignment: AdjutantAssignment,
): string => `${ADJUTANT_TASK_PACKET_DIR}/adjutant-${assignment.id}.md`

const targetUrl = (site: AdjutantTaskPacketSiteContext | null): string =>
  site === null
    ? 'pending Site URL'
    : `https://sites.openagents.com/${site.slug}`

const taskTitle = (
  assignment: AdjutantAssignment,
  site: AdjutantTaskPacketSiteContext | null,
): string => {
  if (site !== null) {
    return `Autopilot Task: Adjutant ${assignment.assignmentKind} for ${site.title}`
  }

  return `Autopilot Task: Adjutant ${assignment.assignmentKind} for ${assignment.softwareOrderId ?? assignment.id}`
}

const outputContract = (
  assignment: AdjutantAssignment,
  site: AdjutantTaskPacketSiteContext | null,
): ReadonlyArray<string> => [
  `Use assignment ID ${assignment.id} as the work receipt.`,
  `Use software order ID ${assignment.softwareOrderId ?? 'none'} as order context.`,
  `Use Site ID ${assignment.siteId ?? 'none'} as Site context.`,
  `Target URL: ${targetUrl(site)}.`,
  'Produce reviewable site source, asset manifest entries, and a concise result summary.',
  'Keep public-facing output focused on the customer subject, not OpenAgents delivery mechanics.',
  'Do not include credentials, OAuth state, callback URLs, bearer tokens, local secret paths, or raw runner payloads.',
]

const safetyRules: ReadonlyArray<string> = [
  'Do not expose secrets, provider grants, callback tokens, OAuth data, billing internals, or raw customer private data.',
  'Do not invent deployment state. Use only the assignment, order, Site, and repository context present in tracked files or APIs.',
  'Do not deploy or widen access without operator review and the Sites launch checklist.',
  'Keep generated artifacts suitable for public review before they are saved as a Site version.',
]

const acceptanceCriteria: ReadonlyArray<string> = [
  'The generated Site artifacts are saved through the Sites version lifecycle.',
  'The operator can review the saved version before deployment.',
  'The assignment ledger records the run, resulting commit, saved version, and deployment decision in later lifecycle steps.',
  'Focused tests or manual verification are recorded in the run summary.',
]

const visualAssetRequirementSection = (
  assignment: AdjutantAssignment,
  operatorNotes: string | undefined,
): ReadonlyArray<string> => {
  const requirements = inferSiteVisualAssetRequirements([
    { source: 'customer_request', text: assignment.objective },
    { source: 'operator_notes', text: operatorNotes ?? '' },
  ])

  if (requirements.length === 0) {
    return [
      '',
      '## Visual Asset Requirements',
      '',
      '- none explicitly requested',
      '- If you add external images anyway, include source/attribution metadata in the result summary.',
    ]
  }

  return [
    '',
    '## Visual Asset Requirements',
    '',
    ...requirements.map(
      requirement =>
        `- ${requirement.kind}: required from ${requirement.source} - ${requirement.summary}`,
    ),
    '- CSS-only diagrams do not satisfy requested image media unless the request explicitly asks only for diagrams.',
    '- Include real image assets or remote image references plus source/attribution metadata in the result summary.',
    '- Do not mark the revision ready for customer review if requested images are missing.',
  ]
}

const briefList = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  values.length === 0 ? ['- none'] : values.map(item => `- ${item}`)

const researchBriefSection = (
  researchBrief: AdjutantResearchBrief | null | undefined,
): ReadonlyArray<string> =>
  researchBrief === undefined || researchBrief === null
    ? []
    : [
        '',
        '## Approved Research Brief',
        '',
        `- researchBriefId: ${researchBrief.id}`,
        `- enrichmentRunId: ${researchBrief.enrichmentRunId ?? 'none'}`,
        `- approvedAt: ${researchBrief.approvedAt ?? 'none'}`,
        '',
        '### Summary',
        '',
        researchBrief.summary,
        '',
        '### Grounded Facts',
        '',
        ...briefList(researchBrief.groundedFacts),
        '',
        '### Suggested Site Sections',
        '',
        ...briefList(researchBrief.suggestedSections),
        '',
        '### Unknowns',
        '',
        ...briefList(researchBrief.unknowns),
        '',
        '### Claims Needing Operator Review',
        '',
        ...briefList(researchBrief.claimsNeedingReview),
        '',
        '### Approved Sources',
        '',
        ...(researchBrief.sourceCards.length === 0
          ? ['- none']
          : researchBrief.sourceCards.map(sourceCard =>
              sourceCard.highlightText === null
                ? `- ${sourceCard.title}: ${sourceCard.url}`
                : `- ${sourceCard.title}: ${sourceCard.url} - ${sourceCard.highlightText}`,
            )),
      ]

export const buildAdjutantTaskPacket = (
  input: BuildAdjutantTaskPacketInput,
): Effect.Effect<AdjutantTaskPacket, AdjutantTaskPacketError> =>
  Effect.gen(function* () {
    const commitSha = input.commitSha.trim()
    const path = (
      input.taskSpecPath ?? defaultAdjutantTaskPacketPath(input.assignment)
    ).trim()

    if (!validAdjutantTaskPacketCommitSha(commitSha)) {
      return yield* new AdjutantTaskPacketValidationError({
        reason: 'Task packet commit SHA must be a 7 to 40 character hex SHA.',
      })
    }

    if (!validAdjutantTaskPacketPath(path)) {
      return yield* new AdjutantTaskPacketValidationError({
        reason:
          'Task packet path must be a Markdown file directly under docs/autopilot-tasks/.',
      })
    }

    if (
      input.researchBrief !== undefined &&
      input.researchBrief !== null &&
      input.researchBrief.status !== 'approved'
    ) {
      return yield* new AdjutantTaskPacketValidationError({
        reason: 'Autopilot research brief must be approved before dispatch.',
      })
    }

    const markdown = [
      `# ${taskTitle(input.assignment, input.site)}`,
      '',
      'Status: ready for dispatch',
      'Target repo: OpenAgentsInc/autopilot-omega',
      'Target branch: main',
      `Primary agent: ${input.assignment.agentId}`,
      `Team: ${input.assignment.teamId ?? 'none'}`,
      `Project: ${input.assignment.projectId ?? 'none'}`,
      `Visibility: ${input.assignment.visibility}`,
      `Task packet path: ${path}`,
      `Commit SHA: ${commitSha}`,
      '',
      '## Assignment',
      '',
      `- assignmentId: ${input.assignment.id}`,
      `- assignmentKind: ${input.assignment.assignmentKind}`,
      `- softwareOrderId: ${input.assignment.softwareOrderId ?? 'none'}`,
      `- siteId: ${input.assignment.siteId ?? 'none'}`,
      `- goalId: ${input.assignment.goalId ?? 'none'}`,
      `- targetUrl: ${targetUrl(input.site)}`,
      '',
      '## Objective',
      '',
      input.assignment.objective,
      '',
      '## Output Contract',
      '',
      ...outputContract(input.assignment, input.site).map(item => `- ${item}`),
      '',
      '## Safety Rules',
      '',
      ...safetyRules.map(item => `- ${item}`),
      '',
      '## Acceptance Criteria',
      '',
      ...acceptanceCriteria.map(item => `- ${item}`),
      ...visualAssetRequirementSection(input.assignment, input.operatorNotes),
      ...researchBriefSection(input.researchBrief),
      ...(input.operatorNotes === undefined || input.operatorNotes.trim() === ''
        ? []
        : ['', '## Operator Notes', '', input.operatorNotes.trim()]),
      '',
    ].join('\n')

    if (containsProviderSecretMaterial(markdown)) {
      return yield* new AdjutantTaskPacketUnsafe({
        reason: 'Autopilot task packet contains secret-shaped material.',
      })
    }

    return { commitSha, markdown, path }
  })
