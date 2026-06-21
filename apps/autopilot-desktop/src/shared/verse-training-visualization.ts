import type {
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunNodeDefinition,
  TrainingRunVisualizationOptions,
  TrainingRunVector,
} from "@openagentsinc/three-effect/core"

import type {
  TrainingPromiseGatesResponse,
  TrainingPublicMetric,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
} from "./rpc"

export const VERSE_TASSADAR_CORE_NODE_ID = "verse-training:tassadar-run-core"
export const VERSE_TRAINING_NODE_PREFIX = "verse-training:"

const NETWORK_CENTER_NODE_ID = "network"

type VerseTrainingInput = Readonly<{
  trainingRuns: TrainingRunsResponse | null
  promiseGates?: TrainingPromiseGatesResponse | null
}>

type StageSpec = Readonly<{
  id: string
  label: string
  detail: string
  status: TrainingRunNodeDefinition["status"]
  position: TrainingRunVector
  connectedTo: readonly string[]
}>

const stagePositions = {
  assignment: [-2.85, -1.45, 0.58] as TrainingRunVector,
  trace: [-1.55, -2.18, 1.12] as TrainingRunVector,
  replay: [0, -2.42, 1.62] as TrainingRunVector,
  verdict: [1.55, -2.18, 1.08] as TrainingRunVector,
  settlement: [2.85, -1.45, 0.78] as TrainingRunVector,
  recipient: [3.32, -0.2, 1.36] as TrainingRunVector,
  blocked: [0, 1.72, 1.74] as TrainingRunVector,
} as const

const uniqueRefs = (refs: ReadonlyArray<string | null | undefined>): string[] => {
  const out: string[] = []
  for (const ref of refs) {
    if (typeof ref !== "string" || ref.length === 0 || out.includes(ref)) continue
    out.push(ref)
  }
  return out
}

const refsFromMetric = (metric: TrainingPublicMetric): string[] =>
  uniqueRefs(metric.sourceRefs)

const metricValue = (metric: TrainingPublicMetric): number =>
  Number.isFinite(metric.value) && metric.value > 0 ? metric.value : 0

export const selectedVerseTrainingSummary = (
  projection: TrainingRunsResponse | null,
): TrainingRunSummaryRow | null => {
  const summaries = projection?.summaries ?? []
  return (
    summaries.find(summary =>
      summary.run.promiseRef.includes("first_real_model_training_run"),
    ) ??
    summaries[0] ??
    null
  )
}

const publicRefDetail = (
  label: string,
  value: string,
  refs: ReadonlyArray<string>,
): string => {
  const suffix = refs.length === 0 ? "refs none" : `refs ${refs.slice(0, 4).join(", ")}`
  return `${label} ${value} · ${suffix}`
}

const stageNode = (spec: StageSpec): TrainingRunNodeDefinition => ({
  id: `${VERSE_TRAINING_NODE_PREFIX}${spec.id}`,
  label: spec.label,
  detail: spec.detail,
  role: spec.status === "blocked" ? "gate" : "lifecycle",
  status: spec.status,
  position: spec.position,
  connectedTo: spec.connectedTo,
})

const statusFromRun = (
  summary: TrainingRunSummaryRow | null,
  blockerCount: number,
): TrainingRunNodeDefinition["status"] => {
  if (blockerCount > 0) return "blocked"
  if (summary === null) return "queued"
  if (summary.run.state === "reconciled") return "verified"
  if (summary.run.state === "sealed") return "sealed"
  if (summary.run.state === "active") return "active"
  return "queued"
}

