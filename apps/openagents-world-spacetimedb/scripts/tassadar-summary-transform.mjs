import { createHash } from 'node:crypto'

export const DEFAULT_TASSADAR_SOURCE_URL =
  'https://openagents.com/api/public/tassadar-run-summary'
export const DEFAULT_TASSADAR_RUN_REF = 'run.tassadar.executor.20260615'
export const DEFAULT_DATABASE = 'openagents-world'
export const DEFAULT_BRIDGE_REF = 'bridge.tassadar.public-summary'

const DEFAULT_WORLD_REGION = {
  avatarPositionMinIntervalMs: 100,
  bounds: {
    maxX: 160,
    maxY: 40,
    maxZ: 160,
    minX: -160,
    minY: 0,
    minZ: -160,
  },
  label: 'Tassadar main run space',
  localOrigin: { x: 0, y: 0, z: 0 },
  proximityRadiusMeters: 12,
  roadDirection: { x: 0, y: 0, z: 1 },
  staleAvatarPositionMs: 20_000,
  starterPylonSiteOffset: { x: 24, y: 0, z: 0 },
  streetNextRegionRef: 'region.run.tassadar.executor.20260615.street.next',
  streetPrevRegionRef: 'region.run.tassadar.executor.20260615.street.prev',
}

const text = value => (typeof value === 'string' ? value.trim() : '')

const numberOrZero = value =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0

const integerOrZero = value => {
  const numeric = numberOrZero(value)
  return Number.isInteger(numeric) && numeric > 0 ? numeric : 0
}

const bool = value => value === true

const array = value => (Array.isArray(value) ? value : [])

const unique = values =>
  [...new Set(array(values).map(text).filter(value => value.length > 0))].sort()

const metricValue = metric =>
  metric !== null && typeof metric === 'object'
    ? integerOrZero(metric.value)
    : 0

const sortedJsonValue = value => {
  if (Array.isArray(value)) {
    return value.map(sortedJsonValue)
  }
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, sortedJsonValue(nested)]),
    )
  }
  return value
}

export const stableJson = value => JSON.stringify(sortedJsonValue(value))

export const stableHash = value =>
  createHash('sha256').update(stableJson(value)).digest('hex')

const firstPublicRef = refs => unique(array(refs))[0] ?? ''

const sourceRefsFor = (...groups) => unique(groups.flatMap(group => array(group)))

const shortRef = ref => {
  const pieces = ref.split('.')
  const tail = pieces[pieces.length - 1] ?? ref
  return tail.length <= 10 ? tail : `${tail.slice(0, 4)}-${tail.slice(-4)}`
}

const trainingRunUrl = (runRef, focusRef) =>
  `https://openagents.com/api/public/training/runs/${encodeURIComponent(runRef)}${
    focusRef === '' ? '' : `?focusRef=${encodeURIComponent(focusRef)}`
  }`

const isReceiptRef = ref => ref.startsWith('receipt.')

const receiptUrl = ref =>
  ref.startsWith('receipt.nexus.') ||
  ref.startsWith('receipt.nexus_') ||
  ref.startsWith('receipt.nexus-pylon.')
    ? `https://openagents.com/api/public/nexus-pylon/receipts/${encodeURIComponent(ref)}`
    : `https://openagents.com/api/forum/receipts/${encodeURIComponent(ref)}`

const proofUrl = (runRef, ref) =>
  isReceiptRef(ref) ? receiptUrl(ref) : trainingRunUrl(runRef, ref)

const settlementStatus = row => {
  if (bool(row?.realBitcoinMoved)) {
    return 'real_settled'
  }
  if (text(row?.state) === 'settled' && text(row?.movementMode) === 'simulation') {
    return 'simulation_settled'
  }
  if (['failed', 'expired', 'rejected'].includes(text(row?.state))) {
    return 'failed_or_expired'
  }
  return 'pending_payout'
}

const settlementRowsForContributor = (settlementRows, contributorRef) =>
  settlementRows.filter(row => text(row.contributorRef) === contributorRef)

