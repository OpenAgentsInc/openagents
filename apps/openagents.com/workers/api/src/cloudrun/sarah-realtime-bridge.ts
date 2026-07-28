import {
  AUDIO_MEDIA_MAGIC,
  AUDIO_PROTOCOL_VERSION,
  MAX_AUDIO_PAYLOAD_BYTES,
  SARAH_VOICE_CONNECT_PATH,
  SARAH_VOICE_MODEL,
  SARAH_VOICE_PROTOCOL_VERSION,
  type SarahEditorCommand,
  type SarahVoiceServerControl,
  type VoiceIdentity,
  decodeMediaHeader,
  decodeSarahEditorCommand,
  decodeSarahVoiceClientControl,
} from '@openagentsinc/audio-contract'
import type {
  SarahRealtimeVoiceStore,
  SarahVoiceSessionRecord,
  SarahVoiceUsage,
} from '@openagentsinc/khala-sync-server'
import type { RuntimeServerWebSocket } from '@openagentsinc/runtime-platform'
import { createHash, randomUUID } from 'node:crypto'
import WebSocket, { type RawData } from 'ws'

import type { BackgroundTasks } from './execution-context'

export type SarahRealtimeBridgeData = {
  readonly _tag: 'sarah_realtime'
  readonly session: SarahVoiceSessionRecord
  readonly apiKey: string
  readonly safetyIdentifier: string
  readonly creditMsatPerMillionTokens: number
  readonly store: SarahRealtimeVoiceStore
  readonly closeStore: () => Promise<void>
  readonly tasks: BackgroundTasks
  upstream: WebSocket | undefined
  expectedControlSequence: number
  expectedAudioSequence: number
  serverSequence: number
  outputAudioSequence: number
  helloReceived: boolean
  clientClosed: boolean
  cleanupStarted: boolean
  currentOutputItemRef: string | undefined
  readonly proposals: Map<string, ToolProposal>
  meteringTail: Promise<void>
  expiryTimer: ReturnType<typeof setTimeout> | undefined
}

type ToolProposal = Readonly<{
  proposalRef: string
  proposalDigest: string
  callRef: string
  command: SarahEditorCommand
  confirmationRequired: boolean
  expiresAtMs: number
  state: 'proposed' | 'execute_sent'
}>

type Socket = RuntimeServerWebSocket<SarahRealtimeBridgeData>

const realtimeUrl =
  `wss://api.openai.com/v1/realtime?model=${SARAH_VOICE_MODEL}` as const
const MAX_CONTROL_BYTES = 32_768
const MAX_PROVIDER_FRAME_BYTES = 1_048_576

const safeTokenCount = (value: unknown): number =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
    ? value
    : 0

export const isSarahRealtimeVoiceUpgrade = (request: Request): boolean =>
  new URL(request.url).pathname === SARAH_VOICE_CONNECT_PATH &&
  request.headers.get('upgrade')?.toLowerCase() === 'websocket'

const sameIdentity = (left: VoiceIdentity, right: VoiceIdentity): boolean =>
  left.ownerRef === right.ownerRef &&
  left.deviceRef === right.deviceRef &&
  left.threadRef === right.threadRef &&
  left.sessionRef === right.sessionRef &&
  left.generation === right.generation

const identityForSession = (
  session: SarahVoiceSessionRecord,
): VoiceIdentity => ({
  ownerRef: session.ownerUserId,
  deviceRef: session.deviceRef,
  threadRef: session.threadRef,
  sessionRef: session.sessionRef,
  generation: session.generation,
})

type WithoutServerEnvelope<T> = T extends unknown
  ? Omit<T, 'schema' | 'identity' | 'sequence'>
  : never

const sendControl = (
  ws: Socket,
  frame: WithoutServerEnvelope<SarahVoiceServerControl>,
): void => {
  const payload = {
    schema: SARAH_VOICE_PROTOCOL_VERSION,
    identity: identityForSession(ws.data.session),
    sequence: ws.data.serverSequence,
    ...frame,
  }
  ws.data.serverSequence += 1
  ws.send(JSON.stringify(payload))
}

