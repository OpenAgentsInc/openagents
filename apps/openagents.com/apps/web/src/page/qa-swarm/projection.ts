import { Schema as S } from 'effect'

export const QaSwarmVerdict = S.Literals([
  'passed',
  'failed',
  'warning',
  'inconclusive',
])
export type QaSwarmVerdict = typeof QaSwarmVerdict.Type

export const QaSwarmTargetVisibility = S.Literals(['public', 'opaque'])
export type QaSwarmTargetVisibility = typeof QaSwarmTargetVisibility.Type

export class QaSwarmTargetProjection extends S.Class<QaSwarmTargetProjection>(
  'QaSwarmTargetProjection',
)({
  label: S.String,
  ref: S.String,
  visibility: QaSwarmTargetVisibility,
}) {}

export class QaSwarmVerdictItem extends S.Class<QaSwarmVerdictItem>(
  'QaSwarmVerdictItem',
)({
  label: S.String,
  receiptRef: S.String,
  summary: S.String,
  verdict: QaSwarmVerdict,
}) {}

export class QaSwarmCoverageFrontierItem extends S.Class<QaSwarmCoverageFrontierItem>(
  'QaSwarmCoverageFrontierItem',
)({
  current: S.Number,
  frontier: S.Number,
  label: S.String,
  receiptRef: S.String,
}) {}

export class QaSwarmPerfBudgetItem extends S.Class<QaSwarmPerfBudgetItem>(
  'QaSwarmPerfBudgetItem',
)({
  actualMs: S.Number,
  budgetMs: S.Number,
  label: S.String,
  receiptRef: S.String,
  verdict: QaSwarmVerdict,
}) {}

export class QaSwarmVideoRef extends S.Class<QaSwarmVideoRef>(
  'QaSwarmVideoRef',
)({
  label: S.String,
  posterRef: S.String,
  traceHref: S.String,
  videoRef: S.String,
}) {}

export class QaSwarmDistilledTestRef extends S.Class<QaSwarmDistilledTestRef>(
  'QaSwarmDistilledTestRef',
)({
  href: S.String,
  label: S.String,
  receiptRef: S.String,
}) {}

export class QaSwarmEngagementProjection extends S.Class<QaSwarmEngagementProjection>(
  'QaSwarmEngagementProjection',
)({
  cadence: S.Literal('weekly'),
  reportHref: S.String,
  reportRef: S.String,
  sourceArtifactRef: S.String,
  status: S.Literal('standing_customer_one'),
}) {}

export class QaSwarmFindingsLedgerProjection extends S.Class<QaSwarmFindingsLedgerProjection>(
  'QaSwarmFindingsLedgerProjection',
)({
  caughtCount: S.Number,
  distilledRegressionCount: S.Number,
  filedIssueCount: S.Number,
  fixedCount: S.Number,
  ledgerRef: S.String,
  rows: S.Array(
    S.Struct({
      findingRef: S.String,
      issueRef: S.String,
      label: S.String,
      status: S.Literals(['caught', 'filed', 'fixed', 'distilled']),
      testRef: S.String,
    }),
  ),
}) {}

export class QaSwarmCaseStudyProjection extends S.Class<QaSwarmCaseStudyProjection>(
  'QaSwarmCaseStudyProjection',
)({
  href: S.String,
  receiptRef: S.String,
  summary: S.String,
  title: S.String,
}) {}

export class QaSwarmRunProjection extends S.Class<QaSwarmRunProjection>(
  'QaSwarmRunProjection',
)({
  caseStudy: QaSwarmCaseStudyProjection,
  coverageFrontier: S.Array(QaSwarmCoverageFrontierItem),
  distilledTests: S.Array(QaSwarmDistilledTestRef),
  engagement: QaSwarmEngagementProjection,
  findingsLedger: QaSwarmFindingsLedgerProjection,
  generatedAt: S.String,
  nightlyArtifactRef: S.String,
  opaqueTargetRefs: S.Array(S.String),
  perfBudgets: S.Array(QaSwarmPerfBudgetItem),
  projectionRef: S.String,
  publicSafetyRefs: S.Array(S.String),
  runRef: S.String,
  schemaVersion: S.Literal('openagents.qa_swarm.run_projection.v1'),
  staleness: S.Struct({
    contractVersion: S.Literal('projection_staleness.v1'),
    maxAgeHours: S.Number,
    mode: S.Literal('artifact_snapshot'),
  }),
  target: QaSwarmTargetProjection,
  title: S.String,
  traceRefs: S.Array(S.String),
  verdict: QaSwarmVerdict,
  verdictWall: S.Array(QaSwarmVerdictItem),
  videoRefs: S.Array(QaSwarmVideoRef),
}) {}

export const QA_SWARM_SAMPLE_RUN_REF = 'qa-run.khala-code-nightly.latest'
export const QA_SWARM_SEED_RUN_REF = 'qa-run.khala-code-nightly.2026-07-02'

const PRIVATE_MATERIAL_PATTERN =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|macaroon|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

