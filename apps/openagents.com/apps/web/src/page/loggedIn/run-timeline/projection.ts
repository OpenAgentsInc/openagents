import { Option } from 'effect'

import {
  nestedUnknown,
  parseJsonRecord,
  recordFromUnknown,
  textFromUnknown,
} from '../../../json-boundary'
import { threadRouter } from '../../../route'
import * as Ui from '../../../ui'
import {
  agentRunExternalRefFromNullable,
  providerAccountBundleFromAuth,
  runDurationFromNullable,
} from '../model'
import type {
  ChatRun,
  ChatRunEvent,
  Model,
  TeamChatMessageRecord,
  TeamChatRunSummary,
} from '../model'

export type RunTimelineTextPart = Extract<
  Ui.WorkroomTimelinePart,
  { kind: 'text' }
>
export type RunTimelineToolPart = Extract<
  Ui.WorkroomTimelinePart,
  { kind: 'tool' }
>
export type RunTimelineReconnectPart = RunTimelineToolPart &
  Readonly<{
    actionHref: '/settings/connections'
    actionLabel: 'Reconnect ChatGPT'
    subtitle: 'Reconnect required'
    title: 'ChatGPT not connected'
  }>
export type RunTimelineArtifactPart = RunTimelineToolPart &
  Readonly<{
    subtitle: 'OpenCode artifact'
    title: 'Artifact captured'
  }>
export type RunTimelineSummaryPart = RunTimelineToolPart
export type RunTimelinePart = Ui.WorkroomTimelinePart
export type RunTimelineMessage = Ui.WorkroomTimelineMessage

type TimelineMessage = RunTimelineMessage
export type ProviderConnectionState = 'connected' | 'not_connected'

const runStatusToToolStatus = (
  status: string | null,
): RunTimelineToolPart['status'] => {
  if (status === 'completed') {
    return 'completed'
  }

  if (status === 'failed' || status === 'canceled') {
    return 'failed'
  }

  if (status === 'running' || status === 'waiting_for_input') {
    return 'running'
  }

  return 'queued'
}

const runStatusToTimelineStatus = (
  status: string,
): NonNullable<TimelineMessage['status']> =>
  status === 'queued' || status === 'running' || status === 'waiting_for_input'
    ? 'streaming'
    : 'complete'

const optionStringOrUndefined = (
  value: Option.Option<string>,
): string | undefined => Option.getOrUndefined(value)

const optionStringOrNull = (value: Option.Option<string>): string | null =>
  Option.getOrNull(value)

const compactValue = (value: unknown, max = 220): string | undefined => {
  const text = textFromUnknown(value)
  if (text !== undefined) {
    return compactLine(text, max)
  }

  if (value === undefined || value === null) {
    return undefined
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value)
  }

  try {
    return compactLine(JSON.stringify(value), max)
  } catch {
    return undefined
  }
}

const firstText = (
  value: Record<string, unknown> | undefined,
  paths: ReadonlyArray<ReadonlyArray<string>>,
): string | undefined =>
  paths
    .map(path => textFromUnknown(nestedUnknown(value, path)))
    .find(text => text !== undefined)

const failureResponseBodyText = (
  raw: Record<string, unknown> | undefined,
  payload: Record<string, unknown> | undefined,
): string | undefined => {
  const responseBody = firstText(raw, [
    ['error', 'data', 'responseBody'],
    ['data', 'error', 'data', 'responseBody'],
  ])

  const parsed = parseJsonRecord(responseBody)
  const message =
    firstText(parsed, [['error', 'message'], ['message']]) ??
    firstText(raw, [
      ['error', 'data', 'message'],
      ['data', 'error', 'data', 'message'],
    ]) ??
    firstText(payload, [
      ['error', 'data', 'message'],
      ['data', 'error', 'data', 'message'],
    ])
  const code =
    firstText(parsed, [['error', 'code'], ['code']]) ??
    firstText(raw, [
      ['error', 'data', 'code'],
      ['data', 'error', 'data', 'code'],
    ])

  if (message === undefined) {
    return undefined
  }

  return code === undefined || message.includes(code)
    ? message
    : `${message} (${code})`
}

const compactLine = (value: string, max = 220): string => {
  const line = value.replace(/\s+/g, ' ').trim()

  return line.length > max ? `${line.slice(0, max - 1)}...` : line
}

