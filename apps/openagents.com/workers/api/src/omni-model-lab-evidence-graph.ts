import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniModelLabGraphAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniModelLabGraphAudience = typeof OmniModelLabGraphAudience.Type

export const OmniModelLabGraphNodeKind = S.Literals([
  'adapter_validation',
  'candidate',
  'eval_rerun',
  'model_artifact',
  'promotion_gate',
  'retained_failure',
  'training_run',
])
export type OmniModelLabGraphNodeKind =
  typeof OmniModelLabGraphNodeKind.Type

export const OmniModelLabGraphNodeState = S.Literals([
  'active',
  'blocked',
  'reviewed',
  'stale',
  'superseded',
])
export type OmniModelLabGraphNodeState =
  typeof OmniModelLabGraphNodeState.Type

export const OmniModelLabGraphEdgeKind = S.Literals([
  'derived_from',
  'evaluated_by',
  'gated_by',
  'produced',
  'supersedes',
  'validated_by',
])
export type OmniModelLabGraphEdgeKind =
  typeof OmniModelLabGraphEdgeKind.Type

export const OmniModelLabGraphRollbackPosture = S.Literals([
  'missing',
  'candidate',
  'ready',
  'verified',
])
export type OmniModelLabGraphRollbackPosture =
  typeof OmniModelLabGraphRollbackPosture.Type

export const OmniModelLabGraphAuthorityBoundary = S.Literals([
  'read_only_model_lab_evidence_graph',
])
export type OmniModelLabGraphAuthorityBoundary =
  typeof OmniModelLabGraphAuthorityBoundary.Type