const closeClient = (
  ws: Socket,
  reason:
    | 'user_stop'
    | 'session_expired'
    | 'credit_limit'
    | 'provider_error'
    | 'transport_error'
    | 'server_shutdown',
  code = 1000,
): void => {
  if (!ws.data.clientClosed) {
    sendControl(ws, { _tag: 'closing', reason })
  }
  ws.data.clientClosed = true
  try {
    ws.close(code, reason)
  } catch {
    // The socket is already closed.
  }
}

const safeProviderClose = (upstream: WebSocket | undefined): void => {
  if (upstream === undefined) return
  try {
    upstream.close(1000)
  } catch {
    // The provider socket is already closed.
  }
}

const cleanup = (ws: Socket, closeReason: string): Promise<void> => {
  if (ws.data.cleanupStarted) return Promise.resolve()
  ws.data.cleanupStarted = true
  if (ws.data.expiryTimer !== undefined) clearTimeout(ws.data.expiryTimer)
  safeProviderClose(ws.data.upstream)
  return ws.data.meteringTail
    .then(() =>
      ws.data.store.settle({
        sessionRef: ws.data.session.sessionRef,
        closeReason,
        nowIso: new Date().toISOString(),
      }),
    )
    .then(
      () => ws.data.closeStore(),
      async () => {
        await ws.data.closeStore()
        throw new Error('Sarah voice settlement failed')
      },
    )
}

const decodeClientMedia = (
  bytes: Uint8Array,
): Readonly<{
  identity: VoiceIdentity
  sequence: number
  payload: Uint8Array
}> => {
  if (
    bytes.byteLength < 8 ||
    Buffer.from(bytes.subarray(0, 4)).toString('ascii') !== AUDIO_MEDIA_MAGIC
  ) {
    throw new Error('media_magic')
  }
  const headerLength = new DataView(
    bytes.buffer,
    bytes.byteOffset + 4,
    4,
  ).getUint32(0)
  if (
    headerLength < 2 ||
    headerLength > 8_192 ||
    8 + headerLength > bytes.byteLength
  ) {
    throw new Error('media_header_length')
  }
  const header = decodeMediaHeader(
    JSON.parse(
      Buffer.from(bytes.subarray(8, 8 + headerLength)).toString('utf8'),
    ) as unknown,
  )
  const payload = bytes.subarray(8 + headerLength)
  if (
    header.kind !== 'client_audio' ||
    header.codec !== 'pcm_s16le' ||
    header.sampleRateHz !== 24_000 ||
    header.channels !== 1 ||
    payload.byteLength === 0 ||
    payload.byteLength % 2 !== 0 ||
    payload.byteLength > MAX_AUDIO_PAYLOAD_BYTES ||
    payload.byteLength !== header.payloadLength ||
    createHash('sha256').update(payload).digest('hex') !== header.sha256
  ) {
    throw new Error('media_invalid')
  }
  return { identity: header.identity, sequence: header.sequence, payload }
}

const encodeServerAudio = (
  data: SarahRealtimeBridgeData,
  payload: Uint8Array,
  itemRef: string,
): Uint8Array => {
  const header = Buffer.from(
    JSON.stringify({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: 'server_tts',
      identity: identityForSession(data.session),
      sequence: data.outputAudioSequence,
      turnRef: itemRef,
      speechRef: itemRef,
      codec: 'pcm_s16le',
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: payload.byteLength,
      sha256: createHash('sha256').update(payload).digest('hex'),
    }),
  )
  data.outputAudioSequence += 1
  const frame = Buffer.allocUnsafe(8 + header.byteLength + payload.byteLength)
  frame.write(AUDIO_MEDIA_MAGIC, 0, 'ascii')
  frame.writeUInt32BE(header.byteLength, 4)
  header.copy(frame, 8)
  Buffer.from(payload).copy(frame, 8 + header.byteLength)
  return new Uint8Array(frame)
}