const transcriptText = (value: string, max = 4_000): string => {
  const text = value
    .replace(/\r\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim()

  return text.length > max ? `${text.slice(0, max - 1)}...` : text
}

const internalAssistantTranscriptPhrases = [
  'closeout receipt',
  'closeout manifest',
  'completion artifact',
  'completion artifacts',
  'github-writeback.json',
  'local artifact',
  'local artifacts',
  'record the requested summary',
  'result.md',
  'run artifact',
  'run artifacts',
  'run outcome',
  'usage receipt',
  'workspace removed',
]

const isVisibleAssistantTranscriptText = (text: string): boolean => {
  const compact = compactLine(text, 1_000)
  const normalized = compact.toLowerCase()
  const generic = [
    'Assistant message completed.',
    'Codex one-shot run completed.',
    'Codex one-shot turn completed.',
    'Codex run resource usage receipt emitted.',
    'Codex workspace removed.',
    'Closeout receipt emitted.',
    'OpenCode run completed and closeout manifest submitted.',
    'OpenCode/Codex one-shot run completed.',
    'OpenCode/Codex one-shot turn completed.',
    'OpenCode/Codex run finished with status completed.',
    'stdout JSON event captured.',
  ]

  if (generic.includes(compact)) {
    return false
  }

  if (
    internalAssistantTranscriptPhrases.some(phrase =>
      normalized.includes(phrase),
    )
  ) {
    return false
  }

  if (
    normalized.includes('artifact') &&
    (normalized.includes('required') ||
      normalized.includes('prepare') ||
      normalized.includes('write') ||
      normalized.includes('adding') ||
      normalized.includes('record'))
  ) {
    return false
  }

  return true
}

const splitTranscriptText = (value: string): ReadonlyArray<string> =>
  transcriptText(value)
    .split(/\n{2,}/)
    .map(line => line.trim())
    .filter(line => line !== '')

const providerAuthFailureText = (event: ChatRunEvent): string | undefined => {
  const haystack = [
    event.type,
    event.summary,
    optionStringOrUndefined(event.status) ?? '',
    optionStringOrUndefined(event.payloadJson) ?? '',
  ]
    .join('\n')
    .toLowerCase()

  const isTokenInvalidated =
    haystack.includes('token_invalidated') ||
    haystack.includes('authentication token has been invalidated')
  const isOpenAiAuthError =
    isTokenInvalidated || haystack.includes('x-openai-authorization-error')

  if (!isOpenAiAuthError) {
    return undefined
  }

  return 'ChatGPT is not connected. Reconnect ChatGPT in Settings before launching Autopilot.'
}

const isChatGptConnectionLaunchError = (error: string): boolean => {
  const normalized = error.toLowerCase()

  return (
    normalized.includes('chatgpt') ||
    normalized.includes('openai') ||
    normalized.includes('token_invalidated') ||
    normalized.includes('requires_reauth')
  )
}

const providerLaunchErrorText = (error: string): string => {
  const normalized = error.toLowerCase()

  if (normalized.includes('no chatgpt')) {
    return 'ChatGPT is not connected. Connect ChatGPT in Settings before launching Autopilot.'
  }

  if (
    normalized.includes('token_invalidated') ||
    normalized.includes('authentication token has been invalidated') ||
    (normalized.includes('chatgpt') && normalized.includes('invalidated'))
  ) {
    return 'ChatGPT is not connected. Reconnect ChatGPT in Settings before launching Autopilot.'
  }

  if (normalized.includes('cannot launch autopilot')) {
    return compactLine(error, 320)
  }

  if (isChatGptConnectionLaunchError(error)) {
    return 'ChatGPT is not connected. Reconnect ChatGPT in Settings before launching Autopilot.'
  }

  return compactLine(error)
}

const providerLaunchActionHref = (error: string): string | undefined =>
  isChatGptConnectionLaunchError(error) ? '/settings/connections' : undefined

const providerReconnectPart = (error: string): RunTimelineReconnectPart => ({
  detail: [providerLaunchErrorText(error)],
  kind: 'tool',
  status: 'failed',
  subtitle: 'Reconnect required',
  title: 'ChatGPT not connected',
  actionHref: '/settings/connections',
  actionLabel: 'Reconnect ChatGPT',
})

const providerRecoveredPart = (error: string): RunTimelineToolPart => ({
  detail: [
    'ChatGPT is connected now. Send the message again to launch Autopilot.',
    providerLaunchErrorText(error),
  ],
  kind: 'tool',
  status: 'failed',
  subtitle: 'ready to retry',
  title: 'ChatGPT connected',
})

export const providerConnectionState = (
  model: Model,
): ProviderConnectionState => {
  const bundle = providerAccountBundleFromAuth(model.auth)

  return bundle.accounts.some(
    account =>
      account.provider === 'chatgpt_codex' &&
      account.health === 'healthy' &&
      account.publicStatus === 'connected' &&
      account.status === 'connected' &&
      account.hasSecretRef,
  )
    ? 'connected'
    : 'not_connected'
}

const providerLaunchConnectionPart = (
  error: string,
  connectionState: ProviderConnectionState,
): RunTimelineToolPart =>
  connectionState === 'connected'
    ? providerRecoveredPart(error)
    : providerReconnectPart(error)

const eventPayload = (
  event: ChatRunEvent,
): Record<string, unknown> | undefined =>
  parseJsonRecord(optionStringOrUndefined(event.payloadJson))

const rawRunnerPayload = (
  event: ChatRunEvent,
): Record<string, unknown> | undefined => {
  const payload = eventPayload(event)
  const dataJson = textFromUnknown(payload?.dataJson)
  const rawPayloadJson =
    textFromUnknown(payload?.rawPayloadJson) ??
    textFromUnknown(payload?.raw_payload_json)

  return (
    parseJsonRecord(dataJson) ??
    parseJsonRecord(rawPayloadJson) ??
    recordFromUnknown(payload)
  )
}

const rawEventType = (event: ChatRunEvent): string =>
  firstText(rawRunnerPayload(event), [['type'], ['event']]) ?? event.type

const openCodePart = (
  event: ChatRunEvent,
): Record<string, unknown> | undefined => {
  const raw = rawRunnerPayload(event)

  return (
    recordFromUnknown(nestedUnknown(raw, ['properties', 'part'])) ??
    recordFromUnknown(nestedUnknown(raw, ['part'])) ??
    recordFromUnknown(nestedUnknown(raw, ['item']))
  )
}

const openCodePartType = (event: ChatRunEvent): string | undefined =>
  textFromUnknown(openCodePart(event)?.type)

const openCodePartState = (
  event: ChatRunEvent,
): Record<string, unknown> | undefined =>
  recordFromUnknown(openCodePart(event)?.state)

const eventCallId = (event: ChatRunEvent): string | undefined => {
  const raw = rawRunnerPayload(event)
  const part = openCodePart(event)

  return (
    textFromUnknown(part?.callID) ??
    textFromUnknown(part?.callId) ??
    textFromUnknown(part?.call_id) ??
    firstText(raw, [
      ['callID'],
      ['callId'],
      ['call_id'],
      ['toolCallId'],
      ['toolCallID'],
      ['tool_call_id'],
      ['properties', 'callID'],
      ['properties', 'callId'],
      ['properties', 'call_id'],
      ['properties', 'toolCallId'],
      ['properties', 'toolCallID'],
      ['properties', 'tool_call_id'],
      ['properties', 'part', 'callID'],
      ['properties', 'part', 'callId'],
      ['properties', 'part', 'call_id'],
      ['properties', 'part', 'toolCallId'],
      ['properties', 'part', 'toolCallID'],
      ['properties', 'part', 'tool_call_id'],
      ['data', 'callID'],
      ['data', 'callId'],
      ['data', 'call_id'],
      ['item', 'callID'],
      ['item', 'callId'],
      ['item', 'call_id'],
      ['item', 'toolCallId'],
      ['item', 'toolCallID'],
      ['item', 'tool_call_id'],
    ])
  )
}

const eventPartId = (event: ChatRunEvent): string | undefined => {
  const part = openCodePart(event)
  const raw = rawRunnerPayload(event)

  return (
    textFromUnknown(part?.id) ??
    firstText(raw, [
      ['partID'],
      ['partId'],
      ['part_id'],
      ['properties', 'partID'],
      ['properties', 'partId'],
      ['properties', 'part_id'],
      ['properties', 'part', 'id'],
      ['item', 'id'],
    ])
  )
}

const openCodeToolName = (event: ChatRunEvent): string | undefined => {
  const part = openCodePart(event)
  const raw = rawRunnerPayload(event)

  return (
    textFromUnknown(part?.tool) ??
    textFromUnknown(part?.toolName) ??
    textFromUnknown(part?.name) ??
    firstText(raw, [
      ['properties', 'part', 'tool'],
      ['properties', 'part', 'toolName'],
      ['item', 'name'],
      ['item', 'tool_name'],
      ['name'],
      ['tool_name'],
      ['tool'],
    ])
  )
}

const assistantTextFromEvent = (event: ChatRunEvent): string | undefined => {
  if (event.type === 'artifact.created') {
    return undefined
  }

  const raw = rawRunnerPayload(event)
  const rawType = rawEventType(event)
  const part = openCodePart(event)
  if (
    (rawType === 'message.part.updated' ||
      rawType === 'text' ||
      event.type === 'message.part.updated') &&
    textFromUnknown(part?.type) === 'text'
  ) {
    const partText =
      textFromUnknown(part?.text) ?? textFromUnknown(part?.content)
    return partText === undefined || !isVisibleAssistantTranscriptText(partText)
      ? undefined
      : transcriptText(partText)
  }

  const text =
    firstText(raw, [
      ['text'],
      ['detail'],
      ['message'],
      ['content'],
      ['output'],
      ['properties', 'delta'],
      ['properties', 'part', 'text'],
      ['properties', 'part', 'content'],
      ['part', 'text'],
      ['part', 'content'],
      ['item', 'text'],
      ['item', 'message'],
      ['item', 'content', '0', 'text'],
      ['item', 'content', '0', 'content'],
      ['response', 'output_text'],
    ]) ?? (event.type === 'message.completed' ? event.summary : undefined)

  if (text === undefined) {
    return undefined
  }

  return isVisibleAssistantTranscriptText(text)
    ? transcriptText(text)
    : undefined
}

const reasoningTextFromEvent = (event: ChatRunEvent): string | undefined => {
  const rawType = rawEventType(event)
  const part = openCodePart(event)

  if (
    (rawType === 'message.part.updated' ||
      event.type === 'message.part.updated') &&
    textFromUnknown(part?.type) === 'reasoning'
  ) {
    return textFromUnknown(part?.text)
  }

  if (!event.type.includes('reasoning') && rawType !== 'message.part.delta') {
    return undefined
  }

  const raw = rawRunnerPayload(event)

  return firstText(raw, [
    ['text'],
    ['delta'],
    ['summary'],
    ['properties', 'delta'],
    ['properties', 'part', 'text'],
    ['item', 'summary', '0', 'text'],
  ])
}

const genericFailureSummaries: ReadonlyArray<string> = [
  'Codex reported a failure event.',
  'Codex VM workroom failed.',
  'OpenCode/Codex run finished with status failed.',
  'Runner event received.',
]

const firstFailureText = (
  event: ChatRunEvent,
  payload: Record<string, unknown> | undefined,
  raw: Record<string, unknown> | undefined,
): string | undefined => {
  const paths: ReadonlyArray<ReadonlyArray<string>> = [
    ['error'],
    ['error', 'message'],
    ['error', 'detail'],
    ['error', 'reason'],
    ['error', 'data', 'message'],
    ['error', 'data', 'detail'],
    ['error', 'data', 'reason'],
    ['failure'],
    ['failure', 'message'],
    ['reason'],
    ['message'],
    ['detail'],
    ['details'],
    ['stderr'],
    ['output'],
    ['properties', 'error'],
    ['properties', 'error', 'message'],
    ['properties', 'reason'],
    ['properties', 'message'],
    ['properties', 'stderr'],
    ['properties', 'part', 'state', 'error'],
    ['properties', 'part', 'state', 'stderr'],
    ['item', 'error'],
    ['item', 'error', 'message'],
    ['item', 'message'],
    ['data', 'error'],
    ['data', 'error', 'message'],
    ['data', 'error', 'data', 'message'],
    ['data', 'reason'],
  ]
  const detail =
    failureResponseBodyText(raw, payload) ??
    firstText(raw, paths) ??
    firstText(payload, paths)
  const summary =
    genericFailureSummaries.includes(event.summary.trim()) ||
    event.summary.trim() === ''
      ? undefined
      : event.summary

  return detail ?? summary
}

const failureDetail = (event: ChatRunEvent): ReadonlyArray<string> => {
  const payload = eventPayload(event)
  const raw = rawRunnerPayload(event)
  const errorText =
    providerAuthFailureText(event) ?? firstFailureText(event, payload, raw)
  const exitCode =
    compactValue(nestedUnknown(raw, ['exitCode']), 80) ??
    compactValue(nestedUnknown(raw, ['exit_code']), 80) ??
    compactValue(nestedUnknown(raw, ['error', 'data', 'statusCode']), 80) ??
    compactValue(nestedUnknown(raw, ['properties', 'exitCode']), 80) ??
    compactValue(nestedUnknown(raw, ['properties', 'exit_code']), 80) ??
    compactValue(nestedUnknown(raw, ['item', 'exit_code']), 80)
  const status =
    compactValue(nestedUnknown(raw, ['status']), 80) ??
    compactValue(nestedUnknown(raw, ['item', 'status']), 80) ??
    compactValue(nestedUnknown(payload, ['status']), 80)
  const lines = uniqueLines([
    errorText === undefined ? undefined : compactLine(errorText, 320),
    exitCode === undefined ? undefined : `exit: ${exitCode}`,
    status === undefined || status === optionStringOrUndefined(event.status)
      ? undefined
      : `status: ${status}`,
  ])

  return lines.length === 0
    ? [
        `No runner error detail was included in ${event.type} from ${event.source} #${event.sequence}.`,
      ]
    : lines
}

const failureHeadline = (event: ChatRunEvent): string =>
  failureDetail(event)[0] ?? `No runner error detail for ${event.type}.`

const lifecycleLabel = (event: ChatRunEvent): string | undefined => {
  if (event.type === 'agent_run.accepted') {
    return 'Assignment accepted'
  }

  if (event.type === 'runner.dispatched' || event.type === 'cloud.run.queued') {
    return 'Dispatched to computer'
  }

  if (event.type === 'repo.checkout.completed') {
    return 'Repository checkout completed'
  }

  if (event.type === 'repo.checkout.started') {
    return 'Repository checkout started'
  }

  if (event.type === 'run.started' || event.type === 'cloud.run.started') {
    return 'OpenCode started'
  }

  if (event.type === 'run.completed' || event.type === 'cloud.run.completed') {
    return 'OpenCode completed'
  }

  if (event.type.includes('failed')) {
    return `Failed: ${compactLine(failureHeadline(event), 120)}`
  }

  if (event.type.includes('timed_out')) {
    return 'Timed out'
  }

  return undefined
}

const uniqueLines = (
  values: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  values.reduce<ReadonlyArray<string>>(
    (lines, value) =>
      value === undefined || lines.includes(value) ? lines : [...lines, value],
    [],
  )

export const artifactNames = (chatRun: Extract<ChatRun, { _tag: 'Active' }>) =>
  uniqueLines(chatRun.events.flatMap(event => event.artifactRefs))

export const formatBytes = (bytes: number): string => {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }

  return `${bytes} B`
}

const runSummaryDetail = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
): ReadonlyArray<string> => {
  if (chatRun.metadata.status === 'failed') {
    const failed = chronologicalEvents(chatRun.events)
      .map(event =>
        event.type.includes('failed') ? failureHeadline(event) : undefined,
      )
      .find(detail => detail !== undefined)

    return failed === undefined ? [] : [`Failed: ${failed}`]
  }

  return chatRun.metadata.tokenTotal > 0
    ? [`tokens: ${chatRun.metadata.tokenTotal}`]
    : []
}

