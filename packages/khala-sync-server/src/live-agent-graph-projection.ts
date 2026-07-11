import {
  LIVE_AGENT_GRAPH_ENTITY_TYPE,
  canonicalJson,
  type ChangelogEntry,
  EntityId,
  EntityType,
  liveAgentGraphScope,
  projectLiveAgentGraphPostImage,
  type LiveAgentGraphEntity,
} from "@openagentsinc/khala-sync"

import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

/** Named system-writer identity required for every non-client changelog append. */
export const LIVE_AGENT_GRAPH_PROJECTION_SYSTEM_REF =
  "system:live_agent_graph_projection.provider_runtime.v1"

/** Structural defense behind the shared ref-only graph contract. */
export const LIVE_AGENT_GRAPH_POST_IMAGE_FORBIDDEN_PATTERN =
  /"(?:token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|[A-Za-z]:\\Users\\)|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/i

export class LiveAgentGraphPostImageRedactionError extends Error {
  readonly _tag = "LiveAgentGraphPostImageRedactionError"
  override readonly name = "LiveAgentGraphPostImageRedactionError"
  constructor() {
    super("refusing to project live_agent_graph: forbidden private material")
  }
}

const assertLiveAgentGraphPostImageRedacted = (value: LiveAgentGraphEntity): void => {
  if (LIVE_AGENT_GRAPH_POST_IMAGE_FORBIDDEN_PATTERN.test(canonicalJson(value))) {
    throw new LiveAgentGraphPostImageRedactionError()
  }
}

export interface LiveAgentGraphProjectionDiagnostic {
  readonly reason: "storage_failed" | "redaction_refused" | "projection_failed"
  readonly messageSafe: string
}

export type LiveAgentGraphProjectionOutcome =
  | { readonly ok: true; readonly entry: ChangelogEntry }
  | { readonly ok: false; readonly diagnostic: LiveAgentGraphProjectionDiagnostic }

const diagnosticFromUnknown = (error: unknown): LiveAgentGraphProjectionDiagnostic => {
  if (error instanceof LiveAgentGraphPostImageRedactionError) {
    return { reason: "redaction_refused", messageSafe: error.message }
  }
  if ((error as { _tag?: unknown })?._tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      reason: "storage_failed",
      messageSafe: typeof messageSafe === "string" ? messageSafe : "storage failure",
    }
  }
  return {
    reason: "projection_failed",
    messageSafe: "live-agent graph projection failed",
  }
}

/**
 * Validate and append one full graph post-image to its canonical thread scope.
 *
 * This boundary is fail-soft because provider observation is not the business
 * transaction that owns a coding session yet. Decode/redaction happens before
 * opening storage; the append itself uses the normal dense-version Khala Sync
 * transaction writer. When session authority moves into the same transaction,
 * the fail-soft call site can be replaced without changing the entity shape.
 */
export const projectLiveAgentGraphBestEffort = async (
  sql: SyncSql,
  raw: unknown,
): Promise<LiveAgentGraphProjectionOutcome> => {
  let postImage: ReturnType<typeof projectLiveAgentGraphPostImage>
  try {
    postImage = projectLiveAgentGraphPostImage(raw as LiveAgentGraphEntity)
    assertLiveAgentGraphPostImageRedacted(postImage.value)
  } catch (error) {
    return { ok: false, diagnostic: diagnosticFromUnknown(error) }
  }

  try {
    const entry = await withSyncTransaction(sql, writer => writer.appendChange({
      scope: liveAgentGraphScope(postImage.value.threadRef),
      entityType: EntityType.make(LIVE_AGENT_GRAPH_ENTITY_TYPE),
      entityId: EntityId.make(postImage.entityId),
      op: "upsert",
      postImage: postImage.value,
      mutationRef: LIVE_AGENT_GRAPH_PROJECTION_SYSTEM_REF,
    }))
    return { ok: true, entry }
  } catch (error) {
    return { ok: false, diagnostic: diagnosticFromUnknown(error) }
  }
}
