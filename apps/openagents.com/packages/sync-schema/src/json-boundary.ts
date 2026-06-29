import { Option, Schema as S } from 'effect'

const JsonRecord = S.Record(S.String, S.Unknown)

const decodeJsonRecord = S.decodeUnknownOption(JsonRecord)

export const recordFromUnknown = (
  value: unknown,
): Record<string, unknown> | undefined =>
  Option.getOrUndefined(decodeJsonRecord(value))

export const isRecord = (value: unknown): value is Record<string, unknown> =>
  recordFromUnknown(value) !== undefined

export const nestedUnknown = (
  value: Record<string, unknown> | undefined,
  path: ReadonlyArray<string>,
): unknown =>
  path.reduce<unknown>(
    (current, part) => (isRecord(current) ? current[part] : undefined),
    value,
  )

export const parseJsonValue = (
  text: string | null | undefined,
): unknown | undefined => {
  if (text === null || text === undefined || text.trim() === '') {
    return undefined
  }

  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

export const parseEmbeddedJsonRecord = (
  text: string,
): Record<string, unknown> | undefined => {
  const trimmed = text.trim()
  const withoutStreamPrefix = trimmed.replace(/^(stdout|stderr):\s*/, '')
  const jsonStart = withoutStreamPrefix.indexOf('{')

  if (jsonStart < 0) {
    return undefined
  }

  return recordFromUnknown(parseJsonValue(withoutStreamPrefix.slice(jsonStart)))
}