const PUBLIC_REF_PATTERN =
  /^(artifact|coverage|frontier|perf|poster|projection|qa-run|redaction|test|trace|video)\.[a-z0-9][a-z0-9._-]*$/i

const isPublicRef = (value: string): boolean =>
  PUBLIC_REF_PATTERN.test(value) && !PRIVATE_MATERIAL_PATTERN.test(value)

const assertPublicRefs = (
  refs: ReadonlyArray<string>,
  field: string,
): void => {
  const unsafe = refs.find(ref => !isPublicRef(ref))
  if (unsafe !== undefined) {
    throw new Error(`Unsafe QA Swarm projection ref in ${field}: ${unsafe}`)
  }
}

export const qaSwarmProjectionHasPrivateMaterial = (
  projection: QaSwarmRunProjection,
): boolean => PRIVATE_MATERIAL_PATTERN.test(JSON.stringify(projection))

export const assertQaSwarmPublicProjection = (
  projection: QaSwarmRunProjection,
): QaSwarmRunProjection => {
  const decoded = S.decodeUnknownSync(QaSwarmRunProjection)(projection)
  if (qaSwarmProjectionHasPrivateMaterial(decoded)) {
    throw new Error('QA Swarm projection contains private material')
  }

  assertPublicRefs([decoded.projectionRef], 'projectionRef')
  assertPublicRefs([decoded.nightlyArtifactRef], 'nightlyArtifactRef')
  assertPublicRefs([decoded.engagement.reportRef], 'engagement.reportRef')
  assertPublicRefs([decoded.engagement.sourceArtifactRef], 'engagement.sourceArtifactRef')
  assertPublicRefs([decoded.findingsLedger.ledgerRef], 'findingsLedger.ledgerRef')
  assertPublicRefs(
    decoded.findingsLedger.rows.flatMap(item => [
      item.findingRef,
      item.issueRef,
      item.testRef,
    ]),
    'findingsLedger.rows',
  )
  assertPublicRefs([decoded.caseStudy.receiptRef], 'caseStudy.receiptRef')
  assertPublicRefs(decoded.opaqueTargetRefs, 'opaqueTargetRefs')
  assertPublicRefs(decoded.publicSafetyRefs, 'publicSafetyRefs')
  assertPublicRefs(decoded.traceRefs, 'traceRefs')
  assertPublicRefs(
    decoded.verdictWall.map(item => item.receiptRef),
    'verdictWall.receiptRef',
  )
  assertPublicRefs(
    decoded.coverageFrontier.map(item => item.receiptRef),
    'coverageFrontier.receiptRef',
  )
  assertPublicRefs(
    decoded.perfBudgets.map(item => item.receiptRef),
    'perfBudgets.receiptRef',
  )
  assertPublicRefs(
    decoded.videoRefs.flatMap(item => [item.videoRef, item.posterRef]),
    'videoRefs',
  )
  assertPublicRefs(
    decoded.distilledTests.map(item => item.receiptRef),
    'distilledTests.receiptRef',
  )

  if (decoded.target.visibility === 'opaque') {
    assertPublicRefs([decoded.target.ref], 'target.ref')
  }

  return decoded
}