const rewriteBaseNodes = (
  baseNodes: readonly TrainingRunNodeDefinition[],
  summary: TrainingRunSummaryRow | null,
  blockerCount: number,
): readonly TrainingRunNodeDefinition[] => {
  const coreDetail =
    summary === null
      ? blockerCount > 0
        ? `Tassadar training blocked · ${blockerCount} public blockers`
        : "Tassadar training run core · waiting for public projection"
      : publicRefDetail(
          summary.run.state,
          summary.run.trainingRunRef,
          uniqueRefs([
            summary.run.trainingRunRef,
            summary.run.promiseRef,
            ...summary.run.sourceRefs,
            ...summary.sourceRefs,
          ]),
        )

  const rewritten = baseNodes.map((node) =>
    node.id === NETWORK_CENTER_NODE_ID
      ? {
          ...node,
          id: VERSE_TASSADAR_CORE_NODE_ID,
          label: "Tassadar",
          detail: coreDetail,
          role: "run" as const,
          status: statusFromRun(summary, blockerCount),
        }
      : {
          ...node,
          connectedTo: node.connectedTo?.map((id) =>
            id === NETWORK_CENTER_NODE_ID ? VERSE_TASSADAR_CORE_NODE_ID : id,
          ),
        },
  )

  return rewritten.some((node) => node.id === VERSE_TASSADAR_CORE_NODE_ID)
    ? rewritten
    : [
        {
          id: VERSE_TASSADAR_CORE_NODE_ID,
          label: "Tassadar",
          detail: coreDetail,
          role: "run",
          status: statusFromRun(summary, blockerCount),
          position: [0, 0, 0.95],
        },
        ...rewritten,
      ]
}

const stageRefs = (summary: TrainingRunSummaryRow) => {
  const metrics = summary.metrics
  const closeout = summary.realGradient.closeoutRequirement
  const loss = summary.realGradient.lossUnderBudget
  const externalAsk = summary.realGradient.externalAsk
  const windowRefs = summary.windows.flatMap((window) => [
    ...window.sourceRefs,
    ...window.receiptRefs,
  ])
  const replayRefs = uniqueRefs([
    ...closeout.freivaldsCommitmentRefs,
    ...closeout.gradientCloseoutRefs,
    closeout.evalRef,
    closeout.mergeRef,
    loss.budgetRef,
    ...loss.sourceRefs,
    ...summary.realGradient.deviceRequirement.sourceRefs,
  ])
  const settlementRefs = uniqueRefs([
    ...refsFromMetric(metrics.providerConfirmedSettledPayoutSats),
    ...summary.receiptRefs,
    ...summary.run.receiptRefs,
    ...summary.windows.flatMap(window => window.receiptRefs),
  ])

  return {
    assignment: uniqueRefs([
      ...refsFromMetric(metrics.assignedContributorCount),
      ...summary.run.sourceRefs,
      ...windowRefs,
    ]),
    trace: uniqueRefs([
      ...refsFromMetric(metrics.receiptRefCount),
      ...summary.sourceRefs,
      ...windowRefs,
    ]),
    replay: replayRefs,
    verdict: uniqueRefs([
      ...refsFromMetric(metrics.verifiedWorkCount),
      ...refsFromMetric(metrics.rejectedWorkCount),
      ...replayRefs,
      ...externalAsk.blockerRefs,
      ...externalAsk.requirementRefs,
    ]),
    settlement: settlementRefs,
    recipient: settlementRefs,
    blocked: uniqueRefs([...externalAsk.blockerRefs, ...externalAsk.requirementRefs]),
  } as const
}