const contributorSettlementStatus = rows => {
  const statuses = rows.map(settlementStatus)
  if (statuses.includes('real_settled')) return 'real_settled'
  if (statuses.includes('simulation_settled')) return 'simulation_settled'
  if (statuses.includes('pending_payout')) return 'pending_payout'
  if (statuses.includes('failed_or_expired')) return 'failed_or_expired'
  return ''
}

const leaderboardStatus = (row, settlementRows) => {
  const contributorRef = text(row?.pylonRef)
  const settlement =
    contributorRef === ''
      ? ''
      : contributorSettlementStatus(
          settlementRowsForContributor(settlementRows, contributorRef),
        )
  if (settlement !== '') return settlement
  if (integerOrZero(row?.verifiedWindowCount) > 0) return 'verified'
  return unique(row?.sourceRefs).length > 0 ? 'assigned' : 'registered'
}

const addCall = (calls, reducer, args) => {
  calls.push({ reducer, args })
}

const addEntity = (calls, input) => {
  addCall(calls, 'upsert_run_entity', [
    input.entityRef,
    input.runRef,
    input.entityKind,
    input.label,
    input.lane,
    input.status,
    input.sourceRef,
    input.proofCount,
  ])
}

const addProof = (calls, input) => {
  if (input.proofRef === '') return
  addCall(calls, 'upsert_proof_ref', [
    input.proofRef,
    input.runRef,
    input.entityRef,
    input.proofKind,
    proofUrl(input.runRef, input.proofRef),
    input.title,
  ])
}

const addEdge = (calls, input) => {
  if (input.fromEntityRef === '' || input.toEntityRef === '') return
  if (input.sourceRef === '') return
  addCall(calls, 'upsert_world_edge', [
    `edge.${stableHash([
      input.runRef,
      input.fromEntityRef,
      input.toEntityRef,
      input.edgeKind,
      input.sourceRef,
    ]).slice(0, 24)}`,
    input.runRef,
    input.fromEntityRef,
    input.toEntityRef,
    input.edgeKind,
    input.sourceRef,
  ])
}

const addWorldEvent = (calls, input) => {
  if (input.sourceRef === '' && input.sourceGeneratedAt === '') return
  addCall(calls, 'append_world_event', [
    `world_event.${stableHash([
      input.runRef,
      input.eventKind,
      input.entityRef,
      input.sourceRef,
    ]).slice(0, 24)}`,
    input.runRef,
    input.eventKind,
    input.entityRef,
    input.sourceRef,
    input.sourceGeneratedAt,
    input.summary,
  ])
}

const addWorldRegion = (calls, input) => {
  addCall(calls, 'upsert_world_region', [
    input.regionRef,
    input.runRef,
    input.label,
    input.bounds.minX,
    input.bounds.minY,
    input.bounds.minZ,
    input.bounds.maxX,
    input.bounds.maxY,
    input.bounds.maxZ,
    input.roadDirection.x,
    input.roadDirection.y,
    input.roadDirection.z,
    input.localOrigin.x,
    input.localOrigin.y,
    input.localOrigin.z,
    input.starterPylonSiteOffset.x,
    input.starterPylonSiteOffset.y,
    input.starterPylonSiteOffset.z,
    input.streetPrevRegionRef,
    input.streetNextRegionRef,
    input.proximityRadiusMeters,
    input.avatarPositionMinIntervalMs,
    input.staleAvatarPositionMs,
  ])
}

const addProofsForEntity = (calls, runRef, entityRef, refs, proofKind) => {
  unique(refs).forEach(ref => {
    addProof(calls, {
      entityRef,
      proofKind,
      proofRef: ref,
      runRef,
      title: `${proofKind}: ${shortRef(ref)}`,
    })
  })
}

const coordinate = value => Math.round(value * 1_000) / 1_000

const spread = (index, total, start, end) =>
  coordinate(
    total <= 1
      ? (start + end) / 2
      : start + ((end - start) * index) / (total - 1),
  )

const pylonStationPosition = (index, total) => ({
  x: -2.35,
  y: 0,
  z: spread(index, total, 1.5, -1.5),
})