const toolDefinitions = [
  {
    type: 'function',
    name: 'editor_context_read',
    description: 'Read a bounded line range from one workspace document.',
    parameters: {
      type: 'object',
      additionalProperties: false,
      required: ['target', 'startLine', 'endLine'],
      properties: {
        target: { $ref: '#/$defs/target' },
        startLine: { type: 'integer', minimum: 1 },
        endLine: { type: 'integer', minimum: 1 },
      },
      $defs: {
        target: {
          type: 'object',
          additionalProperties: false,
          required: ['workspaceRef', 'path'],
          properties: {
            workspaceRef: { type: 'string', minLength: 1, maxLength: 256 },
            path: { type: 'string', minLength: 1, maxLength: 1024 },
            documentVersion: { type: 'string', maxLength: 2048 },
          },
        },
      },
    },
  },
  {
    type: 'function',
    name: 'editor_open_path',
    description: 'Open one workspace document.',
    parameters: {
      type: 'object',
      additionalProperties: false,
      required: ['target'],
      properties: {
        target: {
          type: 'object',
          additionalProperties: false,
          required: ['workspaceRef', 'path'],
          properties: {
            workspaceRef: { type: 'string', minLength: 1, maxLength: 256 },
            path: { type: 'string', minLength: 1, maxLength: 1024 },
            documentVersion: { type: 'string', maxLength: 2048 },
          },
        },
      },
    },
  },
  {
    type: 'function',
    name: 'editor_reveal_range',
    description: 'Reveal a bounded line range in one workspace document.',
    parameters: {
      type: 'object',
      additionalProperties: false,
      required: ['target', 'startLine', 'endLine'],
      properties: {
        target: { type: 'object' },
        startLine: { type: 'integer', minimum: 1 },
        endLine: { type: 'integer', minimum: 1 },
      },
    },
  },
  {
    type: 'function',
    name: 'editor_replace_selection',
    description:
      'Replace the current selection. The user must confirm before execution.',
    parameters: {
      type: 'object',
      additionalProperties: false,
      required: ['target', 'replacement'],
      properties: {
        target: { type: 'object' },
        replacement: { type: 'string', maxLength: 16_384 },
      },
    },
  },
  {
    type: 'function',
    name: 'editor_save_document',
    description:
      'Save one workspace document. The user must confirm before execution.',
    parameters: {
      type: 'object',
      additionalProperties: false,
      required: ['target'],
      properties: { target: { type: 'object' } },
    },
  },
  {
    type: 'function',
    name: 'start_agent_thread',
    description:
      'Start one Omega agent thread with a bounded message. The user must confirm before execution.',
    parameters: {
      type: 'object',
      additionalProperties: false,
      required: ['message', 'presentation'],
      properties: {
        message: { type: 'string', minLength: 1, maxLength: 16_384 },
        presentation: {
          type: 'string',
          enum: ['foreground', 'background'],
        },
      },
    },
  },
] as const

export const sessionUpdateForSarahClientProfile = (
  clientProfile: SarahVoiceSessionRecord['clientProfile'],
) => ({
  type: 'session.update' as const,
  session: {
    type: 'realtime' as const,
    model: SARAH_VOICE_MODEL,
    instructions:
      clientProfile === 'mobile_voice_only'
        ? 'You are Sarah in the OpenAgents mobile app. Have a voice conversation only. ' +
          'Do not request, perform, or claim any editor, file, URL, shell, Git, payment, or device action.'
        : 'You are Sarah in Omega. Use only the supplied editor tools. ' +
          'Never claim that a tool ran until its tool outcome says it ran.',
    output_modalities: ['audio'] as const,
    audio: {
      input: {
        format: { type: 'audio/pcm' as const, rate: 24_000 },
        transcription: { model: 'gpt-4o-mini-transcribe' },
        turn_detection: {
          type: 'semantic_vad' as const,
          eagerness: 'high' as const,
          interrupt_response: true,
          create_response: true,
        },
      },
      output: {
        format: { type: 'audio/pcm' as const, rate: 24_000 },
        voice: 'marin',
      },
    },
    tools: clientProfile === 'mobile_voice_only' ? [] : toolDefinitions,
    tool_choice: clientProfile === 'mobile_voice_only' ? 'none' : 'auto',
    max_output_tokens: 1_024,
  },
})

const commandTagForTool = (
  name: string,
): SarahEditorCommand['_tag'] | undefined =>
  ({
    editor_context_read: 'context_read',
    editor_open_path: 'open_path',
    editor_reveal_range: 'reveal_range',
    editor_replace_selection: 'replace_selection',
    editor_save_document: 'save_document',
    start_agent_thread: 'start_agent_thread',
  })[name] as SarahEditorCommand['_tag'] | undefined