const displayToolTitle = (tool: string | undefined): string => {
  if (tool === undefined || tool === '') {
    return 'Tool call'
  }

  const normalized = tool.toLowerCase()
  if (normalized === 'bash') {
    return 'Shell command'
  }

  if (normalized === 'apply_patch') {
    return 'Patch applied'
  }

  return `${tool} tool`
}

const toolTitle = (event: ChatRunEvent): string | undefined => {
  const rawType = rawEventType(event)
  const partType = openCodePartType(event)
  const tool = openCodeToolName(event)

  if (
    (rawType === 'message.part.updated' || rawType === 'tool_use') &&
    partType === 'tool'
  ) {
    return displayToolTitle(tool)
  }

  if (rawType === 'session.next.shell.started') {
    return 'Shell command'
  }

  if (rawType === 'session.next.shell.ended') {
    return 'Shell command'
  }

  if (rawType === 'item.started' && partType?.includes('function_call')) {
    return 'Tool call'
  }

  if (rawType === 'item.completed' && partType?.includes('function_call')) {
    return 'Tool call'
  }

  if (rawType === 'item.started' && partType === 'command_execution') {
    return 'Shell command'
  }

  if (rawType === 'item.completed' && partType === 'command_execution') {
    return 'Shell command'
  }

  if (
    (rawType === 'item.started' || rawType === 'item.completed') &&
    partType === 'file_change'
  ) {
    return 'File change'
  }

  if (event.type === 'shell.command.started') {
    return 'Shell command'
  }

  if (event.type === 'shell.command.completed') {
    return 'Shell command'
  }

  if (event.type === 'tool.call.started') {
    return 'Tool call'
  }

  if (event.type === 'tool.call.completed') {
    return 'Tool call'
  }

  if (event.type === 'file.edit') {
    return 'File edit'
  }

  if (event.type === 'artifact.created') {
    return 'Artifact captured'
  }

  return undefined
}

