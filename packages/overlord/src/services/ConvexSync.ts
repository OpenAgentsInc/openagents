/**
 * Convex synchronization service for Overlord
 * Handles saving Claude Code conversations to Convex database
 * @since Phase 2
 */
import { client } from "@openagentsinc/convex"
import { Context, Effect, Layer } from "effect"
import * as DatabaseMapper from "./DatabaseMapper.js"
import type * as JSONLParser from "./JSONLParser.js"

// Service interface
export interface ConvexSyncService {
  readonly saveSession: (
    sessionId: string,
    userId: string,
    projectPath: string,
    entries: ReadonlyArray<JSONLParser.LogEntry>
  ) => Effect.Effect<void, Error>
}

export const ConvexSyncService = Context.GenericTag<ConvexSyncService>(
  "@openagentsinc/overlord/ConvexSyncService"
)

// Implementation
export const ConvexSyncServiceLive = Layer.succeed(
  ConvexSyncService,
  {
    saveSession: (sessionId, userId, projectPath, entries) =>
      Effect.gen(function*() {
        // Create session record
        const sessionRecord = DatabaseMapper.createSessionRecord(
          sessionId,
          userId,
          projectPath,
          entries
        )

        // Check if session exists
        const existingSession = yield* client.ConvexClient.sessions.getById(sessionId).pipe(
          Effect.catchAll(() => Effect.succeed(null))
        )

        if (!existingSession) {
          // Create new session
          yield* client.ConvexClient.sessions.create({
            id: sessionRecord.id,
            user_id: sessionRecord.user_id,
            project_path: sessionRecord.project_path,
            project_name: sessionRecord.project_name,
            status: sessionRecord.status,
            started_at: sessionRecord.started_at.getTime(),
            last_activity: sessionRecord.last_activity.getTime(),
            message_count: sessionRecord.message_count,
            total_cost: sessionRecord.total_cost
          })
          yield* Effect.log(`Created new session: ${sessionId}`)
        } else {
          // Update existing session (skip project update if function not deployed)
          yield* client.ConvexClient.sessions.updateActivity(sessionId)
          yield* Effect.log(`Updated existing session: ${sessionId}`)
        }

        // Save all messages
        for (const entry of entries) {
          const messageRecord = DatabaseMapper.logEntryToMessageRecord(entry, sessionId)

          // Check if message exists
          const existingMessage = yield* client.ConvexClient.messages.getByUuid(
            sessionId,
            messageRecord.entry_uuid
          ).pipe(
            Effect.catchAll(() => Effect.succeed(null))
          )

          if (!existingMessage) {
            yield* client.ConvexClient.messages.create({
              session_id: messageRecord.session_id,
              entry_uuid: messageRecord.entry_uuid,
              entry_type: messageRecord.entry_type,
              role: messageRecord.role,
              content: messageRecord.content,
              thinking: messageRecord.thinking,
              summary: messageRecord.summary,
              model: messageRecord.model,
              token_usage: messageRecord.token_usage,
              cost: messageRecord.cost,
              timestamp: messageRecord.timestamp.getTime(),
              turn_count: messageRecord.turn_count,
              tool_name: messageRecord.tool_name,
              tool_input: messageRecord.tool_input,
              tool_use_id: messageRecord.tool_use_id,
              tool_output: messageRecord.tool_output,
              tool_is_error: messageRecord.tool_is_error
            })

            // Log warning for images (not yet implemented)
            if (entry.type === "user" && entry.message.images && entry.message.images.length > 0) {
              yield* Effect.logWarning(
                `Message ${entry.uuid} has ${entry.message.images.length} images - image storage not yet implemented`
              )
            }
          } else {
            // For testing: try to update existing message content to apply fixes
            try {
              if (messageRecord.content !== undefined || messageRecord.thinking !== undefined) {
                // For now, just log what we would update rather than actually updating
                yield* Effect.log(`Would update message ${messageRecord.entry_uuid} with content length: ${messageRecord.content?.length || 0}`)
              }
            } catch (error) {
              yield* Effect.log(`Failed to update message content: ${error}`)
            }
          }
        }

        yield* Effect.log(`Synced ${entries.length} entries for session ${sessionId}`)
      })
  }
)