export const sarahEditorCommandRequiresConfirmation = (
  command: SarahEditorCommand,
): boolean =>
  command._tag === 'replace_selection' ||
  command._tag === 'save_document' ||
  command._tag === 'start_agent_thread'

export const validateSarahEditorCommandTarget = (
  command: SarahEditorCommand,
): SarahEditorCommand => {
  if (command._tag === 'start_agent_thread') {
    if (new TextEncoder().encode(command.message).byteLength > 16_384) {
      throw new Error('agent_thread_message_not_allowed')
    }
    return command
  }
  const path = command.target.path
  const segments = path.split(/[\\/]/u)
  if (
    path.startsWith('/') ||
    /^[A-Za-z]:[\\/]/u.test(path) ||
    path.includes('\u0000') ||
    segments.includes('..')
  ) {
    throw new Error('editor_path_not_allowed')
  }
  if (
    (command._tag === 'context_read' || command._tag === 'reveal_range') &&
    (command.endLine < command.startLine ||
      command.endLine - command.startLine > 500)
  ) {
    throw new Error('editor_range_not_allowed')
  }
  return command
}

const proposalDigest = (
  session: SarahVoiceSessionRecord,
  proposalRef: string,
  command: SarahEditorCommand,
  expiresAtMs: number,
): string =>
  createHash('sha256')
    .update(
      JSON.stringify({
        protocol: SARAH_VOICE_PROTOCOL_VERSION,
        ownerUserId: session.ownerUserId,
        deviceRef: session.deviceRef,
        sessionRef: session.sessionRef,
        generation: session.generation,
        proposalRef,
        command,
        expiresAtMs,
      }),
    )
    .digest('hex')

const sendToolOutput = (
  upstream: WebSocket | undefined,
  callRef: string,
  output: Readonly<Record<string, unknown>>,
): void => {
  if (upstream?.readyState !== WebSocket.OPEN) return
  upstream.send(
    JSON.stringify({
      type: 'conversation.item.create',
      item: {
        type: 'function_call_output',
        call_id: callRef,
        output: JSON.stringify(output).slice(0, 2_048),
      },
    }),
  )
  upstream.send(JSON.stringify({ type: 'response.create' }))
}

const handleToolCall = (
  ws: Socket,
  item: Readonly<Record<string, unknown>>,
): void => {
  const callRef = typeof item.call_id === 'string' ? item.call_id : ''
  if (ws.data.session.clientProfile === 'mobile_voice_only') {
    sendToolOutput(ws.data.upstream, callRef, {
      ok: false,
      error: 'tool_not_allowed',
    })
    sendControl(ws, {
      _tag: 'error',
      code: 'tool_not_allowed',
      retryable: false,
    })
    closeClient(ws, 'transport_error', 1008)
    ws.data.tasks.add(cleanup(ws, 'mobile_tool_violation'))
    return
  }
  const name = typeof item.name === 'string' ? item.name : ''
  const tag = commandTagForTool(name)
  if (tag === undefined || callRef === '') {
    sendToolOutput(ws.data.upstream, callRef, {
      ok: false,
      error: 'tool_not_allowed',
    })
    sendControl(ws, {
      _tag: 'error',
      code: 'tool_not_allowed',
      retryable: false,
    })
    return
  }
  try {
    const args =
      typeof item.arguments === 'string'
        ? (JSON.parse(item.arguments) as unknown)
        : undefined
    const command = validateSarahEditorCommandTarget(
      decodeSarahEditorCommand({
        ...(typeof args === 'object' && args !== null ? args : {}),
        _tag: tag,
      }),
    )
    const proposalRef = `sarah_tool_${randomUUID()}`
    const expiresAtMs = Date.now() + 60_000
    const digest = proposalDigest(
      ws.data.session,
      proposalRef,
      command,
      expiresAtMs,
    )
    const confirmationRequired = sarahEditorCommandRequiresConfirmation(command)
    const proposal: ToolProposal = {
      proposalRef,
      proposalDigest: digest,
      callRef,
      command,
      confirmationRequired,
      expiresAtMs,
      state: confirmationRequired ? 'proposed' : 'execute_sent',
    }
    ws.data.proposals.set(proposalRef, proposal)
    sendControl(ws, {
      _tag: 'tool_proposal',
      proposalRef,
      proposalDigest: digest,
      command,
      confirmationRequired,
      expiresAtMs,
    })
    if (!confirmationRequired) {
      sendControl(ws, {
        _tag: 'tool_execute',
        proposalRef,
        proposalDigest: digest,
        command,
      })
    }
  } catch {
    sendToolOutput(ws.data.upstream, callRef, {
      ok: false,
      error: 'tool_not_allowed',
    })
    sendControl(ws, {
      _tag: 'error',
      code: 'tool_not_allowed',
      retryable: false,
    })
  }
}