export const sampleQaSwarmRunProjection = assertQaSwarmPublicProjection(
  new QaSwarmRunProjection({
    caseStudy: {
      href: '/docs/qa/qa-swarm-khala-code-standing-engagement',
      receiptRef: 'artifact.qa_swarm.case_study.khala_code.20260702',
      summary:
        'The first audit session caught two main-branch regressions in stale visual smokes and one cockpit robustness bug before the standing loop was automated.',
      title: 'Case-study seed: the first Khala Code QA Swarm audit',
    },
    coverageFrontier: [
      {
        current: 42,
        frontier: 58,
        label: 'Seed corpus coverage',
        receiptRef: 'coverage.qa_swarm.khala_code.seed_corpus.20260702',
      },
      {
        current: 17,
        frontier: 24,
        label: 'Desktop state frontier',
        receiptRef: 'frontier.qa_swarm.khala_code.desktop_state.20260702',
      },
      {
        current: 9,
        frontier: 13,
        label: 'Regression tests distilled',
        receiptRef: 'test.qa_swarm.khala_code.distilled.20260702',
      },
    ],
    distilledTests: [
      {
        href: '/docs/qa/khala-code-mechanical-corpus',
        label: 'Mechanical corpus',
        receiptRef: 'test.qa_swarm.khala_code.mechanical_corpus.20260702',
      },
      {
        href: '/docs/qa/khala-code-error-state-corpus',
        label: 'Error-state corpus',
        receiptRef: 'test.qa_swarm.khala_code.error_states.20260702',
      },
    ],
    engagement: {
      cadence: 'weekly',
      reportHref: '/qa/qa-run.khala-code-nightly.latest',
      reportRef: 'artifact.qa_swarm.weekly_report.khala_code.latest',
      sourceArtifactRef: 'artifact.khala_code.qa_status_surface.latest',
      status: 'standing_customer_one',
    },
    findingsLedger: {
      caughtCount: 3,
      distilledRegressionCount: 1,
      filedIssueCount: 3,
      fixedCount: 2,
      ledgerRef: 'artifact.qa_swarm.findings_ledger.khala_code.20260702',
      rows: [
        {
          findingRef: 'artifact.qa_swarm.finding.visual_fleet_run_rpc.20260702',
          issueRef: 'artifact.qa_swarm.issue.visual_fleet_run_rpc.20260702',
          label: 'Fleet-run RPC visual smoke stale fixture',
          status: 'fixed',
          testRef: 'test.qa_swarm.khala_code.visual_fleet_run_rpc.20260702',
        },
        {
          findingRef: 'artifact.qa_swarm.finding.foldkit_cockpit_visual.20260702',
          issueRef: 'artifact.qa_swarm.issue.foldkit_cockpit_visual.20260702',
          label: 'Foldkit cockpit landing visual smoke stale fixture',
          status: 'fixed',
          testRef: 'test.qa_swarm.khala_code.foldkit_cockpit_visual.20260702',
        },
        {
          findingRef: 'artifact.qa_swarm.finding.cockpit_failed_rpc_blank.20260702',
          issueRef: 'artifact.qa_swarm.issue.cockpit_failed_rpc_blank.20260702',
          label: 'Cockpit blanks when one startup RPC fails',
          status: 'filed',
          testRef: 'test.qa_swarm.khala_code.error_state_pending.20260702',
        },
      ],
    },
    generatedAt: '2026-07-02T17:00:00.000Z',
    nightlyArtifactRef: 'artifact.qa_swarm.khala_code.nightly.20260702',
    opaqueTargetRefs: ['artifact.qa_swarm.target.opaque.customer_one'],
    perfBudgets: [
      {
        actualMs: 82,
        budgetMs: 100,
        label: 'Thread switch p95',
        receiptRef: 'perf.qa_swarm.khala_code.thread_switch_p95.20260702',
        verdict: 'passed',
      },
      {
        actualMs: 441,
        budgetMs: 500,
        label: 'Lifecycle to card p95',
        receiptRef: 'perf.qa_swarm.khala_code.lifecycle_card_p95.20260702',
        verdict: 'passed',
      },
      {
        actualMs: 1030,
        budgetMs: 1000,
        label: '25-agent tick p95',
        receiptRef: 'perf.qa_swarm.khala_code.tick_p95.20260702',
        verdict: 'warning',
      },
    ],
    projectionRef: 'projection.qa_swarm.run.khala_code.20260702',
    publicSafetyRefs: [
      'redaction.qa_swarm.public_projection.reviewed.20260702',
    ],
    runRef: QA_SWARM_SAMPLE_RUN_REF,
    schemaVersion: 'openagents.qa_swarm.run_projection.v1',
    staleness: {
      contractVersion: 'projection_staleness.v1',
      maxAgeHours: 24,
      mode: 'artifact_snapshot',
    },
    target: {
      label: 'Khala Code Desktop',
      ref: 'artifact.qa_swarm.target.opaque.customer_one',
      visibility: 'opaque',
    },
    title: 'Khala Code nightly QA swarm',
    traceRefs: [
      'trace.public.qa_swarm.khala_code.seed_corpus.20260702',
      'trace.public.qa_swarm.khala_code.desktop_frontier.20260702',
    ],
    verdict: 'warning',
    verdictWall: [
      {
        label: 'Login and workspace routing',
        receiptRef: 'artifact.qa_swarm.verdict.login_workspace.20260702',
        summary: 'Core public entrypoints passed the mechanical flow.',
        verdict: 'passed',
      },
      {
        label: 'Desktop command palette',
        receiptRef: 'artifact.qa_swarm.verdict.command_palette.20260702',
        summary: 'Explorer found one latency budget warning, no correctness failure.',
        verdict: 'warning',
      },
      {
        label: 'Trace and video archive',
        receiptRef: 'artifact.qa_swarm.verdict.trace_video.20260702',
        summary: 'Trace, screenshot, and video refs are dereferenceable public receipts.',
        verdict: 'passed',
      },
    ],
    videoRefs: [
      {
        label: 'Seed corpus replay',
        posterRef: 'poster.qa_swarm.khala_code.seed_corpus.20260702',
        traceHref: '/trace/24c6fea6-b271-46c6-a9a9-bc614440e9ef',
        videoRef: 'video.qa_swarm.khala_code.seed_corpus.20260702',
      },
      {
        label: 'Desktop frontier replay',
        posterRef: 'poster.qa_swarm.khala_code.desktop_frontier.20260702',
        traceHref: '/trace/db838bdc-3bc6-48a5-8715-a6669f6b10c5',
        videoRef: 'video.qa_swarm.khala_code.desktop_frontier.20260702',
      },
    ],
  }),
)

export const lookupQaSwarmRunProjection = (
  runRef: string,
): QaSwarmRunProjection | null =>
  runRef === QA_SWARM_SAMPLE_RUN_REF || runRef === QA_SWARM_SEED_RUN_REF
    ? sampleQaSwarmRunProjection
    : null