const toolSubtitle = (event: ChatRunEvent): string => {
  const rawType = rawEventType(event)
  const partType = openCodePartType(event)
  const tool = openCodeToolName(event)

  if (event.type === 'artifact.created') {
    return 'OpenCode artifact'
  }

  if (
    rawType === 'session.next.shell.started' ||
    rawType === 'session.next.shell.ended' ||
    partType === 'command_execution' ||
    event.type.includes('shell.command')
  ) {
    return 'OpenCode shell'
  }

  if (partType === 'file_change' || event.type === 'file.edit') {
    return 'OpenCode file'
  }

  if (tool !== undefined) {
    return `OpenCode ${tool}`
  }

  return 'OpenCode tool'
}

const toolCommand = (event: ChatRunEvent): string | undefined => {
  const raw = rawRunnerPayload(event)

  return firstText(raw, [
    ['cmd'],
    ['command'],
    ['part', 'state', 'input', 'command'],
    ['part', 'state', 'input', 'cmd'],
    ['properties', 'command'],
    ['properties', 'part', 'state', 'input', 'command'],
    ['properties', 'part', 'state', 'input', 'cmd'],
    ['arguments', 'cmd'],
    ['input', 'cmd'],
    ['item', 'command'],
  ])
}

const artifactFileName = (event: ChatRunEvent): string | undefined => {
  const raw = rawRunnerPayload(event)

  return (
    firstText(raw, [
      ['detail'],
      ['file'],
      ['path'],
      ['artifact'],
      ['artifactPath'],
    ]) ?? event.artifactRefs[0]
  )
}