export const usageFromProviderResponse = (
  event: Readonly<Record<string, unknown>>,
  creditMsatPerMillionTokens: number,
  observedAt: string,
): SarahVoiceUsage | undefined => {
  const response =
    typeof event.response === 'object' && event.response !== null
      ? (event.response as Readonly<Record<string, unknown>>)
      : undefined
  const usage =
    typeof response?.usage === 'object' && response.usage !== null
      ? (response.usage as Readonly<Record<string, unknown>>)
      : undefined
  const responseRef = typeof response?.id === 'string' ? response.id : undefined
  if (usage === undefined || responseRef === undefined) return undefined
  const inputDetails =
    typeof usage.input_token_details === 'object' &&
    usage.input_token_details !== null
      ? (usage.input_token_details as Readonly<Record<string, unknown>>)
      : {}
  const outputDetails =
    typeof usage.output_token_details === 'object' &&
    usage.output_token_details !== null
      ? (usage.output_token_details as Readonly<Record<string, unknown>>)
      : {}
  const inputTokens = safeTokenCount(usage.input_tokens)
  const outputTokens = safeTokenCount(usage.output_tokens)
  const totalTokens =
    safeTokenCount(usage.total_tokens) || inputTokens + outputTokens
  return {
    providerResponseRef: responseRef,
    inputTokens,
    outputTokens,
    cachedInputTokens: safeTokenCount(inputDetails.cached_tokens),
    audioInputTokens: safeTokenCount(inputDetails.audio_tokens),
    audioOutputTokens: safeTokenCount(outputDetails.audio_tokens),
    chargeMsat: Math.ceil(
      (totalTokens * creditMsatPerMillionTokens) / 1_000_000,
    ),
    observedAt,
  }
}

export const usageFromInputTranscription = (
  event: Readonly<Record<string, unknown>>,
  creditMsatPerMillionTokens: number,
  observedAt: string,
): SarahVoiceUsage | undefined => {
  const usage =
    typeof event.usage === 'object' && event.usage !== null
      ? (event.usage as Readonly<Record<string, unknown>>)
      : undefined
  const itemRef = typeof event.item_id === 'string' ? event.item_id : undefined
  if (usage === undefined || itemRef === undefined) return undefined
  const inputDetails =
    typeof usage.input_token_details === 'object' &&
    usage.input_token_details !== null
      ? (usage.input_token_details as Readonly<Record<string, unknown>>)
      : {}
  const inputTokens = safeTokenCount(usage.input_tokens)
  const outputTokens = safeTokenCount(usage.output_tokens)
  const totalTokens =
    safeTokenCount(usage.total_tokens) || inputTokens + outputTokens
  const contentIndex = safeTokenCount(event.content_index)
  return {
    providerResponseRef: `transcription:${itemRef}:${contentIndex}`,
    inputTokens,
    outputTokens,
    cachedInputTokens: safeTokenCount(inputDetails.cached_tokens),
    audioInputTokens: safeTokenCount(inputDetails.audio_tokens),
    audioOutputTokens: 0,
    chargeMsat: Math.ceil(
      (totalTokens * creditMsatPerMillionTokens) / 1_000_000,
    ),
    observedAt,
  }
}

const recordUsageAndEnforceLimit = (
  ws: Socket,
  usage: SarahVoiceUsage,
): void => {
  const metering = ws.data.meteringTail
    .then(() =>
      ws.data.store
        .recordUsage({ sessionRef: ws.data.session.sessionRef, usage })
        .then(result => {
          if (result.creditLimitReached) {
            closeClient(ws, 'credit_limit', 1008)
          }
        }),
    )
    .then(() => undefined)
  ws.data.meteringTail = metering
  ws.data.tasks.add(metering)
}

