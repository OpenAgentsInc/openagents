import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonLightningGraphImplementationStatus = S.Literals([
  'contract_only',
  'projection_available',
  'route_live',
])
export type PylonLightningGraphImplementationStatus =
  typeof PylonLightningGraphImplementationStatus.Type

export const PylonLightningGraphNodeKind = S.Literals([
  'channel',
  'failed_route',
  'liquidity_movement',
  'payout_event',
  'peer',
  'provider',
  'rail',
  'settlement_receipt',
])
export type PylonLightningGraphNodeKind =
  typeof PylonLightningGraphNodeKind.Type

export const PylonLightningGraphEdgeKind = S.Literals([
  'channel_liquidity',
  'failed_route_on_rail',
  'peer_channel',
  'provider_peer',
  'provider_rail',
  'settlement_evidence',
  'work_payout',
])
export type PylonLightningGraphEdgeKind =
  typeof PylonLightningGraphEdgeKind.Type

export const PylonLightningGraphStatus = S.Literals([
  'active',
  'attention_required',
  'blocked',
  'failed',
  'pending',
  'settled',
  'stale',
  'unknown',
])
export type PylonLightningGraphStatus =
  typeof PylonLightningGraphStatus.Type

export const PylonLightningGraphFreshness = S.Literals([
  'expired',
  'fresh',
  'stale',
  'unknown',
])
export type PylonLightningGraphFreshness =
  typeof PylonLightningGraphFreshness.Type

export const PylonLightningGraphVisibility = S.Literals([
  'private',
  'public',
])
export type PylonLightningGraphVisibility =
  typeof PylonLightningGraphVisibility.Type

export const PylonLightningGraphAuthorityBoundary = S.Literals([
  'read_only_graph_projection',
])
export type PylonLightningGraphAuthorityBoundary =
  typeof PylonLightningGraphAuthorityBoundary.Type

export class PylonLightningGraphAuthority extends S.Class<PylonLightningGraphAuthority>(
  'PylonLightningGraphAuthority',
)({
  authorityBoundary: PylonLightningGraphAuthorityBoundary,
  noChannelMutation: S.Boolean,
  noGraphMutation: S.Boolean,
  noLiquidityMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPeerMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
  noWalletMutation: S.Boolean,
}) {}

export class PylonLightningGraphNode extends S.Class<PylonLightningGraphNode>(
  'PylonLightningGraphNode',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  freshness: PylonLightningGraphFreshness,
  id: S.String,
  kind: PylonLightningGraphNodeKind,
  label: S.String,
  linkRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: PylonLightningGraphStatus,
  updatedAtIso: S.String,
  visibility: PylonLightningGraphVisibility,
}) {}

export class PylonLightningGraphEdge extends S.Class<PylonLightningGraphEdge>(
  'PylonLightningGraphEdge',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  freshness: PylonLightningGraphFreshness,
  fromNodeId: S.String,
  id: S.String,
  kind: PylonLightningGraphEdgeKind,
  label: S.String,
  linkRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: PylonLightningGraphStatus,
  toNodeId: S.String,
  updatedAtIso: S.String,
  visibility: PylonLightningGraphVisibility,
}) {}

export class PylonLightningGraphProjectedNode extends S.Class<PylonLightningGraphProjectedNode>(
  'PylonLightningGraphProjectedNode',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  freshness: PylonLightningGraphFreshness,
  id: S.String,
  kind: PylonLightningGraphNodeKind,
  label: S.String,
  linkRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: PylonLightningGraphStatus,
  updatedAtDisplay: S.String,
  visibility: PylonLightningGraphVisibility,
}) {}

export class PylonLightningGraphProjectedEdge extends S.Class<PylonLightningGraphProjectedEdge>(
  'PylonLightningGraphProjectedEdge',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  freshness: PylonLightningGraphFreshness,
  fromNodeId: S.String,
  id: S.String,
  kind: PylonLightningGraphEdgeKind,
  label: S.String,
  linkRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: PylonLightningGraphStatus,
  toNodeId: S.String,
  updatedAtDisplay: S.String,
  visibility: PylonLightningGraphVisibility,
}) {}