const changedFilesFromPatchText = (
  value: string | undefined,
): ReadonlyArray<string> =>
  value === undefined
    ? []
    : uniqueLines([
        ...Array.from(
          value.matchAll(/^\*\*\* (?:Add|Update|Delete) File: (.+)$/gm),
        ).map(match => match[1]?.trim()),
        ...Array.from(value.matchAll(/\b(?:A|M|D)\s+([^\s]+)/g)).map(match =>
          match[1]?.trim(),
        ),
      ])

const eventToolStatus = (
  event: ChatRunEvent,
  runStatus: string | null,
): RunTimelineToolPart['status'] => {
  const rawType = rawEventType(event)
  const itemStatus =
    firstText(rawRunnerPayload(event), [
      ['item', 'status'],
      ['properties', 'part', 'state', 'status'],
    ]) ?? textFromUnknown(openCodePartState(event)?.status)

  if (
    itemStatus === 'completed' ||
    itemStatus === 'done' ||
    rawType.endsWith('.completed') ||
    rawType.endsWith('.ended') ||
    event.type.endsWith('.completed')
  ) {
    return 'completed'
  }

  if (
    itemStatus === 'failed' ||
    itemStatus === 'error' ||
    event.type.endsWith('.failed')
  ) {
    return 'failed'
  }

  if (
    itemStatus === 'in_progress' ||
    itemStatus === 'running' ||
    rawType.endsWith('.started') ||
    event.type.endsWith('.started')
  ) {
    return 'running'
  }

  return runStatusToToolStatus(runStatus)
}

const toolDetail = (event: ChatRunEvent): ReadonlyArray<string> => {
  const raw = rawRunnerPayload(event)
  const part = openCodePart(event)
  const state = openCodePartState(event)
  const command = toolCommand(event)
  const tool = openCodeToolName(event)
  const normalizedTool = tool?.toLowerCase()
  const artifact = artifactFileName(event)
  if (event.type === 'artifact.created') {
    return artifact === undefined ? [] : [`file: ${artifact}`]
  }

  const input =
    command !== undefined || normalizedTool === 'apply_patch'
      ? undefined
      : (compactValue(state?.input, 180) ??
        compactValue(part?.input, 180) ??
        compactValue(nestedUnknown(raw, ['item', 'arguments']), 180))
  const output =
    firstText(raw, [
      ['output'],
      ['content'],
      ['result'],
      ['part', 'state', 'output'],
      ['part', 'state', 'error'],
      ['properties', 'output'],
      ['properties', 'part', 'state', 'output'],
      ['properties', 'part', 'state', 'error'],
      ['item', 'aggregated_output'],
    ]) ?? textFromUnknown(state?.output)
  const changedPath =
    firstText(raw, [
      ['item', 'changes', '0', 'path'],
      ['properties', 'part', 'changes', '0', 'path'],
    ]) ?? compactValue(nestedUnknown(raw, ['item', 'changes']), 180)
  const files = event.artifactRefs.length === 0 ? [] : event.artifactRefs
  const patchFiles =
    normalizedTool === 'apply_patch'
      ? changedFilesFromPatchText(
          output ??
            textFromUnknown(state?.input) ??
            textFromUnknown(part?.input) ??
            compactValue(state?.input, 2_000),
        )
      : []
  const lines = [
    command === undefined ? undefined : `$ ${compactLine(command)}`,
    tool === undefined ? undefined : `tool: ${tool}`,
    input === undefined ? undefined : `input: ${input}`,
    output === undefined || normalizedTool === 'apply_patch'
      ? undefined
      : transcriptText(output, 1_200),
    event.type.includes('failed')
      ? compactLine(failureHeadline(event), 220)
      : undefined,
    changedPath === undefined ? undefined : `file: ${changedPath}`,
    ...patchFiles.map(file => `file: ${file}`),
    ...files.map(file => `artifact: ${file}`),
    command === undefined &&
    tool === undefined &&
    input === undefined &&
    output === undefined &&
    changedPath === undefined
      ? event.summary
      : undefined,
  ]

  return uniqueLines(lines).slice(0, 4)
}