export class OmniModelLabGraphAuthority extends S.Class<OmniModelLabGraphAuthority>(
  'OmniModelLabGraphAuthority',
)({
  authorityBoundary: OmniModelLabGraphAuthorityBoundary,
  noAdapterInstall: S.Boolean,
  noEvalExecution: S.Boolean,
  noModelTrainingLaunch: S.Boolean,
  noPaymentSpend: S.Boolean,
  noPayoutMutation: S.Boolean,
  noProviderCall: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRoutingMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniModelLabGraphNodeRecord extends S.Class<OmniModelLabGraphNodeRecord>(
  'OmniModelLabGraphNodeRecord',
)({
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  kind: OmniModelLabGraphNodeKind,
  loopRefs: S.Array(S.String),
  nodeRef: S.String,
  staleEvidenceRefs: S.Array(S.String),
  state: OmniModelLabGraphNodeState,
}) {}

export class OmniModelLabGraphEdgeRecord extends S.Class<OmniModelLabGraphEdgeRecord>(
  'OmniModelLabGraphEdgeRecord',
)({
  caveatRefs: S.Array(S.String),
  edgeRef: S.String,
  evidenceRefs: S.Array(S.String),
  fromNodeRef: S.String,
  kind: OmniModelLabGraphEdgeKind,
  toNodeRef: S.String,
}) {}

export class OmniModelLabGraphRollbackRecord extends S.Class<OmniModelLabGraphRollbackRecord>(
  'OmniModelLabGraphRollbackRecord',
)({
  priorNodeRefs: S.Array(S.String),
  rollbackPosture: OmniModelLabGraphRollbackPosture,
  rollbackRefs: S.Array(S.String),
}) {}

export class OmniModelLabEvidenceGraphRecord extends S.Class<OmniModelLabEvidenceGraphRecord>(
  'OmniModelLabEvidenceGraphRecord',
)({
  authority: OmniModelLabGraphAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  edges: S.Array(OmniModelLabGraphEdgeRecord),
  graphRef: S.String,
  id: S.String,
  loopRef: S.String,
  nodes: S.Array(OmniModelLabGraphNodeRecord),
  rollback: OmniModelLabGraphRollbackRecord,
  staleEvidenceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OmniModelLabGraphKindCount extends S.Class<OmniModelLabGraphKindCount>(
  'OmniModelLabGraphKindCount',
)({
  count: S.Number,
  kind: OmniModelLabGraphNodeKind,
}) {}

export class OmniModelLabEvidenceGraphProjection extends S.Class<OmniModelLabEvidenceGraphProjection>(
  'OmniModelLabEvidenceGraphProjection',
)({
  adapterInstallAllowed: S.Boolean,
  audience: OmniModelLabGraphAudience,
  authority: OmniModelLabGraphAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  connected: S.Boolean,
  createdAtDisplay: S.String,
  edgeCount: S.Number,
  edges: S.Array(OmniModelLabGraphEdgeRecord),
  evalExecutionAllowed: S.Boolean,
  graphRef: S.String,
  id: S.String,
  loopRef: S.String,
  modelTrainingLaunchAllowed: S.Boolean,
  nodeCount: S.Number,
  nodeKindCounts: S.Array(OmniModelLabGraphKindCount),
  nodes: S.Array(OmniModelLabGraphNodeRecord),
  paymentSpendAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  providerCallAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  rollback: OmniModelLabGraphRollbackRecord,
  rollbackReady: S.Boolean,
  routingMutationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  staleEvidenceCount: S.Number,
  staleEvidenceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniModelLabEvidenceGraphUnsafe extends S.TaggedErrorClass<OmniModelLabEvidenceGraphUnsafe>()(
  'OmniModelLabEvidenceGraphUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_MODEL_LAB_GRAPH_READ_ONLY_AUTHORITY:
  OmniModelLabGraphAuthority = {
    authorityBoundary: 'read_only_model_lab_evidence_graph',
    noAdapterInstall: true,
    noEvalExecution: true,
    noModelTrainingLaunch: true,
    noPaymentSpend: true,
    noPayoutMutation: true,
    noProviderCall: true,
    noPublicClaimUpgrade: true,
    noRoutingMutation: true,
    noRuntimePromotion: true,
    noSettlementMutation: true,
  }

const allNodeKinds: ReadonlyArray<OmniModelLabGraphNodeKind> = [
  'adapter_validation',
  'candidate',
  'eval_rerun',
  'model_artifact',
  'promotion_gate',
  'retained_failure',
  'training_run',
]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeGraphRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.(raw|private)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(adapter_validation\.private|artifact\.private|candidate\.private|caveat\.private|edge\.private|eval\.private|evidence\.private|failure\.private|gate\.private|graph\.private|loop\.private|node\.private|promotion_gate\.private|retained_failure\.private|rollback\.private|source\.|stale\.private|training_run\.private)/i
const agentUnsafeRefPattern =
  /(adapter_validation\.private|artifact\.private|candidate\.private|edge\.private|eval\.private|evidence\.private|failure\.private|gate\.private|graph\.private|loop\.private|node\.private|promotion_gate\.private|retained_failure\.private|rollback\.private|source\.private|stale\.private|training_run\.private)/i
const customerUnsafeRefPattern = agentUnsafeRefPattern

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeGraphRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: `${label} contains private prompts, source archives, datasets, provider payloads, model weights, secrets, payment/wallet material, private repos, raw logs, raw traces, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniModelLabGraphAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'agent') {
    return agentUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniModelLabGraphAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const refForAudience = (
  label: string,
  ref: string,
  audience: OmniModelLabGraphAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniModelLabGraphAuthority,
): void => {
  if (
    authority.noAdapterInstall !== true ||
    authority.noEvalExecution !== true ||
    authority.noModelTrainingLaunch !== true ||
    authority.noPaymentSpend !== true ||
    authority.noPayoutMutation !== true ||
    authority.noProviderCall !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRoutingMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason:
        'Model Lab evidence graphs are read-only and cannot run evals, launch training, call providers, install adapters, spend money, promote runtime behavior, mutate routes, pay out, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const duplicateRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  uniqueRefs(refs.filter((ref, index) => refs.indexOf(ref) !== index))

const refsByKind = (
  nodes: ReadonlyArray<OmniModelLabGraphNodeRecord>,
  kind: OmniModelLabGraphNodeKind,
): ReadonlyArray<string> =>
  nodes.filter(node => node.kind === kind).map(node => node.nodeRef)

const nodeRefSet = (
  nodes: ReadonlyArray<OmniModelLabGraphNodeRecord>,
): ReadonlySet<string> => new Set(nodes.map(node => node.nodeRef))

const hasCycleFrom = (
  edges: ReadonlyArray<OmniModelLabGraphEdgeRecord>,
  nodeRef: string,
  path: ReadonlyArray<string>,
): boolean => {
  if (path.includes(nodeRef)) {
    return true
  }

  return edges
    .filter(edge => edge.fromNodeRef === nodeRef)
    .some(edge => hasCycleFrom(edges, edge.toNodeRef, [...path, nodeRef]))
}

const graphHasCycle = (
  nodes: ReadonlyArray<OmniModelLabGraphNodeRecord>,
  edges: ReadonlyArray<OmniModelLabGraphEdgeRecord>,
): boolean =>
  nodes.some(node =>
    edges
      .filter(edge => edge.fromNodeRef === node.nodeRef)
      .some(edge => hasCycleFrom(edges, edge.toNodeRef, [node.nodeRef])),
  )

const neighborsFor = (
  edges: ReadonlyArray<OmniModelLabGraphEdgeRecord>,
  nodeRef: string,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...edges
      .filter(edge => edge.fromNodeRef === nodeRef)
      .map(edge => edge.toNodeRef),
    ...edges
      .filter(edge => edge.toNodeRef === nodeRef)
      .map(edge => edge.fromNodeRef),
  ])

const collectReachable = (
  edges: ReadonlyArray<OmniModelLabGraphEdgeRecord>,
  frontier: ReadonlyArray<string>,
  visited: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const next = uniqueRefs(
    frontier
      .flatMap(nodeRef => neighborsFor(edges, nodeRef))
      .filter(nodeRef => !visited.includes(nodeRef)),
  )

  return hasAny(next)
    ? collectReachable(edges, next, uniqueRefs([...visited, ...next]))
    : uniqueRefs(visited)
}

const graphIsConnected = (
  nodes: ReadonlyArray<OmniModelLabGraphNodeRecord>,
  edges: ReadonlyArray<OmniModelLabGraphEdgeRecord>,
): boolean => {
  const first = nodes[0]

  if (first === undefined) {
    return false
  }

  return collectReachable(edges, [first.nodeRef], [first.nodeRef]).length ===
    nodes.length
}

const assertNode = (
  node: OmniModelLabGraphNodeRecord,
  loopRef: string,
): void => {
  assertSafeRefs('Model Lab graph node ref', [node.nodeRef])
  assertSafeRefs('Model Lab graph node caveat refs', node.caveatRefs)
  assertSafeRefs('Model Lab graph node evidence refs', node.evidenceRefs)
  assertSafeRefs('Model Lab graph node loop refs', node.loopRefs)
  assertSafeRefs('Model Lab graph node stale evidence refs', node.staleEvidenceRefs)

  if (!node.loopRefs.includes(loopRef)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Every Model Lab evidence graph node must link to the graph loop ref.',
    })
  }

  if (!hasAny(node.evidenceRefs)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Every Model Lab evidence graph node requires evidence refs.',
    })
  }

  if (node.state === 'stale' && !hasAny(node.staleEvidenceRefs)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Stale Model Lab graph nodes require stale evidence refs.',
    })
  }

  if (node.state === 'blocked' && !hasAny(node.caveatRefs)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Blocked Model Lab graph nodes require caveat refs.',
    })
  }
}

const assertEdge = (
  edge: OmniModelLabGraphEdgeRecord,
  nodeRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Model Lab graph edge ref', [edge.edgeRef])
  assertSafeRefs('Model Lab graph edge caveat refs', edge.caveatRefs)
  assertSafeRefs('Model Lab graph edge evidence refs', edge.evidenceRefs)
  assertSafeRefs('Model Lab graph edge source/target refs', [
    edge.fromNodeRef,
    edge.toNodeRef,
  ])

  if (!nodeRefs.has(edge.fromNodeRef) || !nodeRefs.has(edge.toNodeRef)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graph edges must reference nodes in the same graph.',
    })
  }

  if (edge.fromNodeRef === edge.toNodeRef) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graph edges cannot self-loop.',
    })
  }

  if (!hasAny(edge.evidenceRefs)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graph edges require evidence refs.',
    })
  }
}

const assertRollback = (
  rollback: OmniModelLabGraphRollbackRecord,
  edges: ReadonlyArray<OmniModelLabGraphEdgeRecord>,
  nodeRefs: ReadonlySet<string>,
): void => {
  assertSafeRefs('Model Lab graph rollback refs', rollback.rollbackRefs)
  assertSafeRefs('Model Lab graph prior node refs', rollback.priorNodeRefs)

  rollback.priorNodeRefs.forEach(priorNodeRef => {
    if (!nodeRefs.has(priorNodeRef)) {
      throw new OmniModelLabEvidenceGraphUnsafe({
        reason: 'Model Lab graph rollback prior refs must be graph nodes.',
      })
    }
  })

  if (edges.some(edge => edge.kind === 'gated_by')) {
    if (
      rollback.rollbackPosture !== 'ready' &&
      rollback.rollbackPosture !== 'verified'
    ) {
      throw new OmniModelLabEvidenceGraphUnsafe({
        reason: 'Promotion-gated Model Lab graphs require ready or verified rollback posture.',
      })
    }

    if (!hasAny(rollback.rollbackRefs) || !hasAny(rollback.priorNodeRefs)) {
      throw new OmniModelLabEvidenceGraphUnsafe({
        reason: 'Promotion-gated Model Lab graphs require rollback and prior node refs.',
      })
    }
  }
}

const assertRecord = (record: OmniModelLabEvidenceGraphRecord): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('createdAtIso', record.createdAtIso)
  assertValidIso('updatedAtIso', record.updatedAtIso)

  assertSafeRefs('Model Lab graph id', [record.id])
  assertSafeRefs('Model Lab graph ref', [record.graphRef])
  assertSafeRefs('Model Lab graph loop ref', [record.loopRef])
  assertSafeRefs('Model Lab graph blocker refs', record.blockerRefs)
  assertSafeRefs('Model Lab graph caveat refs', record.caveatRefs)
  assertSafeRefs('Model Lab graph stale evidence refs', record.staleEvidenceRefs)

  if (!hasAny(record.nodes) || !hasAny(record.edges)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graphs require nodes and edges.',
    })
  }

  const nodeRefs = record.nodes.map(node => node.nodeRef)
  const edgeRefs = record.edges.map(edge => edge.edgeRef)

  if (hasAny(duplicateRefs(nodeRefs)) || hasAny(duplicateRefs(edgeRefs))) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graphs cannot contain duplicate node or edge refs.',
    })
  }

  record.nodes.forEach(node => assertNode(node, record.loopRef))
  record.edges.forEach(edge => assertEdge(edge, nodeRefSet(record.nodes)))
  assertRollback(record.rollback, record.edges, nodeRefSet(record.nodes))

  if (graphHasCycle(record.nodes, record.edges)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graphs cannot contain directed cycles.',
    })
  }

  if (!graphIsConnected(record.nodes, record.edges)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Model Lab evidence graph nodes must form one connected graph.',
    })
  }

  if (allNodeKinds.some(kind => !hasAny(refsByKind(record.nodes, kind)))) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason:
        'Model Lab evidence graphs require retained failure, candidate, training run, model artifact, eval rerun, adapter validation, and promotion gate nodes.',
    })
  }

  if (hasAny(record.staleEvidenceRefs) && !hasAny(record.caveatRefs)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason: 'Graph-level stale evidence requires caveat refs.',
    })
  }
}

const redactNode = (
  node: OmniModelLabGraphNodeRecord,
  audience: OmniModelLabGraphAudience,
): OmniModelLabGraphNodeRecord => ({
  ...node,
  caveatRefs: refsForAudience(
    'Model Lab graph node caveat refs',
    node.caveatRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Model Lab graph node evidence refs',
    node.evidenceRefs,
    audience,
  ),
  loopRefs: refsForAudience(
    'Model Lab graph node loop refs',
    node.loopRefs,
    audience,
  ),
  nodeRef: refForAudience(
    'Model Lab graph node ref',
    node.nodeRef,
    audience,
    'node.redacted.model_lab_graph',
  ),
  staleEvidenceRefs: refsForAudience(
    'Model Lab graph node stale evidence refs',
    node.staleEvidenceRefs,
    audience,
  ),
})

const redactEdge = (
  edge: OmniModelLabGraphEdgeRecord,
  audience: OmniModelLabGraphAudience,
): OmniModelLabGraphEdgeRecord => ({
  ...edge,
  caveatRefs: refsForAudience(
    'Model Lab graph edge caveat refs',
    edge.caveatRefs,
    audience,
  ),
  edgeRef: refForAudience(
    'Model Lab graph edge ref',
    edge.edgeRef,
    audience,
    'edge.redacted.model_lab_graph',
  ),
  evidenceRefs: refsForAudience(
    'Model Lab graph edge evidence refs',
    edge.evidenceRefs,
    audience,
  ),
  fromNodeRef: refForAudience(
    'Model Lab graph edge source ref',
    edge.fromNodeRef,
    audience,
    'node.redacted.model_lab_graph',
  ),
  toNodeRef: refForAudience(
    'Model Lab graph edge target ref',
    edge.toNodeRef,
    audience,
    'node.redacted.model_lab_graph',
  ),
})

const rollbackForAudience = (
  rollback: OmniModelLabGraphRollbackRecord,
  audience: OmniModelLabGraphAudience,
): OmniModelLabGraphRollbackRecord => ({
  ...rollback,
  priorNodeRefs: refsForAudience(
    'Model Lab graph prior node refs',
    rollback.priorNodeRefs,
    audience,
  ),
  rollbackRefs: refsForAudience(
    'Model Lab graph rollback refs',
    rollback.rollbackRefs,
    audience,
  ),
})

const nodeKindCounts = (
  nodes: ReadonlyArray<OmniModelLabGraphNodeRecord>,
): ReadonlyArray<OmniModelLabGraphKindCount> =>
  allNodeKinds
    .map(kind => ({ count: refsByKind(nodes, kind).length, kind }))
    .filter(summary => summary.count > 0)

const stringValuesFromUnknown = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValuesFromUnknown)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(stringValuesFromUnknown)
  }

  return []
}

export const omniModelLabEvidenceGraphProjectionHasPrivateMaterial = (
  projection: OmniModelLabEvidenceGraphProjection,
): boolean => {
  const values = stringValuesFromUnknown(projection).join('\n')

  return unsafeGraphRefPattern.test(values) || rawTimestampPattern.test(values)
}

export const projectOmniModelLabEvidenceGraph = (
  record: OmniModelLabEvidenceGraphRecord,
  audience: OmniModelLabGraphAudience,
  nowIso: string,
): OmniModelLabEvidenceGraphProjection => {
  assertRecord(record)
  assertValidIso('nowIso', nowIso)

  const connected = graphIsConnected(record.nodes, record.edges)
  const projection: OmniModelLabEvidenceGraphProjection = {
    adapterInstallAllowed: false,
    audience,
    authority: record.authority,
    blockerRefs: refsForAudience(
      'Model Lab graph blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Model Lab graph caveat refs',
      record.caveatRefs,
      audience,
    ),
    connected,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    edgeCount: record.edges.length,
    edges: record.edges.map(edge => redactEdge(edge, audience)),
    evalExecutionAllowed: false,
    graphRef: refForAudience(
      'Model Lab graph ref',
      record.graphRef,
      audience,
      'graph.redacted.model_lab',
    ),
    id: refForAudience('Model Lab graph id', record.id, audience, 'graph.redacted'),
    loopRef: refForAudience(
      'Model Lab graph loop ref',
      record.loopRef,
      audience,
      'loop.redacted.model_lab',
    ),
    modelTrainingLaunchAllowed: false,
    nodeCount: record.nodes.length,
    nodeKindCounts: nodeKindCounts(record.nodes),
    nodes: record.nodes.map(node => redactNode(node, audience)),
    paymentSpendAllowed: false,
    payoutMutationAllowed: false,
    providerCallAllowed: false,
    publicClaimUpgradeAllowed: false,
    rollback: rollbackForAudience(record.rollback, audience),
    rollbackReady:
      record.rollback.rollbackPosture === 'ready' ||
      record.rollback.rollbackPosture === 'verified',
    routingMutationAllowed: false,
    runtimePromotionAllowed: false,
    settlementMutationAllowed: false,
    staleEvidenceCount:
      record.staleEvidenceRefs.length +
      record.nodes.flatMap(node => node.staleEvidenceRefs).length,
    staleEvidenceRefs: refsForAudience(
      'Model Lab graph stale evidence refs',
      record.staleEvidenceRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (omniModelLabEvidenceGraphProjectionHasPrivateMaterial(projection)) {
    throw new OmniModelLabEvidenceGraphUnsafe({
      reason:
        'Model Lab evidence graph projection contains private prompt, source, dataset, provider, model, payment, wallet, raw log, raw trace, or raw timestamp material.',
    })
  }

  return projection
}

export const exampleOmniModelLabEvidenceGraph =
  (): OmniModelLabEvidenceGraphRecord => ({
    authority: OMNI_MODEL_LAB_GRAPH_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.public.model_lab_graph_evidence_only'],
    createdAtIso: '2026-06-06T23:10:00.000Z',
    edges: [
      {
        caveatRefs: [],
        edgeRef: 'edge.public.failure_to_candidate',
        evidenceRefs: ['evidence.public.failure_candidate_trace'],
        fromNodeRef: 'retained_failure.public.otect_revision_images',
        kind: 'derived_from',
        toNodeRef: 'candidate.public.otect_adapter_candidate',
      },
      {
        caveatRefs: [],
        edgeRef: 'edge.public.candidate_to_training',
        evidenceRefs: ['evidence.public.training_manifest'],
        fromNodeRef: 'candidate.public.otect_adapter_candidate',
        kind: 'produced',
        toNodeRef: 'training_run.public.otect_adapter_tune',
      },
      {
        caveatRefs: [],
        edgeRef: 'edge.public.training_to_artifact',
        evidenceRefs: ['evidence.public.artifact_digest_manifest'],
        fromNodeRef: 'training_run.public.otect_adapter_tune',
        kind: 'produced',
        toNodeRef: 'artifact.public.otect_layout_adapter_v1',
      },
      {
        caveatRefs: [],
        edgeRef: 'edge.public.artifact_to_eval',
        evidenceRefs: ['evidence.public.eval_manifest'],
        fromNodeRef: 'artifact.public.otect_layout_adapter_v1',
        kind: 'evaluated_by',
        toNodeRef: 'eval.public.otect_revision_regression_pass',
      },
      {
        caveatRefs: [],
        edgeRef: 'edge.public.eval_to_adapter_validation',
        evidenceRefs: ['evidence.public.adapter_validation_manifest'],
        fromNodeRef: 'eval.public.otect_revision_regression_pass',
        kind: 'validated_by',
        toNodeRef: 'adapter_validation.public.otect_safety_adapter',
      },
      {
        caveatRefs: [],
        edgeRef: 'edge.public.validation_to_gate',
        evidenceRefs: ['evidence.public.promotion_gate_review'],
        fromNodeRef: 'adapter_validation.public.otect_safety_adapter',
        kind: 'gated_by',
        toNodeRef: 'promotion_gate.public.otect_adapter_review',
      },
    ],
    graphRef: 'graph.public.otect_model_lab_evidence',
    id: 'model_lab_graph.public.otect_revision_two',
    loopRef: 'loop.public.otect_retained_failure_loop',
    nodes: [
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.retained_failure_summary'],
        kind: 'retained_failure',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'retained_failure.public.otect_revision_images',
        staleEvidenceRefs: [],
        state: 'reviewed',
      },
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.candidate_manifest'],
        kind: 'candidate',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'candidate.public.otect_adapter_candidate',
        staleEvidenceRefs: [],
        state: 'active',
      },
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.training_run_manifest'],
        kind: 'training_run',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'training_run.public.otect_adapter_tune',
        staleEvidenceRefs: [],
        state: 'reviewed',
      },
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.artifact_digest_manifest'],
        kind: 'model_artifact',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'artifact.public.otect_layout_adapter_v1',
        staleEvidenceRefs: [],
        state: 'reviewed',
      },
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.eval_manifest'],
        kind: 'eval_rerun',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'eval.public.otect_revision_regression_pass',
        staleEvidenceRefs: [],
        state: 'reviewed',
      },
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.adapter_validation_manifest'],
        kind: 'adapter_validation',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'adapter_validation.public.otect_safety_adapter',
        staleEvidenceRefs: [],
        state: 'reviewed',
      },
      {
        caveatRefs: [],
        evidenceRefs: ['evidence.public.promotion_gate_review'],
        kind: 'promotion_gate',
        loopRefs: ['loop.public.otect_retained_failure_loop'],
        nodeRef: 'promotion_gate.public.otect_adapter_review',
        staleEvidenceRefs: [],
        state: 'reviewed',
      },
    ],
    rollback: {
      priorNodeRefs: ['artifact.public.otect_layout_adapter_v1'],
      rollbackPosture: 'ready',
      rollbackRefs: ['rollback.public.otect_adapter_restore'],
    },
    staleEvidenceRefs: [],
    updatedAtIso: '2026-06-06T23:26:00.000Z',
  })
