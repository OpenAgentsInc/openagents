/**
 * Pure Khala Sync wiring for the AIUR-1 proof-of-connection view: the
 * public `scope.public.tokens-served` counter (packages/khala-sync/src/
 * public-counter.ts). Kept dependency-free from React/DOM so the
 * bootstrap-decode and delta-apply logic is unit-testable without a real
 * transport or socket.
 */
import {
  type BootstrapEntity,
  BootstrapRequest,
  ClientGroupId,
  decodePublicCounterEntity,
  type DeltaFrame,
  KHALA_SYNC_PROTOCOL_VERSION,
  PUBLIC_COUNTER_ENTITY_TYPE,
  publicScope,
  SyncSchemaVersion,
  TOKENS_SERVED_COUNTER_ID,
} from '@openagentsinc/khala-sync'

export const TOKENS_SERVED_SCOPE = publicScope(TOKENS_SERVED_COUNTER_ID)

export type TokensServedSnapshot = Readonly<{
  total: number
  lastEventAt: string | null
}>

export const buildTokensServedBootstrapRequest = (
  clientGroupId: string,
): BootstrapRequest =>
  new BootstrapRequest({
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    schemaVersion: SyncSchemaVersion.make(1),
    scope: TOKENS_SERVED_SCOPE,
    clientGroupId: ClientGroupId.make(clientGroupId),
  })

export const extractTokensServedSnapshot = (
  entities: ReadonlyArray<BootstrapEntity>,
): TokensServedSnapshot | undefined => {
  for (const entity of entities) {
    if (
      entity.entityType !== PUBLIC_COUNTER_ENTITY_TYPE ||
      entity.entityId !== TOKENS_SERVED_COUNTER_ID
    ) {
      continue
    }

    try {
      const decoded = decodePublicCounterEntity(JSON.parse(entity.postImageJson))
      return { total: decoded.total, lastEventAt: decoded.lastEventAt }
    } catch {
      return undefined
    }
  }

  return undefined
}

/** Applies a live `DeltaFrame` to the current snapshot; returns the same
 * reference when the frame carries no matching entry (no re-render). */
export const applyTokensServedDelta = (
  current: TokensServedSnapshot | undefined,
  frame: DeltaFrame,
): TokensServedSnapshot | undefined => {
  let next = current

  for (const entry of frame.entries) {
    if (
      entry.entityType !== PUBLIC_COUNTER_ENTITY_TYPE ||
      entry.entityId !== TOKENS_SERVED_COUNTER_ID
    ) {
      continue
    }

    if (entry.op === 'delete' || entry.postImageJson === undefined) {
      continue
    }

    try {
      const decoded = decodePublicCounterEntity(JSON.parse(entry.postImageJson))
      next = { total: decoded.total, lastEventAt: decoded.lastEventAt }
    } catch {
      // Undecodable post-image: keep the last-known-good snapshot.
    }
  }

  return next
}