const addPylonStation = (calls, input) => {
  addCall(calls, 'upsert_pylon_station_from_projection', [
    input.pylonRef,
    input.runRef,
    input.regionRef,
    input.label,
    input.sourceUrl,
    input.position.x,
    input.position.y,
    input.position.z,
    input.headingYaw,
    input.interactionRadiusMeters,
  ])
}

const addPylonAgentAvatar = (calls, input) => {
  addCall(calls, 'ensure_pylon_agent_avatar', [
    input.avatarRef,
    input.pylonRef,
    input.displayName,
    input.regionRef,
    input.position.x,
    input.position.y,
    input.position.z,
    input.yaw,
  ])
}

export const buildTassadarProjectionPlan = (
  summary,
  options = {},
) => {
  const sourceUrl = options.sourceUrl ?? DEFAULT_TASSADAR_SOURCE_URL
  const runRef = text(summary?.runRef) || DEFAULT_TASSADAR_RUN_REF
  const runState = text(summary?.runState) || 'planned'
  const generatedAt = text(summary?.generatedAt)
  const staleness = summary?.staleness ?? {}
  const stalenessKind = text(staleness.composition) || 'unknown'
  const maxStalenessSeconds = integerOrZero(staleness.maxStalenessSeconds)
  const sourceHash = stableHash(summary)
  const calls = []
  const regionRef = `region.${runRef}.main`

  addCall(calls, 'upsert_training_run', [
    runRef,
    runState,
    sourceUrl,
    generatedAt,
    stalenessKind,
    maxStalenessSeconds,
    sourceHash,
  ])
  addWorldRegion(calls, {
    ...DEFAULT_WORLD_REGION,
    regionRef,
    runRef,
  })

  addEntity(calls, {
    entityKind: 'run',
    entityRef: runRef,
    label: 'Tassadar executor run',
    lane: 'run',
    proofCount: unique(summary?.receiptRefs).length,
    runRef,
    sourceRef: runRef,
    status: runState,
  })
  addWorldEvent(calls, {
    entityRef: runRef,
    eventKind: `run_state_${runState}`,
    runRef,
    sourceGeneratedAt: generatedAt,
    sourceRef: runRef,
    summary: `Run state ${runState}`,
  })

  const settlementRows = array(summary?.settlementRows)
  const leaderboardRows = array(summary?.realGradient?.leaderboardRows)
  const visiblePylonRows = leaderboardRows.filter(
    row => text(row?.pylonRef) !== '',
  )
  visiblePylonRows.forEach((row, index) => {
    const entityRef = text(row.pylonRef)
    if (entityRef === '') return
    const rank = integerOrZero(row.rank)
    const label = rank > 0 ? `P${rank}` : shortRef(entityRef)
    const refs = sourceRefsFor(row.sourceRefs, [entityRef])
    const sourceRef = firstPublicRef(refs)
    const stationPosition = pylonStationPosition(index, visiblePylonRows.length)
    const agentPosition = {
      x: coordinate(stationPosition.x + 0.45),
      y: stationPosition.y,
      z: stationPosition.z,
    }
    const sourceUrlForStation = trainingRunUrl(runRef, sourceRef || entityRef)
    addEntity(calls, {
      entityKind: 'pylon',
      entityRef,
      label,
      lane: 'pylon',
      proofCount: refs.length,
      runRef,
      sourceRef,
      status: leaderboardStatus(row, settlementRows),
    })
    addProofsForEntity(calls, runRef, entityRef, refs, 'pylon_source_ref')
    addEdge(calls, {
      edgeKind: 'run_to_pylon',
      fromEntityRef: runRef,
      runRef,
      sourceRef,
      toEntityRef: entityRef,
    })
    addPylonStation(calls, {
      headingYaw: 0,
      interactionRadiusMeters: 2.4,
      label,
      position: stationPosition,
      pylonRef: entityRef,
      regionRef,
      runRef,
      sourceUrl: sourceUrlForStation,
    })
    addPylonAgentAvatar(calls, {
      avatarRef: `avatar.pylon_agent.${entityRef}`,
      displayName: `${label} agent`,
      position: agentPosition,
      pylonRef: entityRef,
      regionRef,
      yaw: 0,
    })
    addWorldEvent(calls, {
      entityRef,
      eventKind: 'pylon_projected',
      runRef,
      sourceGeneratedAt: generatedAt,
      sourceRef,
      summary: `Projected pylon ${index + 1}`,
    })
  })

  array(summary?.realGradient?.verifiedReplayPairs).forEach((pair, index) => {
    const workerRef = text(pair.workerRef)
    const validatorRef = text(pair.validatorRef)
    const refs = sourceRefsFor(
      pair.sourceRefs,
      pair.verdictRefs,
      [pair.challengeRef, workerRef, validatorRef],
    )
    const sourceRef = text(pair.challengeRef) || firstPublicRef(refs)
    if (workerRef !== '') {
      addEntity(calls, {
        entityKind: 'verified_replay_worker',
        entityRef: workerRef,
        label: `W${index + 1}`,
        lane: 'verified_replay',
        proofCount: refs.length,
        runRef,
        sourceRef,
        status: 'verified',
      })
      addProofsForEntity(calls, runRef, workerRef, refs, 'verified_replay_ref')
    }
    if (validatorRef !== '') {
      addEntity(calls, {
        entityKind: 'verified_replay_validator',
        entityRef: validatorRef,
        label: `V${index + 1}`,
        lane: 'verified_replay',
        proofCount: refs.length,
        runRef,
        sourceRef,
        status: 'verified',
      })
      addProofsForEntity(calls, runRef, validatorRef, refs, 'verified_replay_ref')
    }
    addEdge(calls, {
      edgeKind: 'verified_replay_pair',
      fromEntityRef: workerRef,
      runRef,
      sourceRef,
      toEntityRef: validatorRef,
    })
    addWorldEvent(calls, {
      entityRef: workerRef || validatorRef,
      eventKind: 'verified_replay_pair',
      runRef,
      sourceGeneratedAt: generatedAt,
      sourceRef,
      summary: `Verified replay pair ${index + 1}`,
    })
  })

  array(summary?.realGradient?.rejectedReplayPairs).forEach((pair, index) => {
    const workerRef = text(pair.workerRef)
    const validatorRef = text(pair.validatorRef)
    const refs = sourceRefsFor(
      pair.sourceRefs,
      pair.verdictRefs,
      [pair.challengeRef, workerRef, validatorRef],
    )
    const sourceRef = text(pair.challengeRef) || firstPublicRef(refs)
    if (workerRef !== '') {
      addEntity(calls, {
        entityKind: 'rejected_replay_worker',
        entityRef: workerRef,
        label: `RW${index + 1}`,
        lane: 'rejected_replay',
        proofCount: refs.length,
        runRef,
        sourceRef,
        status: 'rejected',
      })
      addProofsForEntity(calls, runRef, workerRef, refs, 'rejected_replay_ref')
    }
    if (validatorRef !== '') {
      addEntity(calls, {
        entityKind: 'rejected_replay_validator',
        entityRef: validatorRef,
        label: `RV${index + 1}`,
        lane: 'rejected_replay',
        proofCount: refs.length,
        runRef,
        sourceRef,
        status: 'rejected',
      })
      addProofsForEntity(calls, runRef, validatorRef, refs, 'rejected_replay_ref')
    }
    addEdge(calls, {
      edgeKind: 'rejected_replay_pair',
      fromEntityRef: workerRef,
      runRef,
      sourceRef,
      toEntityRef: validatorRef,
    })
    addWorldEvent(calls, {
      entityRef: workerRef || validatorRef,
      eventKind: 'rejected_replay_pair',
      runRef,
      sourceGeneratedAt: generatedAt,
      sourceRef,
      summary: `Rejected replay pair ${index + 1}`,
    })
  })

  settlementRows.forEach((row, index) => {
    const receiptRef = text(row.receiptRef)
    if (receiptRef === '') return
    const contributorRef = text(row.contributorRef)
    const refs = sourceRefsFor(row.sourceRefs, [
      receiptRef,
      contributorRef,
      row.trainingRunRef,
      row.verificationChallengeRef,
    ])
    addEntity(calls, {
      entityKind: 'settlement_receipt',
      entityRef: receiptRef,
      label: `${integerOrZero(row.amountSats)}s`,
      lane: 'settlement',
      proofCount: refs.length,
      runRef,
      sourceRef: receiptRef,
      status: settlementStatus(row),
    })
    addCall(calls, 'upsert_settlement_ref', [
      receiptRef,
      runRef,
      receiptRef,
      receiptRef,
      text(row.movementMode) || 'unknown',
      bool(row.realBitcoinMoved),
      integerOrZero(row.amountSats),
      text(row.apiUrl) || receiptUrl(receiptRef),
    ])
    addProofsForEntity(calls, runRef, receiptRef, refs, 'settlement_ref')
    addEdge(calls, {
      edgeKind: 'pylon_to_settlement',
      fromEntityRef: contributorRef,
      runRef,
      sourceRef: receiptRef,
      toEntityRef: receiptRef,
    })
    addWorldEvent(calls, {
      entityRef: receiptRef,
      eventKind: 'settlement_projected',
      runRef,
      sourceGeneratedAt: generatedAt,
      sourceRef: receiptRef,
      summary: `Settlement receipt ${index + 1}`,
    })
  })

  unique(summary?.corpus?.traceRefs).forEach((traceRef, index) => {
    addEntity(calls, {
      entityKind: 'accepted_trace',
      entityRef: traceRef,
      label: `T${index + 1}`,
      lane: 'trace',
      proofCount: 1,
      runRef,
      sourceRef: traceRef,
      status: 'accepted_trace',
    })
    addProof(calls, {
      entityRef: traceRef,
      proofKind: 'accepted_trace_ref',
      proofRef: traceRef,
      runRef,
      title: `accepted trace ${index + 1}`,
    })
    addEdge(calls, {
      edgeKind: 'run_to_trace',
      fromEntityRef: runRef,
      runRef,
      sourceRef: traceRef,
      toEntityRef: traceRef,
    })
    addWorldEvent(calls, {
      entityRef: traceRef,
      eventKind: 'accepted_trace_projected',
      runRef,
      sourceGeneratedAt: generatedAt,
      sourceRef: traceRef,
      summary: `Accepted trace ${index + 1}`,
    })
  })

  const cursorRowCount = calls.length + 1
  addCall(calls, 'record_projection_cursor', [
    'tassadar-public-summary',
    sourceUrl,
    generatedAt,
    sourceHash,
    cursorRowCount,
  ])

  return {
    bridgeRef: DEFAULT_BRIDGE_REF,
    database: options.database ?? DEFAULT_DATABASE,
    runRef,
    sourceGeneratedAt: generatedAt,
    sourceHash,
    sourceUrl,
    calls,
  }
}

export const reducerCounts = plan =>
  plan.calls.reduce((counts, call) => {
    counts[call.reducer] = (counts[call.reducer] ?? 0) + 1
    return counts
  }, {})

export const assertNoDuplicateWorldEvents = plan => {
  const eventRefs = plan.calls
    .filter(call => call.reducer === 'append_world_event')
    .map(call => call.args[0])
  const duplicates = eventRefs.filter((ref, index) => eventRefs.indexOf(ref) !== index)
  if (duplicates.length > 0) {
    throw new Error(`duplicate world_event refs: ${unique(duplicates).join(', ')}`)
  }
}

export const assertWorldEventsAreSourced = plan => {
  const unsourced = plan.calls
    .filter(call => call.reducer === 'append_world_event')
    .filter(call => text(call.args[4]) === '' && text(call.args[5]) === '')
    .map(call => call.args[0])
  if (unsourced.length > 0) {
    throw new Error(`unsourced world_event refs: ${unsourced.join(', ')}`)
  }
}
