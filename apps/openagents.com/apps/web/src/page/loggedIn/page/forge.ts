// Forge factory dashboard — the software-factory pipeline view.
//
// This view projects the Forge production line (signal -> triage -> code gen ->
// validate -> release -> document -> monitor -> deploy) over the SAME real data
// sources the Forge cockpit already loads:
//
//   - Runs (`autopilotWorkList`)      -> per-stage throughput, cycle time,
//                                          backlog, pass rate, ship rate
//   - the provider-account pool       -> Code Gen capacity / "where work runs"
//   - the overnight report counts     -> awaiting-decision / blocked signal
//   - Customer #1 cohort projection   -> public-safe dogfood loop readiness
//
// Every number is tagged with its provenance. A metric backed by a real
// projection is labeled `live`; a metric with no real source yet is labeled
// `seeded` and is visibly dimmed. We never present a seeded number as live.
//
// Pipeline + panels are rendered on the shared `@openagentsinc/ui` dark
// contract (raw foldkit `html` + `Ui.className`, the same idiom as the
// cockpit in `autopilot-work.ts`). No client, partner, or person names appear.
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { autopilotWorkDetailRouter, forgeRouter } from '../../../route'
import { formatIsoDateTime } from '../../../time-format'
import * as Ui from '../../../ui'
import {
  type ForgeAutomation,
  type ForgeStageKey,
  configuredAutomationCountForStage,
  forgeAutomations,
  workOrderRefsForAutomation,
} from '../forge-automations'
import {
  type Message,
  SelectedForgeAutomationTemplate,
  SubmittedAutopilotWorkComposer,
  SubmittedForgeAutomationRun,
  UpdatedAutopilotWorkComposerField,
} from '../message'
import type {
  AutopilotWorkState,
  AutopilotWorkSummary,
  CustomerOneCohortProjection,
  CustomerOneCohortProjectionRow,
  Model,
  ProviderAccountPoolSummary,
} from '../model'

// ---------------------------------------------------------------------------
// Provenance: every surfaced number is either backed by a real projection
// (`live`), a configured local automation catalog entry (`configured`), or an
// honest placeholder (`seeded`). Seeded values are dimmed and carry a visible tag
// so an operator never mistakes a demo number for a fact.
// ---------------------------------------------------------------------------

type Provenance = 'configured' | 'live' | 'seeded'

interface Metric {
  readonly label: string
  readonly value: string
  readonly provenance: Provenance
}

interface PipelineStage {
  readonly index: number | null
  readonly name: string
  readonly source: string
  readonly stageKey: ForgeStageKey
  readonly automations: number
  readonly automationsProvenance: Provenance
  readonly metrics: ReadonlyArray<Metric>
  readonly progress: StageProgressSummary
  readonly spark: ReadonlyArray<number>
  readonly sparkProvenance: Provenance
}

interface StageProgressSummary {
  readonly active: number
  readonly blocked: number
  readonly completed: number
  readonly failed: number
  readonly omittedUnsafeRefCount: number
  readonly pending: number
  readonly provenance: Provenance
  readonly refs: ReadonlyArray<string>
  readonly total: number
}

interface DetailPanel {
  readonly title: string
  readonly value: string
  readonly unit: string
  readonly pill: string
  readonly delta: number | null
  readonly band: ReadonlyArray<number>
  readonly provenance: Provenance
  readonly note: string
}

interface DogfoodMetric {
  readonly key: string
  readonly label: string
  readonly value: string
  readonly provenance: Provenance
}

type CohortGateStatus = 'blocked' | 'loading' | 'ready' | 'unavailable'

interface RoutingDigest {
  readonly blocked: number
  readonly fallback: number
  readonly metered: number
  readonly requesterPylon: number
}

// ---------------------------------------------------------------------------
// Real-data derivation from the loaded Runs projection.
// ---------------------------------------------------------------------------

const integerFormatter = new Intl.NumberFormat('en-US')
const formatInt = (value: number): string => integerFormatter.format(value)

// Map a Run state onto the production-line stage it currently sits in. This is
// the real bucketing that feeds throughput, backlog, and pass-rate counts.
const stageOf = (state: AutopilotWorkState): string => {
  switch (state) {
    case 'scheduled':
      return 'triage'
    case 'queued_or_running':
      return 'codegen'
    case 'access_required':
    case 'payment_required':
    case 'paid_ready':
      return 'codegen'
    case 'delivered':
    case 'revision_required':
      return 'validate'
    case 'accepted':
    case 'accepted_free_slice':
      return 'release'
    case 'rejected':
    case 'invalid':
      return 'monitor'
    case 'blocked':
      return 'monitor'
  }
}

interface RunDigest {
  readonly total: number
  readonly byStage: Readonly<Record<string, number>>
  readonly byState: ReadonlyMap<AutopilotWorkState, number>
  // Median entry->exit minutes, derived from createdAt -> updatedAt.
  readonly medianCycleMinutes: number | null
  // Daily created counts over the trailing 14 days (oldest -> newest).
  readonly dailyCreated: ReadonlyArray<number>
  // Accepted / (accepted + rejected + invalid) over the loaded window.
  readonly passRate: number | null
  readonly accepted: number
  readonly delivered: number
  readonly rejectedOrInvalid: number
  readonly blocked: number
  readonly scheduled: number
}

const emptyDigest: RunDigest = {
  total: 0,
  byStage: {},
  byState: new Map(),
  medianCycleMinutes: null,
  dailyCreated: [],
  passRate: null,
  accepted: 0,
  delivered: 0,
  rejectedOrInvalid: 0,
  blocked: 0,
  scheduled: 0,
}

const median = (values: ReadonlyArray<number>): number | null => {
  if (values.length === 0) {
    return null
  }
  const sorted = [...values].sort((a, b) => a - b)
  const mid = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 0
    ? (sorted[mid - 1]! + sorted[mid]!) / 2
    : sorted[mid]!
}

const digestRuns = (
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
  generatedAt: string,
): RunDigest => {
  if (workOrders.length === 0) {
    return emptyDigest
  }

  const byStage: Record<string, number> = {}
  const byState = new Map<AutopilotWorkState, number>()
  const cycleMinutes: Array<number> = []
  const now = Date.parse(generatedAt)
  const dayCount = 14
  const daily = new Array<number>(dayCount).fill(0)

  let accepted = 0
  let delivered = 0
  let rejectedOrInvalid = 0
  let blocked = 0
  let scheduled = 0

  for (const order of workOrders) {
    const stage = stageOf(order.state)
    byStage[stage] = (byStage[stage] ?? 0) + 1
    byState.set(order.state, (byState.get(order.state) ?? 0) + 1)

    if (order.state === 'accepted' || order.state === 'accepted_free_slice') {
      accepted += 1
    }
    if (order.state === 'delivered' || order.state === 'revision_required') {
      delivered += 1
    }
    if (order.state === 'rejected' || order.state === 'invalid') {
      rejectedOrInvalid += 1
    }
    if (order.state === 'blocked') {
      blocked += 1
    }
    if (order.state === 'scheduled') {
      scheduled += 1
    }

    const created = Date.parse(order.createdAt)
    const updated = Date.parse(order.updatedAt)
    if (
      Number.isFinite(created) &&
      Number.isFinite(updated) &&
      updated >= created
    ) {
      cycleMinutes.push((updated - created) / 60_000)
    }

    if (Number.isFinite(created) && Number.isFinite(now)) {
      const ageDays = Math.floor((now - created) / 86_400_000)
      if (ageDays >= 0 && ageDays < dayCount) {
        // index 0 = oldest day in the window, dayCount-1 = today
        daily[dayCount - 1 - ageDays] = (daily[dayCount - 1 - ageDays] ?? 0) + 1
      }
    }
  }

  const decided = accepted + rejectedOrInvalid
  const passRate = decided === 0 ? null : accepted / decided

  return {
    total: workOrders.length,
    byStage,
    byState,
    medianCycleMinutes: median(cycleMinutes),
    dailyCreated: daily,
    passRate,
    accepted,
    delivered,
    rejectedOrInvalid,
    blocked,
    scheduled,
  }
}

const safeStageRunRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,160}$/

const safeStageRunRef = (value: string): string | null => {
  const trimmed = value.trim()

  return safeStageRunRefPattern.test(trimmed) &&
    !/(?:\/Users\/|\/home\/|diff --git|raw[-_ ](?:patch|log|prompt|shell)|secret|token|mnemonic|preimage|invoice)/iu.test(
      trimmed,
    )
    ? trimmed
    : null
}

const progressBucketForState = (
  state: AutopilotWorkState,
): 'active' | 'blocked' | 'completed' | 'failed' | 'pending' => {
  if (state === 'scheduled') {
    return 'pending'
  }

  if (state === 'paid_ready' || state === 'queued_or_running') {
    return 'active'
  }

  if (
    state === 'accepted' ||
    state === 'accepted_free_slice' ||
    state === 'delivered'
  ) {
    return 'completed'
  }

  if (
    state === 'access_required' ||
    state === 'blocked' ||
    state === 'payment_required' ||
    state === 'revision_required'
  ) {
    return 'blocked'
  }

  return 'failed'
}

const emptyStageProgress = (provenance: Provenance): StageProgressSummary => ({
  active: 0,
  blocked: 0,
  completed: 0,
  failed: 0,
  omittedUnsafeRefCount: 0,
  pending: 0,
  provenance,
  refs: [],
  total: 0,
})

const stageProgressFor = (
  stageKey: ForgeStageKey,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
  provenance: Provenance,
): StageProgressSummary =>
  workOrders.reduce((progress, order) => {
    if (stageOf(order.state) !== stageKey) {
      return progress
    }

    const bucket = progressBucketForState(order.state)
    const ref = safeStageRunRef(order.workOrderRef)

    return {
      ...progress,
      [bucket]: progress[bucket] + 1,
      omittedUnsafeRefCount:
        progress.omittedUnsafeRefCount + (ref === null ? 1 : 0),
      refs: ref === null ? progress.refs : [...progress.refs, ref],
      total: progress.total + 1,
    }
  }, emptyStageProgress(provenance))

const formatCycle = (minutes: number | null): string => {
  if (minutes === null) {
    return '—'
  }
  if (minutes < 1) {
    return '<1m'
  }
  if (minutes < 60) {
    return `${Math.round(minutes)}m`
  }
  const hours = minutes / 60
  if (hours < 48) {
    return `${hours.toFixed(1)}h`
  }
  return `${Math.round(hours / 24)}d`
}

const formatPct = (rate: number | null): string =>
  rate === null ? '—' : `${(rate * 100).toFixed(1)}%`

// ---------------------------------------------------------------------------
// Stage + panel assembly. The right-hand "Forge source" wiring is documented
// inline so the provenance choices stay auditable.
// ---------------------------------------------------------------------------