const toolMergeKey = (event: ChatRunEvent): string | undefined => {
  const title = toolTitle(event)
  if (title === undefined) {
    return undefined
  }

  const callId = eventCallId(event)
  if (callId !== undefined) {
    return `call:${callId}`
  }

  const partId = eventPartId(event)
  if (partId !== undefined) {
    return `part:${partId}`
  }

  const command = toolCommand(event)
  if (command !== undefined) {
    return `command:${command}`
  }

  const tool = openCodeToolName(event)
  const input =
    compactValue(openCodePartState(event)?.input, 180) ??
    compactValue(openCodePart(event)?.input, 180)

  return tool === undefined || input === undefined
    ? undefined
    : `tool:${tool}:${input}`
}

const textMergeKey = (event: ChatRunEvent): string | undefined => {
  const rawType = rawEventType(event)
  const partType = openCodePartType(event)

  if (
    rawType !== 'message.part.updated' &&
    rawType !== 'message.part.delta' &&
    rawType !== 'text' &&
    event.type !== 'message.part.updated' &&
    event.type !== 'message.part.delta'
  ) {
    return undefined
  }

  if (partType !== 'text' && partType !== 'reasoning') {
    return undefined
  }

  const partId = eventPartId(event)

  return partId === undefined ? undefined : `${partType}:${partId}`
}

const statusRank = (status: RunTimelineToolPart['status']): number => {
  if (status === 'failed') {
    return 4
  }

  if (status === 'completed') {
    return 3
  }

  if (status === 'running') {
    return 2
  }

  return 1
}

const mergeToolParts = (
  previous: RunTimelineToolPart,
  next: RunTimelineToolPart,
): RunTimelineToolPart => ({
  kind: 'tool',
  title: previous.title,
  subtitle: previous.subtitle,
  status:
    statusRank(next.status) >= statusRank(previous.status)
      ? next.status
      : previous.status,
  detail: uniqueLines([...previous.detail, ...next.detail]).slice(0, 6),
})

const shouldMergeToolParts = (
  previous: RunTimelineToolPart,
  next: RunTimelineToolPart,
): boolean =>
  previous.status === 'queued' ||
  previous.status === 'running' ||
  next.status === 'completed' ||
  next.status === 'failed'

const eventTimelinePart = (
  event: ChatRunEvent,
  runStatus: string,
): RunTimelinePart | undefined => {
  if (hiddenTranscriptEvent(event)) {
    return undefined
  }

  const reasoning = reasoningTextFromEvent(event)
  if (reasoning !== undefined) {
    return {
      kind: 'tool',
      title: 'Thinking',
      subtitle: 'reasoning',
      status: runStatusToToolStatus(runStatus),
      detail: [compactLine(reasoning, 360)],
    }
  }

  const assistantText = assistantTextFromEvent(event)
  if (assistantText !== undefined) {
    return {
      kind: 'text',
      body: splitTranscriptText(assistantText),
    }
  }

  const title = toolTitle(event)
  if (title !== undefined) {
    return {
      kind: 'tool',
      title,
      subtitle: toolSubtitle(event),
      status: eventToolStatus(event, optionStringOrNull(event.status)),
      detail: toolDetail(event),
    }
  }

  return undefined
}

const hiddenTranscriptEvent = (event: ChatRunEvent): boolean => {
  if (event.type.includes('failed')) {
    return false
  }

  return [
    'agent_run.accepted',
    'artifact_set.completed',
    'cloud.run.completed',
    'cloud.run.queued',
    'cloud.run.started',
    'receipt.created',
    'repo.checkout.completed',
    'repo.checkout.started',
    'resource.usage.captured',
    'run.completed',
    'run.heartbeat',
    'run.queued',
    'run.started',
    'runner.artifact',
    'runner.auth_grant_resolved',
    'runner.cleanup',
    'runner.completed',
    'runner.dispatched',
    'runner.github_write_grant_resolved',
    'runner.log',
    'runner.receipt',
    'turn.started',
    'usage.unavailable',
  ].includes(event.type)
}

const partSignature = (part: RunTimelinePart): string => {
  if (part.kind === 'text') {
    return `text:${part.body.join('\n')}`
  }

  if (part.kind === 'tool') {
    return `tool:${part.title}:${part.status}:${part.detail.join('\n')}`
  }

  if (part.kind === 'file') {
    return `file:${part.path}:${part.excerpt.join('\n')}`
  }

  return `diff:${part.files.map(file => file.path).join(',')}`
}

type ChronologicalPartsAccumulator = Readonly<{
  seen: Set<string>
  parts: Array<RunTimelinePart>
  textIndexByKey: Map<string, number>
  toolIndexByKey: Map<string, number>
}>

const appendChronologicalEventPart =
  (runStatus: string) =>
  (
    accumulator: ChronologicalPartsAccumulator,
    event: ChatRunEvent,
  ): ChronologicalPartsAccumulator => {
    const part = eventTimelinePart(event, runStatus)
    if (part === undefined) {
      return accumulator
    }

    if (part.kind === 'text') {
      const key = textMergeKey(event)
      const index =
        key === undefined ? undefined : accumulator.textIndexByKey.get(key)
      if (index !== undefined) {
        accumulator.parts[index] = part
        return accumulator
      }

      const signature = partSignature(part)
      if (accumulator.seen.has(signature)) {
        return accumulator
      }

      const nextIndex = accumulator.parts.length
      accumulator.parts.push(part)
      accumulator.seen.add(signature)
      if (key !== undefined) {
        accumulator.textIndexByKey.set(key, nextIndex)
      }

      return accumulator
    }

    if (part.kind === 'tool') {
      const key = toolMergeKey(event)
      const index =
        key === undefined ? undefined : accumulator.toolIndexByKey.get(key)
      const previous =
        index === undefined ? undefined : accumulator.parts[index]

      if (
        index !== undefined &&
        previous !== undefined &&
        previous.kind === 'tool' &&
        shouldMergeToolParts(previous, part)
      ) {
        accumulator.parts[index] = mergeToolParts(previous, part)
        return accumulator
      }

      const signature = partSignature(part)
      if (accumulator.seen.has(signature)) {
        return accumulator
      }

      const nextIndex = accumulator.parts.length
      accumulator.parts.push(part)
      accumulator.seen.add(signature)
      if (key !== undefined) {
        accumulator.toolIndexByKey.set(key, nextIndex)
      }

      return accumulator
    }

    const signature = partSignature(part)
    if (accumulator.seen.has(signature)) {
      return accumulator
    }

    accumulator.parts.push(part)
    accumulator.seen.add(signature)
    return accumulator
  }