const handleProviderEvent = (ws: Socket, raw: string): void => {
  if (ws.data.cleanupStarted) return
  if (Buffer.byteLength(raw) > MAX_PROVIDER_FRAME_BYTES) {
    closeClient(ws, 'provider_error', 1011)
    return
  }
  let event: Readonly<Record<string, unknown>>
  try {
    event = JSON.parse(raw) as Readonly<Record<string, unknown>>
  } catch {
    return
  }
  const type = typeof event.type === 'string' ? event.type : ''
  if (type === 'session.updated') {
    sendControl(ws, {
      _tag: 'session_ready',
      model: SARAH_VOICE_MODEL,
      expiresAtMs: Date.parse(ws.data.session.sessionExpiresAt),
      reservedCreditMsat: ws.data.session.reservedMsat,
    })
    sendControl(ws, { _tag: 'lifecycle', state: 'listening' })
    return
  }
  if (type === 'input_audio_buffer.speech_started') {
    sendControl(ws, { _tag: 'lifecycle', state: 'listening' })
    return
  }
  if (type === 'response.created') {
    sendControl(ws, { _tag: 'lifecycle', state: 'thinking' })
    return
  }
  if (
    type === 'conversation.item.input_audio_transcription.delta' ||
    type === 'response.output_audio_transcript.delta'
  ) {
    const delta = typeof event.delta === 'string' ? event.delta : ''
    const itemRef =
      typeof event.item_id === 'string' ? event.item_id : 'utterance'
    if (delta !== '') {
      sendControl(ws, {
        _tag: 'transcript_delta',
        source: type.startsWith('conversation.') ? 'user' : 'assistant',
        utteranceRef: itemRef,
        text: delta.slice(0, 16_384),
      })
    }
    return
  }
  if (
    type === 'conversation.item.input_audio_transcription.completed' ||
    type === 'response.output_audio_transcript.done'
  ) {
    const text =
      typeof event.transcript === 'string'
        ? event.transcript
        : typeof event.text === 'string'
          ? event.text
          : ''
    const itemRef =
      typeof event.item_id === 'string' ? event.item_id : 'utterance'
    sendControl(ws, {
      _tag: 'transcript_final',
      source: type.startsWith('conversation.') ? 'user' : 'assistant',
      utteranceRef: itemRef,
      text: text.slice(0, 16_384),
    })
    if (type.startsWith('conversation.')) {
      const transcriptionUsage = usageFromInputTranscription(
        event,
        ws.data.creditMsatPerMillionTokens,
        new Date().toISOString(),
      )
      if (transcriptionUsage !== undefined) {
        recordUsageAndEnforceLimit(ws, transcriptionUsage)
      }
    }
    return
  }
  if (type === 'response.output_audio.delta') {
    const delta = typeof event.delta === 'string' ? event.delta : ''
    const itemRef =
      typeof event.item_id === 'string'
        ? event.item_id
        : (ws.data.currentOutputItemRef ?? 'assistant')
    ws.data.currentOutputItemRef = itemRef
    if (delta !== '') {
      const bytes = new Uint8Array(Buffer.from(delta, 'base64'))
      if (
        bytes.byteLength > 0 &&
        bytes.byteLength <= MAX_AUDIO_PAYLOAD_BYTES &&
        bytes.byteLength % 2 === 0
      ) {
        sendControl(ws, { _tag: 'lifecycle', state: 'speaking' })
        ws.send(encodeServerAudio(ws.data, bytes, itemRef))
      }
    }
    return
  }
  if (type === 'response.done') {
    const response =
      typeof event.response === 'object' && event.response !== null
        ? (event.response as Readonly<Record<string, unknown>>)
        : {}
    const output = Array.isArray(response.output) ? response.output : []
    for (const item of output) {
      if (
        typeof item === 'object' &&
        item !== null &&
        (item as { type?: unknown }).type === 'function_call'
      ) {
        handleToolCall(ws, item as Readonly<Record<string, unknown>>)
      }
    }
    const usage = usageFromProviderResponse(
      event,
      ws.data.creditMsatPerMillionTokens,
      new Date().toISOString(),
    )
    if (usage !== undefined) {
      recordUsageAndEnforceLimit(ws, usage)
    }
    sendControl(ws, { _tag: 'lifecycle', state: 'listening' })
    return
  }
  if (type === 'error') {
    sendControl(ws, {
      _tag: 'error',
      code: 'provider_unavailable',
      retryable: true,
    })
    closeClient(ws, 'provider_error', 1011)
  }
}

