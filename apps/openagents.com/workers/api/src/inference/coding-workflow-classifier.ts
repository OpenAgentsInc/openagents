export type CodingWorkflowClass =
  | 'cloud_coding_session'
  | 'codex_agent_task'
  | 'none'

export type CodingWorkflowClassification = Readonly<{
  confidence: number
  evidenceRefs: ReadonlyArray<string>
  workflowClass: CodingWorkflowClass
}>

type ChatMessageLike = Readonly<{
  content?: unknown
  name?: unknown
  role?: unknown
}>

type CodingWorkflowClassificationInput = Readonly<{
  headers?: Headers | undefined
  messages: ReadonlyArray<ChatMessageLike>
  rawBody?: unknown
}>

const explicitWorkflowClasses = new Set<CodingWorkflowClass>([
  'cloud_coding_session',
  'codex_agent_task',
])

const workflowClassFromText = (value: unknown): CodingWorkflowClass => {
  if (typeof value !== 'string') {
    return 'none'
  }

  const normalized = value.trim()
  return explicitWorkflowClasses.has(normalized as CodingWorkflowClass)
    ? (normalized as CodingWorkflowClass)
    : 'none'
}

const recordValue = (value: unknown, key: string): unknown =>
  value !== null && typeof value === 'object'
    ? (value as Record<string, unknown>)[key]
    : undefined

const firstKnownWorkflowClass = (
  values: ReadonlyArray<CodingWorkflowClass>,
): CodingWorkflowClass => values.find(value => value !== 'none') ?? 'none'

const bodyWorkflowClass = (value: unknown): CodingWorkflowClass => {
  const direct = firstKnownWorkflowClass([
    workflowClassFromText(recordValue(value, 'workflowClass')),
    workflowClassFromText(recordValue(value, 'workflow_class')),
  ])
  if (direct !== 'none') {
    return direct
  }

  const openagents = recordValue(value, 'openagents')
  return firstKnownWorkflowClass([
    workflowClassFromText(recordValue(openagents, 'workflowClass')),
    workflowClassFromText(recordValue(openagents, 'workflow_class')),
  ])
}

const messageWorkflowClass = (
  messages: ReadonlyArray<ChatMessageLike>,
): CodingWorkflowClass => {
  for (const message of messages) {
    const role = typeof message.role === 'string' ? message.role : ''
    const name = typeof message.name === 'string' ? message.name : ''
    const content =
      message.content !== null && typeof message.content === 'object'
        ? (message.content as Record<string, unknown>)
        : undefined
    const classFromMessage = firstKnownWorkflowClass([
      workflowClassFromText(recordValue(content, 'workflowClass')),
      workflowClassFromText(recordValue(content, 'workflow_class')),
    ])

    if (classFromMessage !== 'none') {
      return classFromMessage
    }

    if (
      role === 'tool' &&
      (name === 'openagents.coding_workflow' ||
        name === 'coding_workflow_classifier')
    ) {
      const toolClass =
        typeof message.content === 'string'
          ? workflowClassFromText(message.content)
          : classFromMessage
      if (toolClass !== 'none') {
        return toolClass
      }
    }
  }

  return 'none'
}

export const classifyCodingWorkflow = (
  input: CodingWorkflowClassificationInput,
): CodingWorkflowClassification => {
  const headerClass = workflowClassFromText(
    input.headers?.get('x-openagents-workflow-class'),
  )
  if (headerClass !== 'none') {
    return {
      confidence: 1,
      evidenceRefs: ['evidence.coding_workflow.explicit_header'],
      workflowClass: headerClass,
    }
  }

  const bodyClass = bodyWorkflowClass(input.rawBody)
  if (bodyClass !== 'none') {
    return {
      confidence: 1,
      evidenceRefs: ['evidence.coding_workflow.structured_body'],
      workflowClass: bodyClass,
    }
  }

  const messageClass = messageWorkflowClass(input.messages)
  if (messageClass !== 'none') {
    return {
      confidence: 1,
      evidenceRefs: ['evidence.coding_workflow.structured_message'],
      workflowClass: messageClass,
    }
  }

  return {
    confidence: 0,
    evidenceRefs: ['evidence.coding_workflow.none'],
    workflowClass: 'none',
  }
}