const trainingStages = (
  summary: TrainingRunSummaryRow | null,
  promiseGates: TrainingPromiseGatesResponse | null,
): readonly TrainingRunNodeDefinition[] => {
  if (summary === null) {
    const blockerRefs = uniqueRefs(promiseGates?.blockerRefs ?? [])
    return [
      stageNode({
        id: "assignment",
        label: "assignment",
        detail: publicRefDetail("assignment", "waiting for public run", []),
        status: "queued",
        position: stagePositions.assignment,
        connectedTo: [VERSE_TASSADAR_CORE_NODE_ID],
      }),
      stageNode({
        id: "blocked",
        label: "blocked",
        detail: publicRefDetail("blockers", String(blockerRefs.length), blockerRefs),
        status: blockerRefs.length > 0 ? "blocked" : "planned",
        position: stagePositions.blocked,
        connectedTo: [VERSE_TASSADAR_CORE_NODE_ID],
      }),
    ]
  }

  const metrics = summary.metrics
  const refs = stageRefs(summary)
  const assigned = metricValue(metrics.assignedContributorCount)
  const submitted = metricValue(metrics.receiptRefCount)
  const verified = metricValue(metrics.verifiedWorkCount)
  const rejected = metricValue(metrics.rejectedWorkCount)
  const settledSats = metricValue(metrics.providerConfirmedSettledPayoutSats)
  const pendingPayouts = metricValue(metrics.pendingPayoutCount)
  const blockers = refs.blocked.length
  const replaySatisfied = summary.realGradient.closeoutRequirement.satisfied

  const nodes = [
    stageNode({
      id: "assignment",
      label: "assignment",
      detail: publicRefDetail("assigned", `${assigned} pylons`, refs.assignment),
      status: assigned > 0 ? "active" : "queued",
      position: stagePositions.assignment,
      connectedTo: [VERSE_TASSADAR_CORE_NODE_ID],
    }),
    stageNode({
      id: "trace",
      label: "trace",
      detail: publicRefDetail("workload", `${submitted} public receipts`, refs.trace),
      status: submitted > 0 ? "sync" : assigned > 0 ? "queued" : "planned",
      position: stagePositions.trace,
      connectedTo: [`${VERSE_TRAINING_NODE_PREFIX}assignment`],
    }),
    stageNode({
      id: "replay",
      label: "exact replay",
      detail: publicRefDetail(
        "replay",
        replaySatisfied ? "verified" : "pending",
        refs.replay,
      ),
      status: replaySatisfied ? "verified" : blockers > 0 ? "blocked" : "queued",
      position: stagePositions.replay,
      connectedTo: [`${VERSE_TRAINING_NODE_PREFIX}trace`],
    }),
    stageNode({
      id: "verdict",
      label: "verdict",
      detail: publicRefDetail(
        "accepted/rejected",
        `${verified} accepted · ${rejected} rejected`,
        refs.verdict,
      ),
      status: rejected > 0 ? "blocked" : verified > 0 ? "verified" : "queued",
      position: stagePositions.verdict,
      connectedTo: [`${VERSE_TRAINING_NODE_PREFIX}replay`],
    }),
    stageNode({
      id: "settlement",
      label: "settlement",
      detail: publicRefDetail(
        "settlement",
        settledSats > 0 ? `${settledSats} sats` : `${pendingPayouts} pending`,
        refs.settlement,
      ),
      status: settledSats > 0 ? "sealed" : pendingPayouts > 0 ? "active" : "planned",
      position: stagePositions.settlement,
      connectedTo: [`${VERSE_TRAINING_NODE_PREFIX}verdict`],
    }),
    stageNode({
      id: "recipient-confirmed",
      label: "recipient confirmed",
      detail: publicRefDetail("recipient", settledSats > 0 ? "confirmed" : "pending", refs.recipient),
      status: settledSats > 0 ? "verified" : "queued",
      position: stagePositions.recipient,
      connectedTo: [`${VERSE_TRAINING_NODE_PREFIX}settlement`],
    }),
  ]

  return blockers > 0
    ? [
        ...nodes,
        stageNode({
          id: "blocked",
          label: "blocked",
          detail: publicRefDetail("blockers", String(blockers), refs.blocked),
          status: "blocked",
          position: stagePositions.blocked,
          connectedTo: [VERSE_TASSADAR_CORE_NODE_ID],
        }),
      ]
    : nodes
}

const pushMotion = (
  beams: TrainingRunBeamDefinition[],
  bursts: TrainingRunBurstDefinition[],
  input: Readonly<{
    fromId: string
    toId: string
    kind: NonNullable<TrainingRunBeamDefinition["motionKind"]>
    refs: readonly string[]
    generatedAt?: string
    burst?: boolean
  }>,
): void => {
  const refs = uniqueRefs(input.refs)
  if (refs.length === 0) return
  const motion = {
    motionId: `${input.kind}:${input.toId}:${refs[0]}`,
    motionKind: input.kind,
    sourceRefs: refs,
    generatedAt: input.generatedAt,
    simulated: false,
  } as const
  beams.push({ fromId: input.fromId, toId: input.toId, ...motion })
  if (input.burst === true) bursts.push({ atId: input.toId, ...motion })
}

