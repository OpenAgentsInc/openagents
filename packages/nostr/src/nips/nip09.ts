/**
 * NIP-09: Event Deletion
 * Implements event deletion requests and content moderation
 *
 * Deletion Events (kind 5):
 * - Request deletion of previous events by the same author
 * - References events to delete using e-tags
 * - Optional deletion reason in content field
 * - Clients SHOULD honor deletion requests but MAY choose not to
 *
 * Security Considerations:
 * - Only the original author can request deletion of their events
 * - Relays and clients can implement their own deletion policies
 * - Deletion is a request, not a guarantee
 * - Some clients may preserve deleted content for archival purposes
 */

import { Context, Data, Effect, Layer, Schema } from "effect"
import type { PrivateKey } from "../core/Schema.js"
import { PublicKey, Signature } from "../core/Schema.js"

// --- Types ---
export const DeletionReason = Schema.Union(
  Schema.Literal("inappropriate_content"),
  Schema.Literal("personal_request"),
  Schema.Literal("legal_requirement"),
  Schema.Literal("spam"),
  Schema.Literal("duplicate"),
  Schema.Literal("error"),
  Schema.Literal("other")
)
export type DeletionReason = Schema.Schema.Type<typeof DeletionReason>

export const DeletionEvent = Schema.Struct({
  id: Schema.String,
  pubkey: PublicKey,
  created_at: Schema.Number,
  kind: Schema.Literal(5),
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String, // Deletion reason (optional)
  sig: Signature
})
export type DeletionEvent = Schema.Schema.Type<typeof DeletionEvent>

export const DeletionRequest = Schema.Struct({
  eventIds: Schema.Array(Schema.String),
  reason: Schema.optional(DeletionReason),
  comment: Schema.optional(Schema.String)
})
export type DeletionRequest = Schema.Schema.Type<typeof DeletionRequest>

export const DeletionStatus = Schema.Struct({
  eventId: Schema.String,
  deletedAt: Schema.Number,
  deletionEventId: Schema.String,
  reason: Schema.optional(DeletionReason),
  processed: Schema.Boolean
})
export type DeletionStatus = Schema.Schema.Type<typeof DeletionStatus>

export const DeletionPolicy = Schema.Struct({
  allowSelfDeletion: Schema.Boolean,
  allowModeratorDeletion: Schema.Boolean,
  preserveArchive: Schema.Boolean,
  timeLimit: Schema.optional(Schema.Number), // Time limit in seconds for deletion requests
  moderatorPubkeys: Schema.optional(Schema.Array(PublicKey))
})
export type DeletionPolicy = Schema.Schema.Type<typeof DeletionPolicy>

// --- Errors ---
export class Nip09Error extends Data.TaggedError("Nip09Error")<{
  reason:
    | "unauthorized_deletion"
    | "event_not_found"
    | "invalid_request"
    | "policy_violation"
    | "time_limit_exceeded"
    | "already_deleted"
  message: string
  eventId?: string
  requestedBy?: PublicKey
  originalAuthor?: PublicKey
  cause?: unknown
}> {}

// --- Service ---
export class Nip09Service extends Context.Tag("nips/Nip09Service")<
  Nip09Service,
  {
    /**
     * Create a deletion event (kind 5)
     */
    readonly createDeletionEvent: (
      request: DeletionRequest,
      authorPrivkey: PrivateKey
    ) => Effect.Effect<DeletionEvent, Nip09Error>

    /**
     * Parse a deletion event
     */
    readonly parseDeletionEvent: (
      event: DeletionEvent
    ) => Effect.Effect<DeletionRequest, Nip09Error>

    /**
     * Validate deletion request
     */
    readonly validateDeletion: (
      request: DeletionRequest,
      requestedBy: PublicKey,
      originalEvents: Array<{ id: string; pubkey: PublicKey; created_at: number }>,
      policy: DeletionPolicy
    ) => Effect.Effect<void, Nip09Error>

    /**
     * Process deletion request
     */
    readonly processDeletion: (
      request: DeletionRequest,
      requestedBy: PublicKey,
      policy: DeletionPolicy
    ) => Effect.Effect<Array<DeletionStatus>, Nip09Error>

    /**
     * Check if event is deleted
     */
    readonly isEventDeleted: (
      eventId: string
    ) => Effect.Effect<DeletionStatus | undefined, Nip09Error>

    /**
     * Get deletion reason for event
     */
    readonly getDeletionReason: (
      eventId: string
    ) => Effect.Effect<DeletionReason | undefined, Nip09Error>

    /**
     * List all deleted events by author
     */
    readonly getDeletedEventsByAuthor: (
      authorPubkey: PublicKey
    ) => Effect.Effect<Array<DeletionStatus>, Nip09Error>

    /**
     * Restore deleted event (if policy allows)
     */
    readonly restoreEvent: (
      eventId: string,
      requestedBy: PublicKey,
      policy: DeletionPolicy
    ) => Effect.Effect<void, Nip09Error>

    /**
     * Get deletion statistics
     */
    readonly getDeletionStats: () => Effect.Effect<{
      totalDeleted: number
      deletionsByReason: Record<DeletionReason, number>
      deletionsByAuthor: Record<PublicKey, number>
    }, Nip09Error>
  }