const chronologicalEvents = (
  events: ReadonlyArray<ChatRunEvent>,
): ReadonlyArray<ChatRunEvent> =>
  [...events].sort((left, right) =>
    left.sequence === right.sequence
      ? Date.parse(left.createdAt) - Date.parse(right.createdAt)
      : left.sequence - right.sequence,
  )

const providerAuthFailureForActiveRun = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
): string | undefined =>
  chronologicalEvents(chatRun.events)
    .map(event => providerAuthFailureText(event))
    .find(error => error !== undefined)

const providerReconnectPartFromActiveRun = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
  connectionState: ProviderConnectionState,
): RunTimelineToolPart | undefined => {
  const error = providerAuthFailureForActiveRun(chatRun)

  return error === undefined
    ? undefined
    : providerLaunchConnectionPart(error, connectionState)
}

const chronologicalEventParts = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
): ReadonlyArray<RunTimelinePart> =>
  chronologicalEvents(chatRun.events).reduce(
    appendChronologicalEventPart(chatRun.metadata.status),
    {
      parts: [],
      seen: new Set<string>(),
      textIndexByKey: new Map<string, number>(),
      toolIndexByKey: new Map<string, number>(),
    },
  ).parts

const activeRunForTeamMessage = (
  message: TeamChatMessageRecord,
  chatRun: ChatRun,
): Extract<ChatRun, { _tag: 'Active' }> | undefined =>
  chatRun._tag === 'Active' &&
  chatRun.metadata.runId === teamMessageAgentRunId(message)
    ? chatRun
    : undefined

const teamMessageAgentRunId = (
  message: TeamChatMessageRecord,
): string | undefined => {
  const ref = agentRunExternalRefFromNullable(message.agentRunId)

  return ref._tag === 'AgentRunExternalRefPresent' ? ref.value : undefined
}

const formatDuration = (seconds: number): string => {
  if (seconds < 60) {
    return `${seconds}s`
  }

  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60

  return remainingSeconds === 0
    ? `${minutes}m`
    : `${minutes}m ${remainingSeconds}s`
}

const durationSecondsFromMetadata = (
  metadata: Extract<ChatRun, { _tag: 'Active' }>['metadata'],
): number | undefined => {
  const startedAt = Date.parse(metadata.createdAt)
  const endedAt = Date.parse(metadata.updatedAt)

  return Number.isFinite(startedAt) && Number.isFinite(endedAt)
    ? Math.max(0, Math.round((endedAt - startedAt) / 1000))
    : undefined
}

const pluralizeCount = (count: number, label: string): string =>
  `${count} ${label}${count === 1 ? '' : 's'}`

const eventLooksLikeToolCall = (event: ChatRunEvent): boolean =>
  event.type.includes('tool') ||
  rawEventType(event) === 'tool_use' ||
  rawEventType(event) === 'tool_result' ||
  textFromUnknown(openCodePart(event)?.tool) !== undefined

const teamRunSnapshotSummaryLines = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
): ReadonlyArray<string> => {
  const duration = durationSecondsFromMetadata(chatRun.metadata)
  const status =
    chatRun.metadata.status === 'completed'
      ? 'Succeeded'
      : chatRun.metadata.status === 'failed'
        ? 'Failed'
        : chatRun.metadata.status
  const stats = [
    duration === undefined
      ? status
      : `${status} in ${formatDuration(duration)}`,
    pluralizeCount(chatRun.metadata.eventCursor, 'event'),
    pluralizeCount(
      chronologicalEvents(chatRun.events).filter(eventLooksLikeToolCall).length,
      'tool call',
    ),
    ...(chatRun.metadata.tokenTotal === 0
      ? []
      : [pluralizeCount(chatRun.metadata.tokenTotal, 'token')]),
  ]

  return uniqueLines(stats)
}

const teamRunPersistedSummaryLines = (
  summary: TeamChatRunSummary,
): ReadonlyArray<string> => {
  const status =
    summary.status === 'completed'
      ? 'Succeeded'
      : summary.status === 'failed'
        ? 'Failed'
        : summary.status
  const duration = runDurationFromNullable(summary.durationSeconds)
  const stats = [
    duration._tag === 'RunDurationUnknown'
      ? status
      : `${status} in ${formatDuration(duration.seconds)}`,
    pluralizeCount(summary.eventCount, 'event'),
    pluralizeCount(summary.toolCallCount, 'tool call'),
    ...(summary.tokenTotal === 0
      ? []
      : [pluralizeCount(summary.tokenTotal, 'token')]),
  ]

  return uniqueLines(stats)
}

