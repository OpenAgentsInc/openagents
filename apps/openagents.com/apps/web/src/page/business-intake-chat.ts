// Pure state core for the /business Khala intake console.
//
// The browser holds the transcript; every send posts the running conversation
// to the bounded server-side interview at POST /api/public/business-intake-chat
// and receives one Khala reply back. This module is the tested, DOM-free
// state machine; `business-intake-chat-controller.ts` owns the DOM.
//
// Client bounds deliberately mirror the server's fail-closed bounds so a
// well-behaved client never sends a request the server would 400.

export const BUSINESS_INTAKE_CHAT_ENDPOINT = '/api/public/business-intake-chat'

export const INTAKE_CHAT_MAX_MESSAGES = 24
export const INTAKE_CHAT_MAX_MESSAGE_CHARS = 2000

export type IntakeChatRole = 'user' | 'assistant'

export type IntakeChatMessage = Readonly<{
  role: IntakeChatRole
  content: string
}>

export type IntakeChatPhase =
  | 'ready'
  | 'waiting'
  | 'done'
  | 'rate_limited'
  | 'unavailable'
  | 'error'

export type IntakeChatState = Readonly<{
  messages: ReadonlyArray<IntakeChatMessage>
  phase: IntakeChatPhase
  spec: string | null
}>

export const initialIntakeChatState: IntakeChatState = {
  messages: [],
  phase: 'ready',
  spec: null,
}

// A user message is sendable when it is non-empty within the char bound, the
// console is idle, and appending user+assistant stays within the message cap.
export const canSendIntakeMessage = (
  state: IntakeChatState,
  text: string,
): boolean => {
  const trimmed = text.trim()
  return (
    state.phase === 'ready' &&
    trimmed.length > 0 &&
    trimmed.length <= INTAKE_CHAT_MAX_MESSAGE_CHARS &&
    state.messages.length + 2 <= INTAKE_CHAT_MAX_MESSAGES
  )
}

export const appendUserMessage = (
  state: IntakeChatState,
  text: string,
): IntakeChatState => ({
  ...state,
  messages: [...state.messages, { role: 'user', content: text.trim() }],
  phase: 'waiting',
})

export const intakeChatRequestBody = (
  state: IntakeChatState,
): string => JSON.stringify({ messages: state.messages })

export type IntakeChatReply = Readonly<{
  reply: string
  done: boolean
  spec: string | null
}>

export const decodeIntakeChatReply = (value: unknown): IntakeChatReply | null => {
  if (typeof value !== 'object' || value === null) {
    return null
  }
  const record = value as Record<string, unknown>
  if (record['ok'] !== true || typeof record['reply'] !== 'string') {
    return null
  }
  const done = record['done'] === true
  const spec = typeof record['spec'] === 'string' ? record['spec'] : null
  return { reply: record['reply'], done: done && spec !== null, spec }
}

export const applyIntakeChatReply = (
  state: IntakeChatState,
  reply: IntakeChatReply,
): IntakeChatState => ({
  messages: [...state.messages, { role: 'assistant', content: reply.reply }],
  phase: reply.done ? 'done' : 'ready',
  spec: reply.done ? reply.spec : state.spec,
})

// Map a failed send to the honest phase. The transcript keeps the user's
// message (the server is stateless; a retry resends the whole conversation),
// and the console recovers to `ready` for retryable failures so the visitor
// can try again — except `done`, which is terminal.
export const applyIntakeChatFailure = (
  state: IntakeChatState,
  status: number | 'network',
): IntakeChatState => ({
  ...state,
  phase:
    status === 429
      ? 'rate_limited'
      : status === 503
        ? 'unavailable'
        : 'error',
})

export const recoverIntakeChat = (state: IntakeChatState): IntakeChatState =>
  state.phase === 'done' ? state : { ...state, phase: 'ready' }

// Status-strip copy per phase. Short, honest, mono-register.
export const intakeChatStatusLine = (state: IntakeChatState): string => {
  switch (state.phase) {
    case 'ready':
      return state.messages.length === 0
        ? 'describe what you need — Khala scopes the quick win'
        : 'your turn'
    case 'waiting':
      return 'Khala is thinking…'
    case 'done':
      return 'intake spec drafted — review and submit below'
    case 'rate_limited':
      return 'rate limited — wait a minute, then send again'
    case 'unavailable':
      return 'intake console offline — the form below works'
    case 'error':
      return 'send failed — try again, or use the form below'
  }
}