export class PylonLightningGraphFilters extends S.Class<PylonLightningGraphFilters>(
  'PylonLightningGraphFilters',
)({
  edgeKinds: S.Array(PylonLightningGraphEdgeKind),
  freshness: S.Array(PylonLightningGraphFreshness),
  nodeKinds: S.Array(PylonLightningGraphNodeKind),
  providerRefs: S.Array(S.String),
  railRefs: S.Array(S.String),
  statuses: S.Array(PylonLightningGraphStatus),
}) {}

export class PylonLightningGraphPage extends S.Class<PylonLightningGraphPage>(
  'PylonLightningGraphPage',
)({
  limit: S.Number,
  nextCursorRef: S.NullOr(S.String),
  requestedCursorRef: S.NullOr(S.String),
}) {}

export class PylonLightningGraphRecord extends S.Class<PylonLightningGraphRecord>(
  'PylonLightningGraphRecord',
)({
  authority: PylonLightningGraphAuthority,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  edges: S.Array(PylonLightningGraphEdge),
  filters: PylonLightningGraphFilters,
  id: S.String,
  implementationStatus: PylonLightningGraphImplementationStatus,
  nodes: S.Array(PylonLightningGraphNode),
  page: PylonLightningGraphPage,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonLightningGraphProjection extends S.Class<PylonLightningGraphProjection>(
  'PylonLightningGraphProjection',
)({
  audience: OmniProjectionAudience,
  authority: PylonLightningGraphAuthority,
  caveatRefs: S.Array(S.String),
  channelMutationAllowed: S.Boolean,
  createdAtDisplay: S.String,
  edgeCount: S.Number,
  edges: S.Array(PylonLightningGraphProjectedEdge),
  filters: PylonLightningGraphFilters,
  graphMutationAllowed: S.Boolean,
  id: S.String,
  implementationStatus: PylonLightningGraphImplementationStatus,
  liquidityMutationAllowed: S.Boolean,
  liveWalletSpendAllowed: S.Boolean,
  nodeCount: S.Number,
  nodes: S.Array(PylonLightningGraphProjectedNode),
  page: PylonLightningGraphPage,
  payoutDispatchMutationAllowed: S.Boolean,
  peerMutationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  walletMutationAllowed: S.Boolean,
}) {}

export class PylonLightningGraphUnsafe extends S.TaggedErrorClass<PylonLightningGraphUnsafe>()(
  'PylonLightningGraphUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_LIGHTNING_GRAPH_READ_ONLY_AUTHORITY:
  PylonLightningGraphAuthority = {
    authorityBoundary: 'read_only_graph_projection',
    noChannelMutation: true,
    noGraphMutation: true,
    noLiquidityMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPeerMutation: true,
    noSettlementMutation: true,
    noWalletMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeLightningGraphRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth[_-]?content[_-]?json|auth\.json|bearer|bolt11|bolt12|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|node[_-]?pubkey|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw|target)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|peer[_-]?(key|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|graph|invoice|payment|payload|payout|peer|prompt|provider|runner|run[_-]?log|state|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(blocker\.private|caveat\.private|channel\.private|edge\.private|evidence\.private|failed_route\.private|liquidity\.private|link\.private|node\.private|payout\.private|peer\.private|provider\.private|rail\.private|receipt\.private|settlement\.private|source\.private|wallet\.private)/i
const customerUnsafeRefPattern = publicUnsafeRefPattern
const teamUnsafeRefPattern =
  /(channel\.private|edge\.private|failed_route\.private|liquidity\.private|node\.private|payout\.private|peer\.private|provider\.private|rail\.private|receipt\.private|settlement\.private|wallet\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeLightningGraphRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonLightningGraphUnsafe({
      reason: `${label} contains private channel state, peer secrets, wallet material, raw bitcoin payment material, invoices, preimages, credentials, provider secrets, customer data, or raw timestamps.`,
    })
  }
}

const assertSafeLabel = (label: string, value: string): void => {
  const trimmed = value.trim()

  if (
    trimmed.length === 0 ||
    trimmed.length > 120 ||
    unsafeLightningGraphRefPattern.test(trimmed) ||
    rawTimestampPattern.test(trimmed)
  ) {
    throw new PylonLightningGraphUnsafe({
      reason: `${label} contains private channel state, peer secrets, wallet material, raw bitcoin payment material, credentials, customer data, raw timestamps, or an invalid label.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const visibleForAudience = (
  visibility: PylonLightningGraphVisibility,
  audience: typeof OmniProjectionAudience.Type,
): boolean =>
  visibility === 'public' || audience === 'operator' || audience === 'private'

const pageForAudience = (
  page: PylonLightningGraphPage,
  audience: typeof OmniProjectionAudience.Type,
): PylonLightningGraphPage => ({
  limit: page.limit,
  nextCursorRef: page.nextCursorRef === null
    ? null
    : safeRefsForAudience(
      'Lightning graph next cursor ref',
      [page.nextCursorRef],
      audience,
    )[0] ?? null,
  requestedCursorRef: page.requestedCursorRef === null
    ? null
    : safeRefsForAudience(
      'Lightning graph requested cursor ref',
      [page.requestedCursorRef],
      audience,
    )[0] ?? null,
})

const filtersForAudience = (
  filters: PylonLightningGraphFilters,
  audience: typeof OmniProjectionAudience.Type,
): PylonLightningGraphFilters => ({
  edgeKinds: [...new Set(filters.edgeKinds)].sort(),
  freshness: [...new Set(filters.freshness)].sort(),
  nodeKinds: [...new Set(filters.nodeKinds)].sort(),
  providerRefs: safeRefsForAudience(
    'Lightning graph filter provider refs',
    filters.providerRefs,
    audience,
  ),
  railRefs: safeRefsForAudience(
    'Lightning graph filter rail refs',
    filters.railRefs,
    audience,
  ),
  statuses: [...new Set(filters.statuses)].sort(),
})

export const pylonLightningGraphHasNoMutationAuthority = (
  authority: PylonLightningGraphAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_graph_projection' &&
  authority.noChannelMutation &&
  authority.noGraphMutation &&
  authority.noLiquidityMutation &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPeerMutation &&
  authority.noSettlementMutation &&
  authority.noWalletMutation

export const pylonLightningGraphCanMutate = (
  record: PylonLightningGraphRecord,
): boolean => !pylonLightningGraphHasNoMutationAuthority(record.authority)

const assertIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new PylonLightningGraphUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertGraphNodeSafe = (node: PylonLightningGraphNode): void => {
  assertSafeRefs('Lightning graph node id', [node.id])
  assertSafeLabel('Lightning graph node label', node.label)
  assertSafeRefs(`${node.kind} node blocker refs`, node.blockerRefs)
  assertSafeRefs(`${node.kind} node caveat refs`, node.caveatRefs)
  assertSafeRefs(`${node.kind} node evidence refs`, node.evidenceRefs)
  assertSafeRefs(`${node.kind} node link refs`, node.linkRefs)
  assertSafeRefs(`${node.kind} node source refs`, node.sourceRefs)
  assertIso(`${node.kind} node updatedAtIso`, node.updatedAtIso)

  if (
    (node.status === 'blocked' || node.status === 'failed') &&
    node.blockerRefs.length === 0
  ) {
    throw new PylonLightningGraphUnsafe({
      reason: `${node.kind} node ${node.status} status requires blocker refs.`,
    })
  }

  if (
    (node.status === 'attention_required' ||
      node.status === 'stale' ||
      node.freshness === 'stale' ||
      node.freshness === 'expired') &&
    node.caveatRefs.length === 0
  ) {
    throw new PylonLightningGraphUnsafe({
      reason: `${node.kind} node non-fresh or attention state requires caveat refs.`,
    })
  }
}

const assertGraphEdgeSafe = (
  edge: PylonLightningGraphEdge,
  nodeIds: ReadonlySet<string>,
): void => {
  assertSafeRefs('Lightning graph edge ids', [
    edge.id,
    edge.fromNodeId,
    edge.toNodeId,
  ])
  assertSafeLabel('Lightning graph edge label', edge.label)
  assertSafeRefs(`${edge.kind} edge blocker refs`, edge.blockerRefs)
  assertSafeRefs(`${edge.kind} edge caveat refs`, edge.caveatRefs)
  assertSafeRefs(`${edge.kind} edge evidence refs`, edge.evidenceRefs)
  assertSafeRefs(`${edge.kind} edge link refs`, edge.linkRefs)
  assertSafeRefs(`${edge.kind} edge source refs`, edge.sourceRefs)
  assertIso(`${edge.kind} edge updatedAtIso`, edge.updatedAtIso)

  if (!nodeIds.has(edge.fromNodeId) || !nodeIds.has(edge.toNodeId)) {
    throw new PylonLightningGraphUnsafe({
      reason: `${edge.kind} edge must reference known graph nodes.`,
    })
  }

  if (
    (edge.status === 'blocked' || edge.status === 'failed') &&
    edge.blockerRefs.length === 0
  ) {
    throw new PylonLightningGraphUnsafe({
      reason: `${edge.kind} edge ${edge.status} status requires blocker refs.`,
    })
  }

  if (
    (edge.status === 'attention_required' ||
      edge.status === 'stale' ||
      edge.freshness === 'stale' ||
      edge.freshness === 'expired') &&
    edge.caveatRefs.length === 0
  ) {
    throw new PylonLightningGraphUnsafe({
      reason: `${edge.kind} edge non-fresh or attention state requires caveat refs.`,
    })
  }
}

const assertUniqueIds = (
  label: string,
  ids: ReadonlyArray<string>,
): void => {
  if (new Set(ids).size !== ids.length) {
    throw new PylonLightningGraphUnsafe({
      reason: `${label} must be unique.`,
    })
  }
}

const assertPageSafe = (page: PylonLightningGraphPage): void => {
  if (!Number.isInteger(page.limit) || page.limit < 1 || page.limit > 100) {
    throw new PylonLightningGraphUnsafe({
      reason: 'Lightning graph page limit must be an integer between 1 and 100.',
    })
  }

  assertSafeRefs(
    'Lightning graph cursor refs',
    [page.nextCursorRef, page.requestedCursorRef].filter(
      (ref): ref is string => ref !== null,
    ),
  )
}

const assertRecordSafe = (record: PylonLightningGraphRecord): void => {
  assertSafeRefs('Lightning graph record id', [record.id])
  assertSafeRefs('Lightning graph caveat refs', record.caveatRefs)
  assertSafeRefs('Lightning graph source refs', record.sourceRefs)
  assertSafeRefs(
    'Lightning graph filter provider refs',
    record.filters.providerRefs,
  )
  assertSafeRefs('Lightning graph filter rail refs', record.filters.railRefs)
  assertIso('Lightning graph createdAtIso', record.createdAtIso)
  assertIso('Lightning graph updatedAtIso', record.updatedAtIso)
  assertPageSafe(record.page)

  if (!pylonLightningGraphHasNoMutationAuthority(record.authority)) {
    throw new PylonLightningGraphUnsafe({
      reason: 'Lightning/Pylon graph records are read-only and cannot carry graph, channel, peer, liquidity, wallet spend, payout dispatch, or settlement mutation authority.',
    })
  }

  assertUniqueIds(
    'Lightning graph node ids',
    record.nodes.map(node => node.id),
  )
  assertUniqueIds(
    'Lightning graph edge ids',
    record.edges.map(edge => edge.id),
  )

  for (const node of record.nodes) {
    assertGraphNodeSafe(node)
  }

  const nodeIds = new Set(record.nodes.map(node => node.id))

  for (const edge of record.edges) {
    assertGraphEdgeSafe(edge, nodeIds)
  }
}

const visibleNode = (
  node: PylonLightningGraphNode,
  audience: typeof OmniProjectionAudience.Type,
): PylonLightningGraphProjectedNode | null => {
  if (!visibleForAudience(node.visibility, audience)) {
    return null
  }

  return {
    blockerRefs: safeRefsForAudience(
      `${node.kind} node blocker refs`,
      node.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      `${node.kind} node caveat refs`,
      node.caveatRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      `${node.kind} node evidence refs`,
      node.evidenceRefs,
      audience,
    ),
    freshness: node.freshness,
    id: node.id,
    kind: node.kind,
    label: node.label,
    linkRefs: safeRefsForAudience(
      `${node.kind} node link refs`,
      node.linkRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience(
      `${node.kind} node source refs`,
      node.sourceRefs,
      audience,
    ),
    status: node.status,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      node.updatedAtIso,
      node.updatedAtIso,
    ),
    visibility: node.visibility,
  }
}

const visibleEdge = (
  edge: PylonLightningGraphEdge,
  visibleNodeIds: ReadonlySet<string>,
  audience: typeof OmniProjectionAudience.Type,
): PylonLightningGraphProjectedEdge | null => {
  if (
    !visibleForAudience(edge.visibility, audience) ||
    !visibleNodeIds.has(edge.fromNodeId) ||
    !visibleNodeIds.has(edge.toNodeId)
  ) {
    return null
  }

  return {
    blockerRefs: safeRefsForAudience(
      `${edge.kind} edge blocker refs`,
      edge.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      `${edge.kind} edge caveat refs`,
      edge.caveatRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      `${edge.kind} edge evidence refs`,
      edge.evidenceRefs,
      audience,
    ),
    freshness: edge.freshness,
    fromNodeId: edge.fromNodeId,
    id: edge.id,
    kind: edge.kind,
    label: edge.label,
    linkRefs: safeRefsForAudience(
      `${edge.kind} edge link refs`,
      edge.linkRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience(
      `${edge.kind} edge source refs`,
      edge.sourceRefs,
      audience,
    ),
    status: edge.status,
    toNodeId: edge.toNodeId,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      edge.updatedAtIso,
      edge.updatedAtIso,
    ),
    visibility: edge.visibility,
  }
}

const projectionText = (
  projection: PylonLightningGraphProjection,
): string =>
  [
    projection.id,
    projection.page.nextCursorRef ?? '',
    projection.page.requestedCursorRef ?? '',
    ...projection.caveatRefs,
    ...projection.sourceRefs,
    ...projection.filters.providerRefs,
    ...projection.filters.railRefs,
    ...projection.nodes.flatMap(node => [
      node.id,
      node.label,
      node.updatedAtDisplay,
      ...node.blockerRefs,
      ...node.caveatRefs,
      ...node.evidenceRefs,
      ...node.linkRefs,
      ...node.sourceRefs,
    ]),
    ...projection.edges.flatMap(edge => [
      edge.id,
      edge.fromNodeId,
      edge.toNodeId,
      edge.label,
      edge.updatedAtDisplay,
      ...edge.blockerRefs,
      ...edge.caveatRefs,
      ...edge.evidenceRefs,
      ...edge.linkRefs,
      ...edge.sourceRefs,
    ]),
  ].join(' ')

export const pylonLightningGraphProjectionHasPrivateMaterial = (
  projection: PylonLightningGraphProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeLightningGraphRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonLightningGraph = (
  record: PylonLightningGraphRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonLightningGraphProjection => {
  assertRecordSafe(record)

  const nodes = record.nodes
    .map(node => visibleNode(node, audience))
    .filter((node): node is PylonLightningGraphProjectedNode => node !== null)
  const visibleNodeIds = new Set(nodes.map(node => node.id))
  const edges = record.edges
    .map(edge => visibleEdge(edge, visibleNodeIds, audience))
    .filter((edge): edge is PylonLightningGraphProjectedEdge => edge !== null)

  const projection: PylonLightningGraphProjection = {
    audience,
    authority: record.authority,
    caveatRefs: safeRefsForAudience(
      'Lightning graph caveat refs',
      record.caveatRefs,
      audience,
    ),
    channelMutationAllowed: false,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    edgeCount: edges.length,
    edges,
    filters: filtersForAudience(record.filters, audience),
    graphMutationAllowed: false,
    id: safeRefsForAudience('Lightning graph id', [record.id], audience)[0] ??
      'pylon_lightning_graph.redacted',
    implementationStatus: record.implementationStatus,
    liquidityMutationAllowed: false,
    liveWalletSpendAllowed: false,
    nodeCount: nodes.length,
    nodes,
    page: pageForAudience(record.page, audience),
    payoutDispatchMutationAllowed: false,
    peerMutationAllowed: false,
    settlementMutationAllowed: false,
    sourceRefs: safeRefsForAudience(
      'Lightning graph source refs',
      record.sourceRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletMutationAllowed: false,
  }

  if (pylonLightningGraphProjectionHasPrivateMaterial(projection)) {
    throw new PylonLightningGraphUnsafe({
      reason: 'Lightning/Pylon graph projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_LIGHTNING_GRAPH_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonLightningGraphRecord> = [
    {
      authority: PYLON_LIGHTNING_GRAPH_READ_ONLY_AUTHORITY,
      caveatRefs: ['caveat.public.contract_only_no_route_live'],
      createdAtIso: '2026-06-07T10:00:00.000Z',
      edges: [
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.provider_rail_projection'],
          freshness: 'fresh',
          fromNodeId: 'node.provider.public.pylon_1',
          id: 'edge.public.provider_rail.pylon_1',
          kind: 'provider_rail',
          label: 'Provider rail',
          linkRefs: ['link.public.pylon_provider_1'],
          sourceRefs: ['source.public.pylon_readiness'],
          status: 'active',
          toNodeId: 'node.rail.public.ldk',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.private.peer_graph'],
          freshness: 'fresh',
          fromNodeId: 'node.provider.public.pylon_1',
          id: 'edge.private.provider_peer.pylon_1',
          kind: 'provider_peer',
          label: 'Provider peer',
          linkRefs: ['link.private.operator_graph'],
          sourceRefs: ['source.private.ldk_graph_snapshot'],
          status: 'active',
          toNodeId: 'node.peer.private.peer_a',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'private',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.private.channel_graph'],
          freshness: 'fresh',
          fromNodeId: 'node.peer.private.peer_a',
          id: 'edge.private.peer_channel.pylon_1',
          kind: 'peer_channel',
          label: 'Peer channel',
          linkRefs: ['link.private.operator_graph'],
          sourceRefs: ['source.private.ldk_graph_snapshot'],
          status: 'active',
          toNodeId: 'node.channel.private.chan_a',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'private',
        },
        {
          blockerRefs: [],
          caveatRefs: ['caveat.public.summary_amount_only'],
          evidenceRefs: ['evidence.public.liquidity_summary'],
          freshness: 'fresh',
          fromNodeId: 'node.channel.private.chan_a',
          id: 'edge.private.channel_liquidity.pylon_1',
          kind: 'channel_liquidity',
          label: 'Channel liquidity summary',
          linkRefs: ['link.private.operator_graph'],
          sourceRefs: ['source.private.ldk_graph_snapshot'],
          status: 'active',
          toNodeId: 'node.liquidity.public.inbound_summary',
          updatedAtIso: '2026-06-07T10:24:00.000Z',
          visibility: 'private',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.accepted_work_reward'],
          freshness: 'fresh',
          fromNodeId: 'node.payout.public.site_otc_revision_3',
          id: 'edge.public.work_payout.site_otc_revision_3',
          kind: 'work_payout',
          label: 'Accepted work payout',
          linkRefs: ['link.public.site_order.otc'],
          sourceRefs: ['source.public.payout_row_projection'],
          status: 'settled',
          toNodeId: 'node.receipt.public.site_otc_revision_3',
          updatedAtIso: '2026-06-07T10:20:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: ['blocker.public.no_route_summary'],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.failed_route_summary'],
          freshness: 'fresh',
          fromNodeId: 'node.failed_route.public.no_route_summary',
          id: 'edge.public.failed_route.ldk',
          kind: 'failed_route_on_rail',
          label: 'Failed route summary',
          linkRefs: ['link.public.pylon_readiness'],
          sourceRefs: ['source.public.ldk_readiness_projection'],
          status: 'failed',
          toNodeId: 'node.rail.public.ldk',
          updatedAtIso: '2026-06-07T10:18:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.settlement_receipt_summary'],
          freshness: 'fresh',
          fromNodeId: 'node.receipt.public.site_otc_revision_3',
          id: 'edge.public.settlement_evidence.site_otc_revision_3',
          kind: 'settlement_evidence',
          label: 'Settlement evidence',
          linkRefs: ['link.public.proof.otc'],
          sourceRefs: ['source.public.settlement_receipt_projection'],
          status: 'settled',
          toNodeId: 'node.rail.public.ldk',
          updatedAtIso: '2026-06-07T10:21:00.000Z',
          visibility: 'public',
        },
      ],
      filters: {
        edgeKinds: ['provider_rail', 'work_payout'],
        freshness: ['fresh'],
        nodeKinds: ['provider', 'rail', 'payout_event'],
        providerRefs: ['provider.public.pylon_1'],
        railRefs: ['rail.public.ldk'],
        statuses: ['active', 'settled'],
      },
      id: 'pylon_lightning_graph.contract.v1',
      implementationStatus: 'contract_only',
      nodes: [
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.pylon_provider_1'],
          freshness: 'fresh',
          id: 'node.provider.public.pylon_1',
          kind: 'provider',
          label: 'Pylon provider',
          linkRefs: ['link.public.pylon_provider_1'],
          sourceRefs: ['source.public.pylon_readiness'],
          status: 'active',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.ldk_rail'],
          freshness: 'fresh',
          id: 'node.rail.public.ldk',
          kind: 'rail',
          label: 'LDK rail',
          linkRefs: ['link.public.pylon_readiness'],
          sourceRefs: ['source.public.ldk_readiness_projection'],
          status: 'active',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.private.peer_graph'],
          freshness: 'fresh',
          id: 'node.peer.private.peer_a',
          kind: 'peer',
          label: 'Peer summary',
          linkRefs: ['link.private.operator_graph'],
          sourceRefs: ['source.private.ldk_graph_snapshot'],
          status: 'active',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'private',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.private.channel_graph'],
          freshness: 'fresh',
          id: 'node.channel.private.chan_a',
          kind: 'channel',
          label: 'Channel summary',
          linkRefs: ['link.private.operator_graph'],
          sourceRefs: ['source.private.ldk_graph_snapshot'],
          status: 'active',
          updatedAtIso: '2026-06-07T10:25:00.000Z',
          visibility: 'private',
        },
        {
          blockerRefs: [],
          caveatRefs: ['caveat.public.summary_amount_only'],
          evidenceRefs: ['evidence.public.liquidity_summary'],
          freshness: 'fresh',
          id: 'node.liquidity.public.inbound_summary',
          kind: 'liquidity_movement',
          label: 'Inbound liquidity summary',
          linkRefs: ['link.public.pylon_readiness'],
          sourceRefs: ['source.public.wallet_liquidity_projection'],
          status: 'active',
          updatedAtIso: '2026-06-07T10:24:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.accepted_work_reward'],
          freshness: 'fresh',
          id: 'node.payout.public.site_otc_revision_3',
          kind: 'payout_event',
          label: 'Accepted work payout',
          linkRefs: ['link.public.site_order.otc'],
          sourceRefs: ['source.public.payout_row_projection'],
          status: 'settled',
          updatedAtIso: '2026-06-07T10:20:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: ['blocker.public.no_route_summary'],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.failed_route_summary'],
          freshness: 'fresh',
          id: 'node.failed_route.public.no_route_summary',
          kind: 'failed_route',
          label: 'Failed route summary',
          linkRefs: ['link.public.pylon_readiness'],
          sourceRefs: ['source.public.ldk_readiness_projection'],
          status: 'failed',
          updatedAtIso: '2026-06-07T10:18:00.000Z',
          visibility: 'public',
        },
        {
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: ['evidence.public.settlement_receipt_summary'],
          freshness: 'fresh',
          id: 'node.receipt.public.site_otc_revision_3',
          kind: 'settlement_receipt',
          label: 'Settlement receipt',
          linkRefs: ['link.public.proof.otc'],
          sourceRefs: ['source.public.settlement_receipt_projection'],
          status: 'settled',
          updatedAtIso: '2026-06-07T10:21:00.000Z',
          visibility: 'public',
        },
      ],
      page: {
        limit: 50,
        nextCursorRef: 'cursor.public.graph_page_2',
        requestedCursorRef: null,
      },
      sourceRefs: [
        'source.public.pylon_readiness',
        'source.public.wallet_liquidity_projection',
        'source.public.payout_row_projection',
      ],
      updatedAtIso: '2026-06-07T10:25:00.000Z',
    },
  ]