>() {}

// --- Implementation ---
export const Nip09ServiceLive = Layer.succeed(
  Nip09Service,
  {
    createDeletionEvent: (
      request: DeletionRequest,
      _authorPrivkey: PrivateKey
    ): Effect.Effect<DeletionEvent, Nip09Error> =>
      Effect.gen(function*() {
        // Validate request
        if (request.eventIds.length === 0) {
          return yield* Effect.fail(
            new Nip09Error({
              reason: "invalid_request",
              message: "Deletion request must specify at least one event ID"
            })
          )
        }

        // Create e-tags for events to delete
        const tags: Array<Array<string>> = request.eventIds.map((eventId) => ["e", eventId])

        // Add reason as a tag if provided
        if (request.reason) {
          tags.push(["reason", request.reason])
        }

        // Create content with reason and comment
        const contentParts: Array<string> = []
        if (request.reason) {
          contentParts.push(`Reason: ${request.reason}`)
        }
        if (request.comment) {
          contentParts.push(`Comment: ${request.comment}`)
        }
        const content = contentParts.join("\n")

        const now = Math.floor(Date.now() / 1000)

        // TODO: Implement proper event signing with authorPrivkey
        // For now, create a placeholder event
        const event: DeletionEvent = {
          id: "placeholder-id",
          pubkey: "placeholder-pubkey" as PublicKey,
          created_at: now,
          kind: 5,
          tags,
          content,
          sig: "placeholder-signature" as Signature
        }

        return event
      }),

    parseDeletionEvent: (
      event: DeletionEvent
    ): Effect.Effect<DeletionRequest, Nip09Error> =>
      Effect.gen(function*() {
        // Extract event IDs from e-tags
        const eventIds: Array<string> = []
        for (const tag of event.tags) {
          if (tag[0] === "e" && tag[1]) {
            eventIds.push(tag[1])
          }
        }

        if (eventIds.length === 0) {
          return yield* Effect.fail(
            new Nip09Error({
              reason: "invalid_request",
              message: "Deletion event must specify at least one event to delete"
            })
          )
        }

        // Extract reason from tags
        const reasonTag = event.tags.find((tag) => tag[0] === "reason")
        const reason = reasonTag?.[1] as DeletionReason | undefined

        // Extract comment from content
        const comment = event.content.trim() || undefined

        return {
          eventIds,
          reason,
          comment
        }
      }),

    validateDeletion: (
      request: DeletionRequest,
      requestedBy: PublicKey,
      originalEvents: Array<{ id: string; pubkey: PublicKey; created_at: number }>,
      policy: DeletionPolicy
    ): Effect.Effect<void, Nip09Error> =>
      Effect.gen(function*() {
        const now = Math.floor(Date.now() / 1000)

        for (const eventData of originalEvents) {
          // Check if requester is the original author
          const isOriginalAuthor = eventData.pubkey === requestedBy

          // Check if requester is a moderator
          const isModerator = policy.moderatorPubkeys?.includes(requestedBy) ?? false

          if (!isOriginalAuthor && !isModerator) {
            return yield* Effect.fail(
              new Nip09Error({
                reason: "unauthorized_deletion",
                message: "Only the original author or moderators can delete events",
                eventId: eventData.id,
                requestedBy,
                originalAuthor: eventData.pubkey
              })
            )
          }

          // Check self-deletion policy
          if (isOriginalAuthor && !policy.allowSelfDeletion) {
            return yield* Effect.fail(
              new Nip09Error({
                reason: "policy_violation",
                message: "Self-deletion not allowed by policy",
                eventId: eventData.id,
                requestedBy
              })
            )
          }

          // Check moderator deletion policy
          if (isModerator && !policy.allowModeratorDeletion) {
            return yield* Effect.fail(
              new Nip09Error({
                reason: "policy_violation",
                message: "Moderator deletion not allowed by policy",
                eventId: eventData.id,
                requestedBy
              })
            )
          }

          // Check time limit
          if (policy.timeLimit && isOriginalAuthor) {
            const eventAge = now - eventData.created_at
            if (eventAge > policy.timeLimit) {
              return yield* Effect.fail(
                new Nip09Error({
                  reason: "time_limit_exceeded",
                  message: `Deletion time limit exceeded for event ${eventData.id}`,
                  eventId: eventData.id,
                  requestedBy
                })
              )
            }
          }
        }
      }),

    processDeletion: (
      request: DeletionRequest,
      _requestedBy: PublicKey,
      _policy: DeletionPolicy
    ): Effect.Effect<Array<DeletionStatus>, Nip09Error> =>
      Effect.sync(() => {
        const now = Date.now()
        const deletionEventId = `deletion-${now}-${Math.random().toString(36).substr(2, 9)}`

        const statuses: Array<DeletionStatus> = request.eventIds.map((eventId) => ({
          eventId,
          deletedAt: now,
          deletionEventId,
          reason: request.reason,
          processed: true
        }))

        // TODO: Implement actual deletion logic
        // This would involve:
        // 1. Marking events as deleted in storage
        // 2. Optionally preserving in archive if policy.preserveArchive is true
        // 3. Notifying subscribers about deletions
        // 4. Updating indexes

        return statuses
      }),

    isEventDeleted: (
      _eventId: string
    ): Effect.Effect<DeletionStatus | undefined, Nip09Error> => Effect.succeed(undefined), // TODO: Implement storage lookup

    getDeletionReason: (
      _eventId: string
    ): Effect.Effect<DeletionReason | undefined, Nip09Error> => Effect.succeed(undefined), // TODO: Implement storage lookup

    getDeletedEventsByAuthor: (
      _authorPubkey: PublicKey
    ): Effect.Effect<Array<DeletionStatus>, Nip09Error> => Effect.succeed([]), // TODO: Implement storage lookup

    restoreEvent: (
      _eventId: string,
      _requestedBy: PublicKey,
      _policy: DeletionPolicy
    ): Effect.Effect<void, Nip09Error> =>
      Effect.gen(function*() {
        // TODO: Implement event restoration
        // This would require checking if:
        // 1. Event exists in archive
        // 2. Requester has permission to restore
        // 3. Policy allows restoration
      }),

    getDeletionStats: (): Effect.Effect<{
      totalDeleted: number
      deletionsByReason: Record<DeletionReason, number>
      deletionsByAuthor: Record<PublicKey, number>
    }, Nip09Error> =>
      Effect.succeed({
        totalDeleted: 0,
        deletionsByReason: {
          inappropriate_content: 0,
          personal_request: 0,
          legal_requirement: 0,
          spam: 0,
          duplicate: 0,
          error: 0,
          other: 0
        },
        deletionsByAuthor: {}
      })
  }
)