const buildStages = (
  digest: RunDigest,
  pool: ProviderAccountPoolSummary | null,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
  runsLoaded: boolean,
): ReadonlyArray<PipelineStage> => {
  const stageCount = (key: string): number => digest.byStage[key] ?? 0
  const hasRuns = digest.total > 0
  const progress = (stageKey: ForgeStageKey): StageProgressSummary =>
    stageProgressFor(
      stageKey,
      runsLoaded ? workOrders : [],
      runsLoaded ? 'live' : 'seeded',
    )

  // A real per-stage trailing series is only meaningful for the whole intake;
  // we reuse the real daily-created series as the Triage/Signal sparkline and
  // dim the rest as seeded so we never imply per-stage history we don't have.
  const seededSpark = [3, 5, 4, 6, 5, 7, 6]

  return [
    {
      index: null,
      name: 'Signal',
      source: 'Inbound intent feeding Run intake',
      stageKey: 'signal',
      automations: configuredAutomationCountForStage('signal'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'Runs in / window',
          value: formatInt(digest.dailyCreated.reduce((a, b) => a + b, 0)),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        { label: 'Input sources', value: '—', provenance: 'seeded' },
      ],
      progress: progress('signal'),
      spark: hasRuns ? digest.dailyCreated.slice(-7) : seededSpark,
      sparkProvenance: hasRuns ? 'live' : 'seeded',
    },
    {
      index: 1,
      name: 'Triage',
      source: 'Run intake / scoping; backlog = scheduled Runs',
      stageKey: 'triage',
      automations: configuredAutomationCountForStage('triage'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'In triage',
          value: formatInt(stageCount('triage')),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        {
          label: 'Backlog',
          value: formatInt(digest.scheduled),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        {
          label: 'Cycle',
          value: formatCycle(digest.medianCycleMinutes),
          provenance: digest.medianCycleMinutes === null ? 'seeded' : 'live',
        },
      ],
      progress: progress('triage'),
      spark: hasRuns ? digest.dailyCreated.slice(-7) : seededSpark,
      sparkProvenance: hasRuns ? 'live' : 'seeded',
    },
    {
      index: 2,
      name: 'Code Gen',
      source: 'Runs on materialized Workspaces; capacity = account pool',
      stageKey: 'codegen',
      automations: configuredAutomationCountForStage('codegen'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'Running',
          value: formatInt(stageCount('codegen')),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        {
          label: 'Eligible nodes',
          value: pool === null ? '—' : formatInt(pool.eligible),
          provenance: pool === null ? 'seeded' : 'live',
        },
        {
          label: 'Cycle',
          value: formatCycle(digest.medianCycleMinutes),
          provenance: digest.medianCycleMinutes === null ? 'seeded' : 'live',
        },
      ],
      progress: progress('codegen'),
      spark: seededSpark,
      sparkProvenance: 'seeded',
    },
    {
      index: 3,
      name: 'Validate',
      source: 'Verification reports + checks; merge gate',
      stageKey: 'validate',
      automations: configuredAutomationCountForStage('validate'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'In review',
          value: formatInt(stageCount('validate')),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        {
          label: 'Pass rate',
          value: formatPct(digest.passRate),
          provenance: digest.passRate === null ? 'seeded' : 'live',
        },
      ],
      progress: progress('validate'),
      spark: seededSpark,
      sparkProvenance: 'seeded',
    },
    {
      index: 4,
      name: 'Release',
      source: 'Delivery gating -> accepted-outcome receipts',
      stageKey: 'release',
      automations: configuredAutomationCountForStage('release'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'Accepted',
          value: formatInt(digest.accepted),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        {
          label: 'Pass rate',
          value: formatPct(digest.passRate),
          provenance: digest.passRate === null ? 'seeded' : 'live',
        },
      ],
      progress: progress('release'),
      spark: seededSpark,
      sparkProvenance: 'seeded',
    },
    {
      index: 5,
      name: 'Document',
      source: 'Documentation Runs (no dedicated projection yet)',
      stageKey: 'document',
      automations: configuredAutomationCountForStage('document'),
      automationsProvenance: 'configured',
      metrics: [
        { label: 'Docs / wk', value: '—', provenance: 'seeded' },
        { label: 'Pages / wk', value: '—', provenance: 'seeded' },
      ],
      progress: progress('document'),
      spark: seededSpark,
      sparkProvenance: 'seeded',
    },
    {
      index: 6,
      name: 'Monitor',
      source: 'Post-delivery signals: blocked / rejected Runs',
      stageKey: 'monitor',
      automations: configuredAutomationCountForStage('monitor'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'Incidents',
          value: formatInt(digest.blocked + digest.rejectedOrInvalid),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        { label: 'Token eff', value: '—', provenance: 'seeded' },
        { label: 'MTTR', value: '—', provenance: 'seeded' },
      ],
      progress: progress('monitor'),
      spark: seededSpark,
      sparkProvenance: 'seeded',
    },
    {
      index: null,
      name: 'Deploy',
      source: 'Completed delivery receipts (accepted outcomes)',
      stageKey: 'deploy',
      automations: configuredAutomationCountForStage('deploy'),
      automationsProvenance: 'configured',
      metrics: [
        {
          label: 'Accepted / window',
          value: formatInt(digest.accepted),
          provenance: hasRuns ? 'live' : 'seeded',
        },
        { label: 'KPIs', value: '—', provenance: 'seeded' },
      ],
      progress: progress('deploy'),
      spark: seededSpark,
      sparkProvenance: 'seeded',
    },
  ]
}

const buildPanels = (digest: RunDigest): ReadonlyArray<DetailPanel> => {
  const hasRuns = digest.total > 0
  const triaged = digest.byStage.triage ?? 0
  // WoW delta from the real daily-created series: last 7 vs prior 7.
  const recent = digest.dailyCreated.slice(-7).reduce((a, b) => a + b, 0)
  const prior = digest.dailyCreated.slice(-14, -7).reduce((a, b) => a + b, 0)
  const intakeDelta =
    prior === 0 ? null : Math.round(((recent - prior) / prior) * 100)
  const realBand = digest.dailyCreated.length > 0 ? digest.dailyCreated : []
  const seededBand = [4, 6, 5, 8, 7, 9, 8, 10, 9, 11, 10, 12, 11, 13]

  return [
    {
      title: 'Runs Triaged',
      value: formatInt(triaged),
      unit: 'RUNS',
      pill: 'QUEUE BURN',
      delta: intakeDelta,
      band: realBand.length > 0 ? realBand : seededBand,
      provenance: hasRuns ? 'live' : 'seeded',
      note: 'Runs currently in triage, including scheduled backlog (from Runs projection).',
    },
    {
      title: 'Checks Validated',
      value: '—',
      unit: 'CHECKS',
      pill: 'MERGE GATE',
      delta: null,
      band: seededBand,
      provenance: 'seeded',
      note: 'No verification-check projection wired yet — placeholder.',
    },
    {
      title: 'Outcomes Shipped',
      value: formatInt(digest.accepted),
      unit: 'RECEIPTS',
      pill: 'SHIP RATE',
      delta: null,
      band: realBand.length > 0 ? realBand : seededBand,
      provenance: hasRuns ? 'live' : 'seeded',
      note: 'Accepted-outcome receipts (accepted Runs).',
    },
    {
      title: 'Incidents Processed',
      value: formatInt(digest.blocked + digest.rejectedOrInvalid),
      unit: 'INC',
      pill: 'RELIABILITY',
      delta: null,
      band: realBand.length > 0 ? realBand : seededBand,
      provenance: hasRuns ? 'live' : 'seeded',
      note: 'Blocked + rejected/invalid Runs (Monitor signal).',
    },
  ]
}

const buildDogfoodMetrics = (
  data: FactoryData,
): ReadonlyArray<DogfoodMetric> => {
  const openWork = Math.max(
    0,
    data.digest.total - data.digest.accepted - data.digest.rejectedOrInvalid,
  )

  return [
    {
      key: 'open-work',
      label: 'Open work',
      value: data.runsLoaded ? formatInt(openWork) : '—',
      provenance: data.runsLoaded ? 'live' : 'seeded',
    },
    {
      key: 'eligible-nodes',
      label: 'Eligible nodes',
      value: data.pool === null ? '—' : formatInt(data.pool.eligible),
      provenance: data.pool === null ? 'seeded' : 'live',
    },
    {
      key: 'accepted',
      label: 'Accepted',
      value: data.runsLoaded ? formatInt(data.digest.accepted) : '—',
      provenance: data.runsLoaded ? 'live' : 'seeded',
    },
    {
      key: 'incidents',
      label: 'Incidents',
      value: data.runsLoaded
        ? formatInt(data.digest.blocked + data.digest.rejectedOrInvalid)
        : '—',
      provenance: data.runsLoaded ? 'live' : 'seeded',
    },
  ]
}

const digestRouting = (
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
): RoutingDigest =>
  workOrders.reduce(
    (digest, order) => {
      const routing = order.routing

      if (routing === undefined) {
        return digest
      }

      return {
        blocked:
          digest.blocked +
          (routing.source === 'none_available' ||
          routing.availabilityState !== 'selected'
            ? 1
            : 0),
        fallback: digest.fallback + (routing.source === 'fallback' ? 1 : 0),
        metered: digest.metered + (routing.buyerDebitRequired === true ? 1 : 0),
        requesterPylon:
          digest.requesterPylon +
          (routing.source === 'requester_pylon' ? 1 : 0),
      }
    },
    {
      blocked: 0,
      fallback: 0,
      metered: 0,
      requesterPylon: 0,
    } satisfies RoutingDigest,
  )

const buildRoutingMetrics = (
  data: FactoryData,
): ReadonlyArray<DogfoodMetric> => {
  const digest = digestRouting(data.workOrders)
  const provenance = data.runsLoaded ? 'live' : 'seeded'

  return [
    {
      key: 'requester-pylon',
      label: 'Owned nodes',
      provenance,
      value: data.runsLoaded ? formatInt(digest.requesterPylon) : '—',
    },
    {
      key: 'fallback-lanes',
      label: 'Fallback lanes',
      provenance,
      value: data.runsLoaded ? formatInt(digest.fallback) : '—',
    },
    {
      key: 'metered-work',
      label: 'Metered work',
      provenance,
      value: data.runsLoaded ? formatInt(digest.metered) : '—',
    },
    {
      key: 'blocked-routing',
      label: 'Blocked routing',
      provenance,
      value: data.runsLoaded ? formatInt(digest.blocked) : '—',
    },
  ]
}

const countCompletionBundles = (
  projection: CustomerOneCohortProjection,
): number =>
  projection.rows.filter(row => row.completionBundleRef !== undefined).length

const countPrivacyReviews = (projection: CustomerOneCohortProjection): number =>
  projection.rows.filter(row => row.privacyReviewRef !== undefined).length

const cohortGateStatus = (data: FactoryData): CohortGateStatus => {
  if (data.cohort !== null) {
    return data.cohort.gate.state
  }

  return data.cohortError === null ? 'loading' : 'unavailable'
}

const cohortGateLabel = (status: CohortGateStatus): string => {
  if (status === 'ready') {
    return 'Ready'
  }

  if (status === 'blocked') {
    return 'Blocked'
  }

  if (status === 'unavailable') {
    return 'Unavailable'
  }

  return 'Loading'
}

const cohortReadinessCopy = (data: FactoryData): string => {
  const status = cohortGateStatus(data)

  if (status === 'ready') {
    return 'Three public-safe completion bundles are recorded for Customer #1 dogfood.'
  }

  if (status === 'blocked') {
    return 'Customer #1 stays blocked until three loop-completion bundles pass privacy review.'
  }

  if (status === 'unavailable') {
    return 'Cohort evidence is unavailable, so the D3 gate stays blocked.'
  }

  return 'Loading public-safe cohort evidence for the D3 gate.'
}

const buildCohortReadinessMetrics = (
  data: FactoryData,
): ReadonlyArray<DogfoodMetric> => {
  const projection = data.cohort
  const provenance = projection === null ? 'seeded' : 'live'
  const target =
    projection === null
      ? '3-5'
      : `${projection.target.minimumCompletedTeams}-${projection.target.maximumTargetTeams}`

  return [
    {
      key: 'target-teams',
      label: 'Target teams',
      provenance: projection === null ? 'configured' : 'live',
      value: target,
    },
    {
      key: 'completion-bundles',
      label: 'Completion bundles',
      provenance,
      value:
        projection === null
          ? '—'
          : formatInt(countCompletionBundles(projection)),
    },
    {
      key: 'privacy-reviews',
      label: 'Privacy reviews',
      provenance,
      value:
        projection === null ? '—' : formatInt(countPrivacyReviews(projection)),
    },
    {
      key: 'gate-status',
      label: 'D3 gate',
      provenance,
      value: cohortGateLabel(cohortGateStatus(data)),
    },
  ]
}

// ---------------------------------------------------------------------------
// Rendering primitives (dark contract, no model-authored markup).
// ---------------------------------------------------------------------------

const provenanceTag = (provenance: Provenance): Html => {
  const h = html<Message>()
  const cls = (() => {
    if (provenance === 'live') {
      return 'border-[#1b5e20] text-[#7ccf8a]'
    }
    if (provenance === 'configured') {
      return 'border-[#24415f] text-[#8fc8ff]'
    }
    return 'border-[#5a3b00] text-[#ffb400]'
  })()

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex min-h-5 items-center border px-1.5 text-[0.5625rem] uppercase tracking-wide ${cls}`,
      ),
    ],
    [provenance],
  )
}

// A minimal inline sparkline. Seeded series render dimmed.
const sparkline = (
  values: ReadonlyArray<number>,
  provenance: Provenance,
): Html => {
  const h = html<Message>()
  const points = values.length < 2 ? [0, 0, ...values] : values
  const max = Math.max(...points, 1)
  const min = Math.min(...points, 0)
  const span = max - min || 1
  const width = 96
  const height = 24
  const step = points.length <= 1 ? width : width / (points.length - 1)
  const coords = points
    .map((value, index) => {
      const x = (index * step).toFixed(1)
      const y = (height - ((value - min) / span) * height).toFixed(1)
      return `${x},${y}`
    })
    .join(' ')
  const stroke = provenance === 'live' ? '#7ccf8a' : '#5a5a5a'

  return h.svg(
    [
      h.ViewBox(`0 0 ${width} ${height}`),
      h.AriaHidden(true),
      Ui.className<Message>('h-6 w-24'),
      h.Attribute('preserveAspectRatio', 'none'),
    ],
    [
      h.polyline(
        [
          h.Attribute('points', coords),
          h.Attribute('fill', 'none'),
          h.Attribute('stroke', stroke),
          h.Attribute('stroke-width', '1.5'),
        ],
        [],
      ),
    ],
  )
}

// Banded trend chart (MAX / MID / MIN guides + the live/seeded line).
const bandedChart = (
  values: ReadonlyArray<number>,
  provenance: Provenance,
): Html => {
  const h = html<Message>()
  const points = values.length < 2 ? [0, ...values, 0] : values
  const max = Math.max(...points, 1)
  const min = Math.min(...points, 0)
  const span = max - min || 1
  const width = 240
  const height = 56
  const step = points.length <= 1 ? width : width / (points.length - 1)
  const coords = points
    .map((value, index) => {
      const x = (index * step).toFixed(1)
      const y = (height - ((value - min) / span) * height).toFixed(1)
      return `${x},${y}`
    })
    .join(' ')
  const stroke = provenance === 'live' ? '#7ccf8a' : '#5a5a5a'

  return h.svg(
    [
      h.ViewBox(`0 0 ${width} ${height}`),
      h.AriaHidden(true),
      Ui.className<Message>('h-14 w-full'),
      h.Attribute('preserveAspectRatio', 'none'),
    ],
    [
      h.line(
        [
          h.Attribute('x1', '0'),
          h.Attribute('y1', '1'),
          h.Attribute('x2', `${width}`),
          h.Attribute('y2', '1'),
          h.Attribute('stroke', '#222'),
          h.Attribute('stroke-width', '1'),
        ],
        [],
      ),
      h.line(
        [
          h.Attribute('x1', '0'),
          h.Attribute('y1', `${height / 2}`),
          h.Attribute('x2', `${width}`),
          h.Attribute('y2', `${height / 2}`),
          h.Attribute('stroke', '#1a1a1a'),
          h.Attribute('stroke-width', '1'),
        ],
        [],
      ),
      h.line(
        [
          h.Attribute('x1', '0'),
          h.Attribute('y1', `${height - 1}`),
          h.Attribute('x2', `${width}`),
          h.Attribute('y2', `${height - 1}`),
          h.Attribute('stroke', '#222'),
          h.Attribute('stroke-width', '1'),
        ],
        [],
      ),
      h.polyline(
        [
          h.Attribute('points', coords),
          h.Attribute('fill', 'none'),
          h.Attribute('stroke', stroke),
          h.Attribute('stroke-width', '1.5'),
        ],
        [],
      ),
    ],
  )
}

const metricCell = (metric: Metric): Html => {
  const h = html<Message>()
  const dim = metric.provenance === 'seeded' ? 'text-white/35' : 'text-white/85'

  return h.div(
    [Ui.className<Message>('grid gap-0.5')],
    [
      h.div(
        [
          Ui.className<Message>(
            'text-[0.5625rem] uppercase tracking-wide text-white/35',
          ),
        ],
        [metric.label],
      ),
      h.div(
        [Ui.className<Message>(`text-sm font-medium tabular-nums ${dim}`)],
        [metric.value],
      ),
    ],
  )
}

const stageProgressChip = (
  stageKey: ForgeStageKey,
  label: string,
  value: number,
  tone: 'active' | 'blocked' | 'completed' | 'failed' | 'pending',
  provenance: Provenance,
): Html => {
  const h = html<Message>()
  const toneClass =
    provenance === 'seeded'
      ? 'border-[#333] text-white/35'
      : tone === 'completed'
        ? 'border-[#1b5e20] text-[#7ccf8a]'
        : tone === 'failed'
          ? 'border-[#5c1f1f] text-[#ff8a80]'
          : tone === 'blocked'
            ? 'border-[#5a3b00] text-[#ffb400]'
            : tone === 'active'
              ? 'border-[#1d3d63] text-[#8ab4ff]'
              : 'border-[#333] text-white/55'

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex min-h-5 items-center gap-1 border px-1.5 text-[0.5625rem] uppercase tracking-wide tabular-nums ${toneClass}`,
      ),
      h.DataAttribute('forge-stage-progress-chip', tone),
      h.DataAttribute('forge-stage-progress-stage', stageKey),
      h.DataAttribute('forge-stage-progress-value', String(value)),
    ],
    [`${label} ${formatInt(value)}`],
  )
}