const handleControl = (ws: Socket, raw: string): void => {
  if (Buffer.byteLength(raw) > MAX_CONTROL_BYTES) {
    sendControl(ws, {
      _tag: 'error',
      code: 'invalid_frame',
      retryable: false,
    })
    return
  }
  try {
    const control = decodeSarahVoiceClientControl(JSON.parse(raw) as unknown)
    const identity = identityForSession(ws.data.session)
    if (
      !sameIdentity(control.identity, identity) ||
      control.sequence !== ws.data.expectedControlSequence
    ) {
      sendControl(ws, {
        _tag: 'error',
        code: 'sequence_gap',
        retryable: false,
      })
      return
    }
    ws.data.expectedControlSequence += 1
    if (!ws.data.helloReceived && control._tag !== 'session_hello') {
      throw new Error('hello_required')
    }
    switch (control._tag) {
      case 'session_hello':
        if (
          ws.data.helloReceived ||
          control.disclosureRef !== ws.data.session.disclosureRef
        ) {
          throw new Error('invalid_hello')
        }
        ws.data.helloReceived = true
        break
      case 'interrupt':
        ws.data.upstream?.send(JSON.stringify({ type: 'response.cancel' }))
        if (
          control.providerItemRef !== undefined &&
          control.playedAudioMs !== undefined
        ) {
          ws.data.upstream?.send(
            JSON.stringify({
              type: 'conversation.item.truncate',
              item_id: control.providerItemRef,
              content_index: 0,
              audio_end_ms: control.playedAudioMs,
            }),
          )
        }
        sendControl(ws, { _tag: 'interrupt_ack' })
        sendControl(ws, { _tag: 'lifecycle', state: 'interrupted' })
        break
      case 'tool_decision': {
        const proposal = ws.data.proposals.get(control.proposalRef)
        if (
          proposal === undefined ||
          proposal.proposalDigest !== control.proposalDigest ||
          !proposal.confirmationRequired ||
          proposal.state !== 'proposed' ||
          proposal.expiresAtMs < Date.now()
        ) {
          sendControl(ws, {
            _tag: 'error',
            code: 'confirmation_required',
            retryable: false,
          })
          break
        }
        if (control.decision === 'decline') {
          ws.data.proposals.delete(proposal.proposalRef)
          sendToolOutput(ws.data.upstream, proposal.callRef, {
            ok: false,
            error: 'confirmation_refused',
          })
          break
        }
        const executing = { ...proposal, state: 'execute_sent' as const }
        ws.data.proposals.set(proposal.proposalRef, executing)
        sendControl(ws, {
          _tag: 'tool_execute',
          proposalRef: proposal.proposalRef,
          proposalDigest: proposal.proposalDigest,
          command: proposal.command,
        })
        break
      }
      case 'tool_outcome': {
        const proposal = ws.data.proposals.get(control.proposalRef)
        if (
          proposal === undefined ||
          proposal.proposalDigest !== control.proposalDigest ||
          proposal.state !== 'execute_sent' ||
          proposal.expiresAtMs < Date.now()
        ) {
          throw new Error('invalid_tool_outcome')
        }
        ws.data.proposals.delete(proposal.proposalRef)
        sendToolOutput(ws.data.upstream, proposal.callRef, {
          ok: control.ok,
          outcomeRef: control.outcomeRef,
          summary: control.summary,
        })
        sendControl(ws, {
          _tag: 'tool_outcome_ref',
          proposalRef: proposal.proposalRef,
          outcomeRef: control.outcomeRef,
        })
        break
      }
      case 'heartbeat':
        sendControl(ws, { _tag: 'heartbeat' })
        break
      case 'close':
        closeClient(ws, 'user_stop')
        ws.data.tasks.add(cleanup(ws, control.reason))
        break
    }
  } catch {
    sendControl(ws, {
      _tag: 'error',
      code: 'invalid_frame',
      retryable: false,
    })
  }
}