export const teamAutopilotRunCardParts = (
  message: TeamChatMessageRecord,
  chatRun: ChatRun,
  connectionState: ProviderConnectionState = 'not_connected',
): ReadonlyArray<RunTimelinePart> => {
  const activeRun = activeRunForTeamMessage(message, chatRun)
  const summary = message.runSummary
  const activeProviderReconnectPart =
    activeRun === undefined
      ? undefined
      : providerReconnectPartFromActiveRun(activeRun, connectionState)
  const status = activeRun?.metadata.status ?? summary?.status ?? 'queued'
  const detail =
    activeRun === undefined
      ? summary === undefined
        ? ['Waiting for run details.']
        : teamRunPersistedSummaryLines(summary)
      : teamRunSnapshotSummaryLines(activeRun)
  const runtime = activeRun?.metadata.runtime ?? summary?.runtime ?? 'Autopilot'
  const backend = activeRun?.metadata.backend ?? summary?.backend ?? 'runner'

  if (message.launchError !== undefined) {
    return isChatGptConnectionLaunchError(message.launchError)
      ? [providerLaunchConnectionPart(message.launchError, connectionState)]
      : [
          {
            detail: [providerLaunchErrorText(message.launchError)],
            kind: 'tool',
            status: 'failed',
            subtitle: 'launch failed',
            title: 'Autopilot launch failed',
          },
        ]
  }

  if (activeProviderReconnectPart !== undefined) {
    return [activeProviderReconnectPart]
  }

  const agentRunId = teamMessageAgentRunId(message)

  if (agentRunId === undefined) {
    return [
      {
        detail: [
          'Autopilot did not create a child run for this team message. Try again after reconnecting the required account.',
        ],
        kind: 'tool',
        status: 'failed',
        subtitle: 'no run created',
        title: 'Autopilot launch unavailable',
      },
    ]
  }

  return [
    {
      detail,
      href: threadRouter({ threadId: agentRunId }),
      kind: 'tool',
      status: runStatusToToolStatus(status),
      subtitle: `${runtime} on ${backend}`,
      title: 'Autopilot run',
    },
  ]
}

const summaryEventParts = (
  chatRun: Extract<ChatRun, { _tag: 'Active' }>,
): ReadonlyArray<RunTimelineSummaryPart> => {
  const lifecycle = uniqueLines(
    chronologicalEvents(chatRun.events).map(lifecycleLabel),
  )

  return [
    {
      kind: 'tool',
      title: `Autopilot ${chatRun.metadata.status}`,
      subtitle: 'OpenCode on computer',
      status: runStatusToToolStatus(chatRun.metadata.status),
      detail: runSummaryDetail(chatRun),
    },
    ...(lifecycle.length === 0
      ? []
      : [
          {
            kind: 'tool' as const,
            title: 'Computer workroom',
            subtitle: `${lifecycle.length} stages`,
            status: runStatusToToolStatus(chatRun.metadata.status),
            detail: lifecycle.slice(-6),
          },
        ]),
  ]
}

export const chatRunTimelineMessages = (
  chatRun: ChatRun,
  connectionState: ProviderConnectionState = 'not_connected',
): ReadonlyArray<RunTimelineMessage> => {
  if (chatRun._tag === 'Idle') {
    return []
  }

  if (chatRun._tag === 'Launching') {
    return [
      {
        id: `launching-${chatRun.requestId}`,
        author: 'assistant',
        label: 'Autopilot',
        time: 'launching',
        status: 'streaming',
        parts: [
          {
            kind: 'tool',
            title: 'Launch Autopilot run',
            subtitle: 'OpenAgents -> computer',
            status: 'running',
            detail: [
              'Assignment accepted by the browser client.',
              'Waiting for computer callbacks.',
            ],
          },
        ],
      },
    ]
  }

  if (chatRun._tag === 'Loading') {
    return [
      {
        id: `loading-${chatRun.runId}`,
        author: 'assistant',
        label: 'Autopilot',
        time: 'loading',
        status: 'streaming',
        parts: [
          {
            kind: 'tool',
            title: 'loading',
            subtitle: 'saved transcript',
            status: 'running',
            detail: [],
          },
        ],
      },
    ]
  }

  if (chatRun._tag === 'Failed') {
    const actionHref = providerLaunchActionHref(chatRun.error)
    const reconnectPart = isChatGptConnectionLaunchError(chatRun.error)
      ? providerLaunchConnectionPart(chatRun.error, connectionState)
      : undefined

    return [
      {
        id: 'autopilot-run-failed',
        author: 'assistant',
        label: 'Autopilot',
        time: 'failed',
        parts: [
          reconnectPart ?? {
            kind: 'tool',
            title: 'Autopilot run failed',
            subtitle: 'Worker / computer dispatch',
            status: 'failed',
            detail: [providerLaunchErrorText(chatRun.error)],
            ...(actionHref === undefined
              ? {}
              : { actionHref, actionLabel: 'Reconnect ChatGPT' }),
          },
        ],
      },
    ]
  }

  const activeProviderReconnectPart = providerReconnectPartFromActiveRun(
    chatRun,
    connectionState,
  )
  if (activeProviderReconnectPart !== undefined) {
    return [
      {
        id: `${chatRun.metadata.runId}-provider-reconnect`,
        author: 'assistant',
        label: 'Autopilot',
        time: 'failed',
        status: 'complete',
        parts: [activeProviderReconnectPart],
      },
    ]
  }

  const parts = [
    ...summaryEventParts(chatRun),
    ...chronologicalEventParts(chatRun),
  ]

  return [
    {
      id: `${chatRun.metadata.runId}-transcript`,
      author: 'assistant',
      label: 'Autopilot',
      time: chatRun.metadata.status,
      status: runStatusToTimelineStatus(chatRun.metadata.status),
      parts,
    },
  ]
}