const stageProgressRunLinks = (
  progress: StageProgressSummary,
): ReadonlyArray<Html> => {
  const h = html<Message>()

  return progress.refs
    .slice(0, 3)
    .map(ref =>
      h.a(
        [
          h.Href(autopilotWorkDetailRouter({ workOrderRef: ref })),
          Ui.className<Message>(
            'min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-[0.625rem] text-white/55 underline decoration-white/20 underline-offset-3 hover:text-white',
          ),
          h.DataAttribute('forge-stage-progress-run', ref),
        ],
        [ref],
      ),
    )
}

const stageProgressView = (
  stageKey: ForgeStageKey,
  progress: StageProgressSummary,
): Html => {
  const h = html<Message>()
  const links = stageProgressRunLinks(progress)

  return h.div(
    [
      Ui.className<Message>('grid gap-2 border-t border-[#1b1b1b] pt-2'),
      h.DataAttribute('forge-stage-progress', stageKey),
      h.DataAttribute('forge-stage-progress-provenance', progress.provenance),
      h.DataAttribute('forge-stage-progress-total', String(progress.total)),
      h.DataAttribute('forge-stage-progress-active', String(progress.active)),
      h.DataAttribute('forge-stage-progress-pending', String(progress.pending)),
      h.DataAttribute(
        'forge-stage-progress-completed',
        String(progress.completed),
      ),
      h.DataAttribute('forge-stage-progress-blocked', String(progress.blocked)),
      h.DataAttribute('forge-stage-progress-failed', String(progress.failed)),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-1.5')],
        [
          stageProgressChip(
            stageKey,
            'active',
            progress.active,
            'active',
            progress.provenance,
          ),
          stageProgressChip(
            stageKey,
            'pending',
            progress.pending,
            'pending',
            progress.provenance,
          ),
          stageProgressChip(
            stageKey,
            'done',
            progress.completed,
            'completed',
            progress.provenance,
          ),
          stageProgressChip(
            stageKey,
            'blocked',
            progress.blocked,
            'blocked',
            progress.provenance,
          ),
          stageProgressChip(
            stageKey,
            'failed',
            progress.failed,
            'failed',
            progress.provenance,
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-1')],
        links.length === 0
          ? [
              h.span(
                [Ui.className<Message>('text-[0.625rem] text-white/30')],
                [
                  progress.provenance === 'live'
                    ? 'No Runs in stage'
                    : 'Awaiting Runs',
                ],
              ),
            ]
          : links,
      ),
      progress.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.span(
            [Ui.className<Message>('text-[0.625rem] text-[#ffb400]')],
            [`${progress.omittedUnsafeRefCount} unsafe Run ref(s) omitted`],
          ),
    ],
  )
}

const stageCard = (stage: PipelineStage): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-w-[10rem] flex-1 content-start gap-3 border border-[#222] bg-[#050505] p-3',
      ),
      h.DataAttribute('forge-stage-key', stage.stageKey),
      h.DataAttribute(
        'forge-stage-automation-count',
        String(stage.automations),
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-2')],
        [
          h.div(
            [Ui.className<Message>('flex items-center gap-2')],
            [
              stage.index === null
                ? h.span([Ui.className<Message>('hidden')], [])
                : h.span(
                    [
                      Ui.className<Message>(
                        'inline-flex size-5 items-center justify-center border border-[#333] text-[0.625rem] text-white/55',
                      ),
                    ],
                    [String(stage.index)],
                  ),
              h.div(
                [Ui.className<Message>('text-sm font-medium text-white/85')],
                [stage.name],
              ),
            ],
          ),
          sparkline(stage.spark, stage.sparkProvenance),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex items-center gap-2')],
        [
          h.span(
            [Ui.className<Message>('text-[0.625rem] text-white/40')],
            [`${formatInt(stage.automations)} automations`],
          ),
          provenanceTag(stage.automationsProvenance),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid grid-cols-2 gap-x-3 gap-y-2')],
        stage.metrics.map(metricCell),
      ),
      stageProgressView(stage.stageKey, stage.progress),
      h.div(
        [Ui.className<Message>('text-[0.5625rem] leading-snug text-white/25')],
        [stage.source],
      ),
    ],
  )
}

