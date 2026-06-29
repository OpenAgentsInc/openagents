export type NormalizedOmniRunnerEventPayload = Readonly<{
  artifactRefs: ReadonlyArray<string>
  externalEventId?: string
  payload: Record<string, unknown>
  sequence: number
  source: string
  status?: string
  summary: string
  type: string
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const optionalText = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const optionalTextFromAny = (...values: ReadonlyArray<unknown>): string | undefined =>
  values.map(optionalText).find((value): value is string => value !== undefined)

const stringArray = (value: unknown): ReadonlyArray<string> =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : []

const optionalSequence = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

export const normalizeOmniRunnerEventPayload = (
  payload: unknown,
  fallbackSequence: number,
): NormalizedOmniRunnerEventPayload | undefined => {
  if (!isRecord(payload)) {
    return undefined
  }

  const externalEventId = optionalTextFromAny(
    payload.externalEventId,
    payload.external_event_id,
  )
  const status = optionalText(payload.status)

  return {
    artifactRefs: [
      ...stringArray(payload.artifactRefs),
      ...stringArray(payload.artifact_refs),
    ],
    ...(externalEventId === undefined ? {} : { externalEventId }),
    payload,
    sequence: optionalSequence(payload.sequence) ?? fallbackSequence,
    source: optionalText(payload.source) ?? 'runner',
    ...(status === undefined ? {} : { status }),
    summary: optionalText(payload.summary) ?? 'Runner event received.',
    type: optionalText(payload.type) ?? 'runner.event',
  }
}