const trainingMotion = (
  summary: TrainingRunSummaryRow | null,
): Readonly<{
  beams: readonly TrainingRunBeamDefinition[]
  bursts: readonly TrainingRunBurstDefinition[]
}> => {
  if (summary === null) return { beams: [], bursts: [] }

  const refs = stageRefs(summary)
  const beams: TrainingRunBeamDefinition[] = []
  const bursts: TrainingRunBurstDefinition[] = []
  const generatedAt = summary.run.updatedAtDisplay
  const assigned = metricValue(summary.metrics.assignedContributorCount)
  const submitted = metricValue(summary.metrics.receiptRefCount)
  const verified = metricValue(summary.metrics.verifiedWorkCount)
  const rejected = metricValue(summary.metrics.rejectedWorkCount)
  const settledSats = metricValue(summary.metrics.providerConfirmedSettledPayoutSats)

  if (assigned > 0) {
    pushMotion(beams, bursts, {
      fromId: VERSE_TASSADAR_CORE_NODE_ID,
      toId: `${VERSE_TRAINING_NODE_PREFIX}assignment`,
      kind: "assignment",
      refs: refs.assignment,
      generatedAt,
    })
  }
  if (submitted > 0) {
    pushMotion(beams, bursts, {
      fromId: `${VERSE_TRAINING_NODE_PREFIX}assignment`,
      toId: `${VERSE_TRAINING_NODE_PREFIX}trace`,
      kind: "trace_submitted",
      refs: refs.trace,
      generatedAt,
    })
  }
  if (summary.realGradient.closeoutRequirement.satisfied) {
    pushMotion(beams, bursts, {
      fromId: `${VERSE_TRAINING_NODE_PREFIX}trace`,
      toId: `${VERSE_TRAINING_NODE_PREFIX}replay`,
      kind: "replay_verified",
      refs: refs.replay,
      generatedAt,
      burst: true,
    })
  }
  if (verified > 0 || rejected > 0) {
    pushMotion(beams, bursts, {
      fromId: `${VERSE_TRAINING_NODE_PREFIX}replay`,
      toId: `${VERSE_TRAINING_NODE_PREFIX}verdict`,
      kind: rejected > 0 ? "replay_rejected" : "corpus_accepted",
      refs: refs.verdict,
      generatedAt,
      burst: true,
    })
  }
  if (settledSats > 0) {
    pushMotion(beams, bursts, {
      fromId: `${VERSE_TRAINING_NODE_PREFIX}verdict`,
      toId: `${VERSE_TRAINING_NODE_PREFIX}settlement`,
      kind: "settlement_recorded",
      refs: refs.settlement,
      generatedAt,
      burst: true,
    })
    pushMotion(beams, bursts, {
      fromId: `${VERSE_TRAINING_NODE_PREFIX}settlement`,
      toId: `${VERSE_TRAINING_NODE_PREFIX}recipient-confirmed`,
      kind: "real_bitcoin_moved",
      refs: refs.recipient,
      generatedAt,
      burst: true,
    })
  }

  return { beams, bursts }
}

export const withVerseTrainingLayer = (
  base: TrainingRunVisualizationOptions,
  input: VerseTrainingInput,
): TrainingRunVisualizationOptions => {
  const trainingRuns = input.trainingRuns?.ok === false ? null : input.trainingRuns
  const summary = selectedVerseTrainingSummary(trainingRuns)
  const promiseGates = input.promiseGates ?? null
  const blockerCount =
    summary === null
      ? (promiseGates?.blockerRefs.length ?? 0)
      : stageRefs(summary).blocked.length
  const nodes = [
    ...rewriteBaseNodes(base.nodes ?? [], summary, blockerCount),
    ...trainingStages(summary, promiseGates),
  ]
  const motion = trainingMotion(summary)

  return {
    ...base,
    nodes,
    beams: [...(base.beams ?? []), ...motion.beams],
    bursts: [...(base.bursts ?? []), ...motion.bursts],
    motionPolicy: {
      ...(base.motionPolicy ?? {}),
      ambient: "static",
      bursts: "once",
      evidence: "required",
      structuralEdges: "static",
    },
    sceneChrome: {
      ...(base.sceneChrome ?? {}),
      contributorOrbit: "hidden",
      lossPanel: "hidden",
      staleRing: "hidden",
      statusChart: "hidden",
    },
    stageNodeGlyph: "compact_gate",
    worldLabelDensity: "pylons",
    keyboardTargeting: {
      enabled: true,
      maxTargets: 18,
    },
  }
}