const deltaLabel = (delta: number | null): Html => {
  const h = html<Message>()
  if (delta === null) {
    return h.span([Ui.className<Message>('text-xs text-white/35')], ['— WoW'])
  }
  const positive = delta >= 0
  const cls = positive ? 'text-[#7ccf8a]' : 'text-[#ff8a80]'
  const arrow = positive ? '▲' : '▼'

  return h.span(
    [Ui.className<Message>(`text-xs ${cls}`)],
    [`${arrow} ${Math.abs(delta)}% WoW`],
  )
}

const detailPanelKey = (title: string): string =>
  title.toLowerCase().replaceAll(' ', '-')

const detailPanelCard = (panel: DetailPanel): Html => {
  const h = html<Message>()
  const panelKey = detailPanelKey(panel.title)
  const valueDim =
    panel.provenance === 'seeded' ? 'text-white/35' : 'text-white'

  return h.div(
    [
      Ui.className<Message>(
        'grid content-start gap-3 border border-[#222] bg-[#050505] p-4',
      ),
      h.DataAttribute('forge-detail-panel', panel.title),
      h.DataAttribute('forge-detail-panel-key', panelKey),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-start justify-between gap-2')],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.div(
                [Ui.className<Message>('text-sm font-medium text-white/80')],
                [panel.title],
              ),
              h.div(
                [Ui.className<Message>('flex items-baseline gap-1.5')],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        `text-2xl font-semibold tabular-nums ${valueDim}`,
                      ),
                      h.DataAttribute('forge-detail-panel-value', panel.title),
                      h.DataAttribute('forge-detail-panel-value-key', panelKey),
                      h.DataAttribute(
                        'forge-detail-panel-value-text',
                        panel.value,
                      ),
                    ],
                    [panel.value],
                  ),
                  h.span(
                    [
                      Ui.className<Message>(
                        'text-[0.625rem] uppercase text-white/35',
                      ),
                    ],
                    [panel.unit],
                  ),
                ],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid justify-items-end gap-1')],
            [
              provenanceTag(panel.provenance),
              h.span(
                [
                  Ui.className<Message>(
                    'inline-flex min-h-5 items-center border border-[#333] px-1.5 text-[0.5625rem] uppercase tracking-wide text-white/45',
                  ),
                ],
                [panel.pill],
              ),
            ],
          ),
        ],
      ),
      bandedChart(panel.band, panel.provenance),
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-2')],
        [
          deltaLabel(panel.delta),
          h.span(
            [Ui.className<Message>('text-[0.625rem] text-white/30')],
            ['MAX / MID / MIN'],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.625rem] leading-snug text-white/30')],
        [panel.note],
      ),
    ],
  )
}

