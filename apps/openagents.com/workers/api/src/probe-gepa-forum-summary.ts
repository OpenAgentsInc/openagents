import { Schema as S } from 'effect'

import {
  ProbeGepaCampaignProjection,
  ProbeGepaCampaignProjectionUnsafe,
  assertProbeGepaCampaignProjectionSafe,
  probeGepaCampaignPublicSummary,
} from './probe-gepa-campaign-projection'
import {
  ProbeGepaOutcomeMetricsProjection,
  probeGepaOutcomeMetricSummary,
} from './probe-gepa-outcome-metrics'
import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const ProbeGepaForumSummarySchemaVersion =
  'omega.probe_gepa_forum_summary.v1'

export const ProbeGepaForumPostingMode = S.Literals([
  'draft_only',
  'operator_artanis_authority_required',
  'probe_registered_agent_reply',
])
export type ProbeGepaForumPostingMode = typeof ProbeGepaForumPostingMode.Type

export class ProbeGepaForumSummaryInput extends S.Class<ProbeGepaForumSummaryInput>(
  'ProbeGepaForumSummaryInput',
)({
  forumTopicRef: S.String,
  outcomeMetrics: S.NullOr(ProbeGepaOutcomeMetricsProjection),
  projection: ProbeGepaCampaignProjection,
  proofBundleRefs: S.Array(S.String),
  scorerRefs: S.Array(S.String),
  targetThreadRef: S.NullOr(S.String),
  verifierRefs: S.Array(S.String),
}) {}

export class ProbeGepaForumSummaryDraft extends S.Class<ProbeGepaForumSummaryDraft>(
  'ProbeGepaForumSummaryDraft',
)({
  bodyMarkdown: S.String,
  claimBoundaryLine: S.String,
  forumTopicRef: S.String,
  idempotencyKey: S.String,
  postingAuthorityBoundary: S.String,
  postingMode: ProbeGepaForumPostingMode,
  schemaVersion: S.Literal(ProbeGepaForumSummarySchemaVersion),
  targetThreadRef: S.NullOr(S.String),
  title: S.String,
}) {}

export class ProbeGepaForumSummaryUnsafe extends S.TaggedErrorClass<ProbeGepaForumSummaryUnsafe>()(
  'ProbeGepaForumSummaryUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|fixture[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(auth|benchmark|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ProbeGepaForumSummaryUnsafe({
      reason: `${label} contains secrets, raw traces, raw prompts, raw benchmark fixtures, private paths, account refs, bearer material, wallet material, invoices, or preimages.`,
    })
  }
}

