import type { SessionNotification } from '@agentclientprotocol/sdk'
import { sessionNotificationSchema } from '@agentclientprotocol/sdk'

export type ParseResult<T> = { ok: true; value: T } | { ok: false; error: unknown }

export function parseSessionNotification(input: unknown): ParseResult<SessionNotification> {
  try {
    const r = sessionNotificationSchema.safeParse(input as any)
    if (r.success) return { ok: true, value: r.data }
    return { ok: false, error: r.error }
  } catch (err) {
    return { ok: false, error: err }
  }
}

export function isSessionUpdateKind(
  n: SessionNotification,
  kind:
    | 'user_message_chunk'
    | 'agent_message_chunk'
    | 'agent_thought_chunk'
    | 'tool_call'
    | 'tool_call_update'
    | 'plan'
    | 'available_commands_update'
    | 'current_mode_update',
): boolean {
  return n?.update?.sessionUpdate === kind
}