const automationModeLabel = (automation: ForgeAutomation): string =>
  automation.mode === 'deterministic' ? 'deterministic' : 'AI assisted'

const automationRunLinks = (
  automation: ForgeAutomation,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
): ReadonlyArray<Html> => {
  const h = html<Message>()
  const refs = workOrderRefsForAutomation(automation, workOrders).slice(0, 2)

  return refs.map(ref =>
    h.a(
      [
        h.Href(autopilotWorkDetailRouter({ workOrderRef: ref })),
        Ui.className<Message>(
          'text-[0.6875rem] text-white/60 underline decoration-white/20 underline-offset-3 hover:text-white',
        ),
      ],
      [ref],
    ),
  )
}

const automationRow = (
  automation: ForgeAutomation,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
): Html => {
  const h = html<Message>()
  const runLinks = automationRunLinks(automation, workOrders)

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#050505] p-3 @2xl:grid-cols-[7rem_minmax(0,1fr)_8rem_10rem]',
      ),
      h.DataAttribute('forge-automation-id', automation.id),
      h.DataAttribute('forge-automation-stage', automation.stageKey),
    ],
    [
      h.div(
        [Ui.className<Message>('grid content-start gap-1')],
        [
          h.div(
            [Ui.className<Message>(Ui.eyebrowClass)],
            [automation.stageName],
          ),
          provenanceTag('configured'),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid min-w-0 gap-1')],
        [
          h.div(
            [
              Ui.className<Message>(
                'truncate text-sm font-medium text-white/85',
              ),
            ],
            [automation.label],
          ),
          h.div(
            [Ui.className<Message>('text-sm/6 text-white/45 sm:text-xs/5')],
            [automation.description],
          ),
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap gap-2 text-[0.6875rem] text-white/35',
              ),
            ],
            [
              h.span([], [automation.repositoryFullName]),
              h.span([], [`branch ${automation.branch}`]),
              h.span([], [automation.verificationCommand]),
            ],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid content-start gap-1 text-[0.6875rem] text-white/45',
          ),
        ],
        [
          h.span([Ui.className<Message>('uppercase text-white/30')], ['Mode']),
          h.span([], [automationModeLabel(automation)]),
          h.span([Ui.className<Message>('uppercase text-white/30')], ['Runs']),
          ...(runLinks.length === 0 ? [h.span([], ['none yet'])] : runLinks),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap content-start gap-2 @2xl:justify-end',
          ),
        ],
        [
          Ui.button<Message>({
            attrs: [
              h.Type('button'),
              h.OnClick(
                SelectedForgeAutomationTemplate({
                  automationId: automation.id,
                }),
              ),
            ],
            label: `Load ${automation.label}`,
            size: 'sm',
            variant: 'secondary',
          }),
          Ui.button<Message>({
            attrs: [
              h.Type('button'),
              h.OnClick(
                SubmittedForgeAutomationRun({ automationId: automation.id }),
              ),
            ],
            label: `Run ${automation.label}`,
            size: 'sm',
            variant: 'secondary',
          }),
        ],
      ),
    ],
  )
}

const automationCatalogSection = (
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 @container')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-base font-medium text-white/80',
                  ),
                ],
                ['Automations'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-sm/6 text-white/45')],
                [
                  'Configured units that staff each stage. Running one creates a real Autopilot work order.',
                ],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'text-[0.6875rem] uppercase tracking-wide text-white/35 tabular-nums',
              ),
              h.DataAttribute(
                'forge-automation-total',
                String(forgeAutomations.length),
              ),
            ],
            [`${formatInt(forgeAutomations.length)} configured`],
          ),
        ],
      ),
      h.ul(
        [Ui.className<Message>('grid gap-2'), h.Attribute('role', 'list')],
        forgeAutomations.map(automation =>
          automationRow(automation, workOrders),
        ),
      ),
    ],
  )
}

const automationRunStatus = (model: Model): Html | null => {
  const h = html<Message>()
  const state = model.autopilotWorkComposer

  if (state._tag === 'AutopilotWorkComposerSubmitting') {
    return h.p(
      [Ui.className<Message>('m-0 text-sm/6 text-white/45')],
      ['Submitting automation run...'],
    )
  }

  if (state._tag === 'AutopilotWorkComposerFailed') {
    return h.p(
      [Ui.className<Message>('m-0 text-sm/6 text-[#ff8a80]')],
      [state.error],
    )
  }

  if (state._tag === 'AutopilotWorkComposerSucceeded') {
    const ref = state.response.work.workOrderRef

    return h.p(
      [Ui.className<Message>('m-0 text-sm/6 text-[#7ccf8a]')],
      [
        'Created ',
        h.a(
          [
            h.Href(autopilotWorkDetailRouter({ workOrderRef: ref })),
            Ui.className<Message>(
              'underline decoration-[#7ccf8a]/30 underline-offset-3 hover:text-white',
            ),
          ],
          [ref],
        ),
        '. Evidence and delivery receipts now belong to that work-order lifecycle.',
      ],
    )
  }

  return null
}