export const generateProbeGepaForumSummary = (
  input: ProbeGepaForumSummaryInput,
  postingMode: ProbeGepaForumPostingMode = 'draft_only',
): ProbeGepaForumSummaryDraft => {
  const projection = assertProbeGepaCampaignProjectionSafe(input.projection)
  const publicSummary = probeGepaCampaignPublicSummary(projection)
  const outcomeLine = input.outcomeMetrics === null
    ? 'Product outcome boundary: no accepted coding outcome evidence attached.'
    : `Product outcome boundary: ${probeGepaOutcomeMetricSummary(input.outcomeMetrics).claimText}`
  const proofBundleRefs = uniqueRefs(input.proofBundleRefs)
  const scorerRefs = uniqueRefs(input.scorerRefs)
  const verifierRefs = uniqueRefs(input.verifierRefs)

  assertSafeRefs('Forum topic refs', [
    input.forumTopicRef,
    ...(input.targetThreadRef === null ? [] : [input.targetThreadRef]),
  ])
  assertSafeRefs('Forum proof bundle refs', proofBundleRefs)
  assertSafeRefs('Forum scorer refs', scorerRefs)
  assertSafeRefs('Forum verifier refs', verifierRefs)

  if (
    postingMode !== 'draft_only' &&
    postingMode !== 'operator_artanis_authority_required' &&
    postingMode !== 'probe_registered_agent_reply'
  ) {
    throw new ProbeGepaForumSummaryUnsafe({
      reason:
        'Probe Forum summaries cannot post as Artanis or invoke an Artanis bridge.',
    })
  }

  const claimBoundaryLine = claimBoundaryForState(projection.claimState)
  const postingAuthorityBoundary =
    'Posting boundary: Probe may publish only as its own registered agent or leave this as a draft. Posting as Artanis requires Omega/operator authority; this draft does not invoke the Artanis bridge.'
  const title = `Probe GEPA ${projection.stage} summary: ${projection.campaignRef}`
  const idempotencyKey = [
    'forum_summary.probe_gepa',
    publicRefSegment(projection.campaignRef, 'campaign'),
    projection.stage,
    projection.claimState,
    String(projection.completedMetricCalls),
  ].join('.')

  const bodyMarkdown = [
    `Campaign: ${projection.campaignRef}`,
    `Stage: ${projection.stage}`,
    `Dataset/split refs: ${joinRefs([
      ...projection.benchmarkSuiteRefs,
      ...projection.splitManifestRefs,
    ])}`,
    `Candidate hash refs: ${joinRefs(projection.candidateHashRefs)}`,
    `Completed metric calls: ${projection.completedMetricCalls}`,
    `Valid/invalid rollout count: ${projection.validMetricCalls}/${projection.invalidMetricCalls}`,
    `Pylon assignment refs: ${joinRefs(projection.pylonBatchRefs)}`,
    `Artifact/proof refs: ${joinRefs([
      ...projection.artifactManifestRefs,
      ...proofBundleRefs,
    ])}`,
    `Verifier/scorer refs: ${joinRefs([...verifierRefs, ...scorerRefs])}`,
    `Policy findings: ${joinRefs(projection.policyFindingRefs)}`,
    `Blockers: ${joinRefs(projection.blockerRefs)}`,
    `Next action: ${joinRefs(projection.nextActionRefs)}`,
    `Evidence counts: retained=${publicSummary.evidenceCounts.retained}, validation=${publicSummary.evidenceCounts.validation}, holdout=${publicSummary.evidenceCounts.holdout}`,
    claimBoundaryLine,
    outcomeLine,
    postingAuthorityBoundary,
  ].join('\n')

  assertGeneratedCopySafe(bodyMarkdown)

  return new ProbeGepaForumSummaryDraft({
    bodyMarkdown,
    claimBoundaryLine,
    forumTopicRef: input.forumTopicRef,
    idempotencyKey,
    postingAuthorityBoundary,
    postingMode,
    schemaVersion: ProbeGepaForumSummarySchemaVersion,
    targetThreadRef: input.targetThreadRef,
    title,
  })
}

const joinRefs = (refs: ReadonlyArray<string>): string =>
  uniqueRefs(refs).length === 0 ? 'none' : uniqueRefs(refs).join(', ')

const claimBoundaryForState = (
  claimState: ProbeGepaCampaignProjection['claimState'],
): string => {
  switch (claimState) {
    case 'measured_retained_smoke':
      return 'Claim boundary: measured retained smoke only; this is not a public benchmark score.'
    case 'retained_summary':
      return 'Claim boundary: retained evidence summary only; this is not a public benchmark score.'
    case 'validation_measured_only':
      return 'Claim boundary: validation measured only; this is not frozen holdout performance.'
    case 'holdout_summary':
      return 'Claim boundary: holdout summary only; this is not a public ranking without a separate release authority.'
    case 'none':
      return 'Claim boundary: no public benchmark claim.'
  }
}

const assertGeneratedCopySafe = (copy: string): void => {
  if (
    unsafeRefPattern.test(copy) ||
    rawTimestampPattern.test(copy) ||
    /beats terminal-bench|public benchmark score|frozen holdout performance/i.test(
      copy.replace(
        /this is not a public benchmark score|this is not frozen holdout performance/gi,
        '',
      ),
    )
  ) {
    throw new ProbeGepaForumSummaryUnsafe({
      reason:
        'Generated Forum copy contains unsafe material or overclaims retained, validation, or holdout evidence.',
    })
  }
}

export const probeGepaForumSummaryFromUnknown = (
  value: unknown,
): ProbeGepaForumSummaryDraft =>
  S.decodeUnknownSync(ProbeGepaForumSummaryDraft)(value)

export const projectionUnsafeToForumUnsafe = (
  error: ProbeGepaCampaignProjectionUnsafe,
): ProbeGepaForumSummaryUnsafe =>
  new ProbeGepaForumSummaryUnsafe({ reason: error.reason })