// --- Utility Functions ---

/**
 * Create default deletion policy
 */
export const createDefaultDeletionPolicy = (): DeletionPolicy => ({
  allowSelfDeletion: true,
  allowModeratorDeletion: true,
  preserveArchive: true,
  timeLimit: 24 * 60 * 60, // 24 hours
  moderatorPubkeys: []
})

/**
 * Create restrictive deletion policy
 */
export const createRestrictiveDeletionPolicy = (): DeletionPolicy => ({
  allowSelfDeletion: false,
  allowModeratorDeletion: true,
  preserveArchive: true,
  timeLimit: 60 * 60, // 1 hour
  moderatorPubkeys: []
})

/**
 * Extract event IDs from deletion event
 */
export const extractEventIds = (event: DeletionEvent): Array<string> => {
  return event.tags
    .filter((tag) => tag[0] === "e" && tag[1])
    .map((tag) => tag[1])
}

/**
 * Extract deletion reason from event
 */
export const extractDeletionReason = (event: DeletionEvent): DeletionReason | undefined => {
  const reasonTag = event.tags.find((tag) => tag[0] === "reason")
  return reasonTag?.[1] as DeletionReason | undefined
}

/**
 * Check if deletion is within time limit
 */
export const isWithinTimeLimit = (
  eventCreatedAt: number,
  deletionRequestedAt: number,
  timeLimit: number
): boolean => {
  return (deletionRequestedAt - eventCreatedAt) <= timeLimit
}

/**
 * Format deletion reason for display
 */
export const formatDeletionReason = (reason: DeletionReason): string => {
  switch (reason) {
    case "inappropriate_content":
      return "Inappropriate Content"
    case "personal_request":
      return "Personal Request"
    case "legal_requirement":
      return "Legal Requirement"
    case "spam":
      return "Spam"
    case "duplicate":
      return "Duplicate Content"
    case "error":
      return "Posted in Error"
    case "other":
      return "Other"
    default:
      return "Unknown"
  }
}

/**
 * Create deletion request for single event
 */
export const createSingleEventDeletion = (
  eventId: string,
  reason?: DeletionReason,
  comment?: string
): DeletionRequest => ({
  eventIds: [eventId],
  reason,
  comment
})

/**
 * Create bulk deletion request
 */
export const createBulkDeletion = (
  eventIds: Array<string>,
  reason?: DeletionReason,
  comment?: string
): DeletionRequest => ({
  eventIds,
  reason,
  comment
})

/**
 * Validate deletion event structure
 */
export const validateDeletionEventStructure = (event: DeletionEvent): Effect.Effect<void, Nip09Error> =>
  Effect.gen(function*() {
    if (event.kind !== 5) {
      return yield* Effect.fail(
        new Nip09Error({
          reason: "invalid_request",
          message: "Event must be kind 5 for deletion"
        })
      )
    }

    const eventIds = extractEventIds(event)
    if (eventIds.length === 0) {
      return yield* Effect.fail(
        new Nip09Error({
          reason: "invalid_request",
          message: "Deletion event must reference at least one event to delete"
        })
      )
    }

    // Validate event ID format (32-byte hex strings)
    for (const eventId of eventIds) {
      if (!/^[a-f0-9]{64}$/.test(eventId)) {
        return yield* Effect.fail(
          new Nip09Error({
            reason: "invalid_request",
            message: `Invalid event ID format: ${eventId}`,
            eventId
          })
        )
      }
    }
  })