const automationTuningSection = (model: Model): Html => {
  const h = html<Message>()
  const draft = model.autopilotWorkComposerDraft
  const submitting =
    model.autopilotWorkComposer._tag === 'AutopilotWorkComposerSubmitting'
  const status = automationRunStatus(model)

  return h.form(
    [
      Ui.className<Message>('grid gap-3 border border-[#222] bg-black p-4'),
      h.OnSubmit(SubmittedAutopilotWorkComposer()),
      h.DataAttribute('forge-automation-tuning', 'true'),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Add / tune']),
      h.label(
        [Ui.className<Message>('grid gap-2')],
        [
          h.span(
            [Ui.className<Message>('text-sm font-medium text-white/80')],
            ['Automation objective'],
          ),
          h.textarea(
            [
              h.Name('forge-automation-objective'),
              h.Value(draft.objective),
              h.Rows(3),
              h.OnInput(value =>
                UpdatedAutopilotWorkComposerField({
                  field: 'objective',
                  value,
                }),
              ),
              Ui.className<Message>(
                'min-h-24 resize-y border border-[#333] bg-[#050505] p-3 text-base/7 text-white/85 outline-none focus:border-white/45 sm:text-sm/6',
              ),
            ],
            [],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 md:grid-cols-[minmax(0,1.2fr)_8rem_10rem]',
          ),
        ],
        [
          h.label(
            [Ui.className<Message>('grid gap-2')],
            [
              h.span(
                [Ui.className<Message>('text-xs uppercase text-white/40')],
                ['Repository'],
              ),
              h.input([
                h.Name('forge-automation-repository'),
                h.Value(draft.repositoryFullName),
                h.OnInput(value =>
                  UpdatedAutopilotWorkComposerField({
                    field: 'repositoryFullName',
                    value,
                  }),
                ),
                Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
              ]),
            ],
          ),
          h.label(
            [Ui.className<Message>('grid gap-2')],
            [
              h.span(
                [Ui.className<Message>('text-xs uppercase text-white/40')],
                ['Branch'],
              ),
              h.input([
                h.Name('forge-automation-branch'),
                h.Value(draft.branch),
                h.OnInput(value =>
                  UpdatedAutopilotWorkComposerField({ field: 'branch', value }),
                ),
                Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
              ]),
            ],
          ),
          h.label(
            [Ui.className<Message>('grid gap-2')],
            [
              h.span(
                [Ui.className<Message>('text-xs uppercase text-white/40')],
                ['Budget cents'],
              ),
              h.input([
                h.Name('forge-automation-budget'),
                h.Type('number'),
                h.Value(draft.maxSpendCents),
                h.OnInput(value =>
                  UpdatedAutopilotWorkComposerField({
                    field: 'maxSpendCents',
                    value,
                  }),
                ),
                Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
              ]),
            ],
          ),
        ],
      ),
      h.label(
        [Ui.className<Message>('grid gap-2')],
        [
          h.span(
            [Ui.className<Message>('text-xs uppercase text-white/40')],
            ['Verification command'],
          ),
          h.input([
            h.Name('forge-automation-verification'),
            h.Value(draft.verificationCommand),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({
                field: 'verificationCommand',
                value,
              }),
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ],
      ),
      ...(status === null ? [] : [status]),
      Ui.button<Message>({
        attrs: [h.Type('submit'), ...(submitting ? [h.Disabled(true)] : [])],
        label: submitting ? 'Submitting...' : 'Run tuned automation',
        size: 'sm',
        variant: 'primary',
      }),
    ],
  )
}

// ---------------------------------------------------------------------------
// Page-level state handling. The factory reads the cockpit's projections; when
// none have loaded we say so honestly rather than render seeded numbers as live.
// ---------------------------------------------------------------------------

interface FactoryData {
  readonly digest: RunDigest
  readonly pool: ProviderAccountPoolSummary | null
  readonly cohort: CustomerOneCohortProjection | null
  readonly cohortError: string | null
  readonly generatedAt: string | null
  readonly runsLoaded: boolean
  readonly poolLoaded: boolean
  readonly error: string | null
  readonly workOrders: ReadonlyArray<AutopilotWorkSummary>
}

const readFactoryData = (model: Model): FactoryData => {
  const list = model.autopilotWorkList
  const poolState = model.providerAccountPool
  const cohortState = model.customerOneCohort

  const pool =
    poolState._tag === 'ProviderAccountPoolLoaded'
      ? poolState.response.summary
      : null
  const cohort =
    cohortState._tag === 'CustomerOneCohortLoaded' ? cohortState.response : null
  const cohortError =
    cohortState._tag === 'CustomerOneCohortFailed' ? cohortState.error : null

  if (list._tag === 'AutopilotWorkListLoaded') {
    const generatedAt = list.response.generatedAt
    return {
      cohort,
      cohortError,
      digest: digestRuns(list.response.workOrders, generatedAt),
      pool,
      generatedAt,
      runsLoaded: true,
      poolLoaded: poolState._tag === 'ProviderAccountPoolLoaded',
      error: null,
      workOrders: list.response.workOrders,
    }
  }

  return {
    cohort,
    cohortError,
    digest: emptyDigest,
    pool,
    generatedAt: null,
    runsLoaded: false,
    poolLoaded: poolState._tag === 'ProviderAccountPoolLoaded',
    error: list._tag === 'AutopilotWorkListFailed' ? list.error : null,
    workOrders: [],
  }
}

const liveIndicator = (data: FactoryData): Html => {
  const h = html<Message>()
  const live = data.runsLoaded
  const cls = live
    ? 'border-[#1b5e20] text-[#7ccf8a]'
    : 'border-[#5a3b00] text-[#ffb400]'

  return h.div(
    [Ui.className<Message>('flex items-center gap-2')],
    [
      h.span(
        [
          Ui.className<Message>(
            `inline-flex min-h-7 items-center gap-1.5 border px-2 text-[0.6875rem] uppercase tracking-wide ${cls}`,
          ),
        ],
        [
          h.span(
            [
              Ui.className<Message>(
                `size-1.5 ${live ? 'bg-[#7ccf8a]' : 'bg-[#ffb400]'}`,
              ),
            ],
            [],
          ),
          live ? 'Live data' : 'Awaiting data',
        ],
      ),
    ],
  )
}

const header = (data: FactoryData): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')],
    [
      h.div(
        [Ui.className<Message>('grid gap-1')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Forge']),
          h.h1(
            [Ui.className<Message>('m-0 text-2xl font-semibold text-white')],
            ['Factory'],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-sm/6 text-white/50')],
            [
              data.generatedAt === null
                ? 'Signal to deploy. Live numbers are tagged; placeholders are marked seeded.'
                : `Signal to deploy · generated ${formatIsoDateTime(data.generatedAt)}`,
            ],
          ),
        ],
      ),
      liveIndicator(data),
    ],
  )
}

const provenanceLegend = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'flex flex-wrap items-center gap-3 border border-[#222] bg-[#050505] px-3 py-2 text-[0.625rem] text-white/40',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center gap-1.5')],
        [provenanceTag('live'), h.span([], ['backed by a real projection'])],
      ),
      h.div(
        [Ui.className<Message>('flex items-center gap-1.5')],
        [
          provenanceTag('configured'),
          h.span([], ['configured automation catalog']),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex items-center gap-1.5')],
        [
          provenanceTag('seeded'),
          h.span([], ['placeholder — no live source wired yet']),
        ],
      ),
    ],
  )
}

const pipelineSection = (stages: ReadonlyArray<PipelineStage>): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.h2(
        [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
        ['Production line'],
      ),
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap gap-2 lg:flex-nowrap lg:overflow-x-auto',
          ),
        ],
        stages.map(stageCard),
      ),
    ],
  )
}

const panelSection = (panels: ReadonlyArray<DetailPanel>): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.h2(
        [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
        ['Detail panels'],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 sm:grid-cols-2 xl:grid-cols-4')],
        panels.map(detailPanelCard),
      ),
    ],
  )
}

const dogfoodMetricView = (metric: DogfoodMetric): Html => {
  const h = html<Message>()
  const valueClass =
    metric.provenance === 'live' ? 'text-white/85' : 'text-white/35'

  return h.div(
    [
      Ui.className<Message>('grid min-w-0 gap-1'),
      h.DataAttribute('forge-dogfood-metric', metric.key),
      h.DataAttribute('forge-dogfood-value', metric.value),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.5625rem] uppercase tracking-wide text-white/35',
          ),
        ],
        [metric.label],
      ),
      h.div(
        [
          Ui.className<Message>(
            `text-lg font-semibold tabular-nums ${valueClass}`,
          ),
        ],
        [metric.value],
      ),
      h.div(
        [Ui.className<Message>('justify-self-start')],
        [provenanceTag(metric.provenance)],
      ),
    ],
  )
}

const routingMetricView = (metric: DogfoodMetric): Html => {
  const h = html<Message>()
  const valueClass =
    metric.provenance === 'live' ? 'text-white/80' : 'text-white/35'

  return h.div(
    [
      Ui.className<Message>(
        'grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-[#1b1b1b] bg-black/30 px-3 py-2',
      ),
      h.DataAttribute('forge-routing-metric', metric.key),
      h.DataAttribute('forge-routing-value', metric.value),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.625rem] uppercase tracking-wide text-white/35',
          ),
        ],
        [metric.label],
      ),
      h.div(
        [
          Ui.className<Message>(
            `text-sm font-semibold tabular-nums ${valueClass}`,
          ),
        ],
        [metric.value],
      ),
    ],
  )
}