export const makeSarahRealtimeWebSocketHandlers = () => ({
  open(ws: Socket) {
    sendControl(ws, { _tag: 'lifecycle', state: 'connecting' })
    const upstream = new WebSocket(realtimeUrl, {
      headers: {
        authorization: `Bearer ${ws.data.apiKey}`,
        'openai-safety-identifier': ws.data.safetyIdentifier,
      },
      maxPayload: MAX_PROVIDER_FRAME_BYTES,
    })
    ws.data.upstream = upstream
    upstream.on('open', () => {
      upstream.send(
        JSON.stringify(
          sessionUpdateForSarahClientProfile(
            ws.data.session.clientProfile,
          ),
        ),
      )
    })
    upstream.on('message', (data: RawData, isBinary: boolean) => {
      if (isBinary) return
      handleProviderEvent(ws, data.toString())
    })
    upstream.on('close', () => {
      if (!ws.data.clientClosed) closeClient(ws, 'provider_error', 1011)
      ws.data.tasks.add(cleanup(ws, 'provider_closed'))
    })
    upstream.on('error', () => {
      sendControl(ws, {
        _tag: 'error',
        code: 'provider_unavailable',
        retryable: true,
      })
      closeClient(ws, 'provider_error', 1011)
    })
    const expiryDelay = Math.max(
      1,
      Date.parse(ws.data.session.sessionExpiresAt) - Date.now(),
    )
    ws.data.expiryTimer = setTimeout(() => {
      closeClient(ws, 'session_expired', 1008)
      ws.data.tasks.add(cleanup(ws, 'session_expired'))
    }, expiryDelay)
  },
  message(ws: Socket, message: string | Uint8Array) {
    if (typeof message === 'string') {
      handleControl(ws, message)
      return
    }
    if (
      !ws.data.helloReceived ||
      ws.data.upstream?.readyState !== WebSocket.OPEN
    ) {
      sendControl(ws, {
        _tag: 'error',
        code: 'invalid_frame',
        retryable: true,
      })
      return
    }
    try {
      const media = decodeClientMedia(message)
      if (
        !sameIdentity(media.identity, identityForSession(ws.data.session)) ||
        media.sequence !== ws.data.expectedAudioSequence
      ) {
        sendControl(ws, {
          _tag: 'error',
          code: 'sequence_gap',
          retryable: false,
        })
        return
      }
      ws.data.expectedAudioSequence += 1
      ws.data.upstream.send(
        JSON.stringify({
          type: 'input_audio_buffer.append',
          audio: Buffer.from(media.payload).toString('base64'),
        }),
      )
      sendControl(ws, {
        _tag: 'audio_ack',
        acknowledgedClientSequence: media.sequence,
      })
    } catch {
      sendControl(ws, {
        _tag: 'error',
        code: 'invalid_frame',
        retryable: false,
      })
    }
  },
  close(ws: Socket, _code: number, reason: string) {
    ws.data.clientClosed = true
    ws.data.tasks.add(cleanup(ws, reason === '' ? 'client_closed' : reason))
  },
})

export const makeSarahRealtimeBridgeData = (
  input: Readonly<{
    session: SarahVoiceSessionRecord
    apiKey: string
    safetyIdentifier: string
    creditMsatPerMillionTokens: number
    store: SarahRealtimeVoiceStore
    closeStore: () => Promise<void>
    tasks: BackgroundTasks
  }>,
): SarahRealtimeBridgeData => ({
  _tag: 'sarah_realtime',
  ...input,
  upstream: undefined,
  expectedControlSequence: 0,
  expectedAudioSequence: 0,
  serverSequence: 0,
  outputAudioSequence: 0,
  helloReceived: false,
  clientClosed: false,
  cleanupStarted: false,
  currentOutputItemRef: undefined,
  proposals: new Map(),
  meteringTail: Promise.resolve(),
  expiryTimer: undefined,
})

export const parseSarahRealtimeBridgeCreditRate = (
  value: string | undefined,
): number | undefined => {
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined
}