const cohortMetricView = (metric: DogfoodMetric): Html => {
  const h = html<Message>()
  const valueClass =
    metric.provenance === 'seeded' ? 'text-white/35' : 'text-white/70'

  return h.div(
    [
      Ui.className<Message>(
        'grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-[#1b1b1b] bg-black/30 px-3 py-2',
      ),
      h.DataAttribute('forge-cohort-metric', metric.key),
      h.DataAttribute('forge-cohort-value', metric.value),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.625rem] uppercase tracking-wide text-white/35',
          ),
        ],
        [metric.label],
      ),
      h.div(
        [
          Ui.className<Message>(
            `truncate text-sm font-semibold tabular-nums ${valueClass}`,
          ),
        ],
        [metric.value],
      ),
    ],
  )
}

const cohortRowStateLabel = (row: CustomerOneCohortProjectionRow): string => {
  if (row.countsTowardD3Completion) {
    return 'Complete'
  }

  if (row.state === 'blocked') {
    return 'Blocked'
  }

  if (row.state === 'deferred') {
    return 'Deferred'
  }

  if (row.privacyReviewRef !== undefined) {
    return 'Reviewed'
  }

  if (row.completionBundleRef !== undefined) {
    return 'Needs review'
  }

  return 'In progress'
}

const cohortRowView = (row: CustomerOneCohortProjectionRow): Html => {
  const h = html<Message>()
  const stateLabel = cohortRowStateLabel(row)
  const stateClass = row.countsTowardD3Completion
    ? 'border-[#1b5e20] text-[#7ccf8a]'
    : row.state === 'blocked'
      ? 'border-[#5a3b00] text-[#ffb400]'
      : row.state === 'deferred'
        ? 'border-[#333] text-white/40'
        : 'border-[#24415f] text-[#8fc8ff]'

  return h.div(
    [
      Ui.className<Message>(
        'grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-[#1b1b1b] bg-black/20 px-3 py-2',
      ),
      h.DataAttribute('forge-cohort-row', row.displayLabel),
      h.DataAttribute('forge-cohort-row-state', row.state),
      h.DataAttribute(
        'forge-cohort-row-complete',
        row.countsTowardD3Completion ? 'true' : 'false',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.6875rem] font-medium text-white/65',
          ),
        ],
        [row.displayLabel],
      ),
      h.div(
        [
          Ui.className<Message>(
            `inline-flex min-h-5 items-center border px-1.5 text-[0.5625rem] uppercase tracking-wide ${stateClass}`,
          ),
        ],
        [stateLabel],
      ),
    ],
  )
}

const cohortRowsView = (data: FactoryData): ReadonlyArray<Html> => {
  const h = html<Message>()

  if (data.cohort === null) {
    return []
  }

  if (data.cohort.rows.length === 0) {
    return [
      h.div(
        [
          Ui.className<Message>(
            'border border-[#1b1b1b] bg-black/20 px-3 py-2 text-[0.6875rem] text-white/35',
          ),
          h.DataAttribute('forge-cohort-empty', 'true'),
        ],
        ['No cohort rows recorded.'],
      ),
    ]
  }

  return data.cohort.rows.map(cohortRowView)
}

const cohortReadinessSection = (data: FactoryData): Html => {
  const h = html<Message>()
  const gate = cohortGateStatus(data)
  const completed =
    data.cohort === null ? '0' : formatInt(data.cohort.counts.loop_completed)

  return h.div(
    [
      Ui.className<Message>('grid gap-2 border-t border-[#1b1b1b] pt-3'),
      h.DataAttribute('forge-cohort-readiness', 'true'),
      h.DataAttribute('forge-cohort-gate', gate),
      h.DataAttribute('forge-cohort-completed', completed),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.625rem] uppercase tracking-wide text-white/35',
          ),
        ],
        ['Cohort readiness'],
      ),
      h.div(
        [Ui.className<Message>('text-sm/6 text-white/45')],
        [cohortReadinessCopy(data)],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 @2xl:grid-cols-4 sm:grid-cols-2')],
        buildCohortReadinessMetrics(data).map(cohortMetricView),
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-3')],
        cohortRowsView(data),
      ),
    ],
  )
}

const dogfoodFactorySection = (data: FactoryData): Html => {
  const h = html<Message>()
  const ready = data.runsLoaded && data.poolLoaded
  const status = ready ? 'live' : 'awaiting'
  const statusClass = ready
    ? 'border-[#1b5e20] text-[#7ccf8a]'
    : 'border-[#5a3b00] text-[#ffb400]'

  return h.section(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#050505] p-4 @container',
      ),
      h.DataAttribute('forge-dogfood-panel', 'true'),
      h.DataAttribute('forge-dogfood-status', status),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-start justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-base font-medium text-white/80',
                  ),
                ],
                ['Customer #1 factory'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-sm/6 text-white/45')],
                [
                  'Our own OpenAgents development pipeline, projected from Runs and provider-pool capacity.',
                ],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                `inline-flex min-h-7 items-center border px-2 text-[0.6875rem] uppercase tracking-wide ${statusClass}`,
              ),
            ],
            [ready ? 'Live dogfood loop' : 'Awaiting live dogfood data'],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 @2xl:grid-cols-4 sm:grid-cols-2')],
        buildDogfoodMetrics(data).map(dogfoodMetricView),
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 border-t border-[#1b1b1b] pt-3')],
        [
          h.div(
            [
              Ui.className<Message>(
                'truncate text-[0.625rem] uppercase tracking-wide text-white/35',
              ),
            ],
            ['Spend routing'],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-2 @2xl:grid-cols-4 sm:grid-cols-2',
              ),
            ],
            buildRoutingMetrics(data).map(routingMetricView),
          ),
        ],
      ),
      cohortReadinessSection(data),
    ],
  )
}

const statusNote = (data: FactoryData): Html | null => {
  const h = html<Message>()

  if (data.error !== null) {
    return h.p(
      [Ui.className<Message>('m-0 text-sm text-[#ff8a80]')],
      [
        `Runs projection failed to load: ${data.error}. Showing seeded placeholders only.`,
      ],
    )
  }

  if (!data.runsLoaded) {
    return h.p(
      [Ui.className<Message>('m-0 text-sm text-white/45')],
      [
        'Loading the Runs projection. Stage numbers populate once real data arrives; until then everything is marked seeded.',
      ],
    )
  }

  if (!data.poolLoaded) {
    return h.p(
      [Ui.className<Message>('m-0 text-sm text-white/35')],
      [
        'Account-pool capacity not loaded — Code Gen capacity is shown as seeded.',
      ],
    )
  }

  return null
}

export const view = (model: Model): Html => {
  const h = html<Message>()
  const data = readFactoryData(model)
  const stages = buildStages(
    data.digest,
    data.pool,
    data.workOrders,
    data.runsLoaded,
  )
  const panels = buildPanels(data.digest)
  const note = statusNote(data)

  return h.section(
    [
      Ui.className<Message>('grid gap-5'),
      h.DataAttribute('component', 'forge-factory-dashboard'),
      h.DataAttribute('route', forgeRouter()),
    ],
    [
      header(data),
      provenanceLegend(),
      ...(note === null ? [] : [note]),
      dogfoodFactorySection(data),
      pipelineSection(stages),
      automationCatalogSection(data.workOrders),
      automationTuningSection(model),
      panelSection(panels),
    ],
  )
}
