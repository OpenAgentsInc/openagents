import { Option, Schema as S } from 'effect'
export {
  decodeRowEffect,
  decodeUnknownEffect,
  expectBoundaryFailure,
  OpenAgentsBoundaryError,
  parseJsonEffect,
  readJsonFileEffect,
  readRedactedConfigEffect,
  readRequestJsonEffect,
} from '@openagentsinc/effect-boundary'

const JsonRecord = S.Record(S.String, S.Unknown)
const UnknownArray = S.Array(S.Unknown)
const StringArray = S.Array(S.String)

const decodeJsonRecord = S.decodeUnknownOption(JsonRecord)
const decodeUnknownArray = S.decodeUnknownOption(UnknownArray)
const decodeStringArray = S.decodeUnknownOption(StringArray)

export const recordFromUnknown = (
  value: unknown,
): Record<string, unknown> | undefined =>
  Option.getOrUndefined(decodeJsonRecord(value))

export const arrayFromUnknown = (
  value: unknown,
): ReadonlyArray<unknown> | undefined =>
  Option.getOrUndefined(decodeUnknownArray(value))

export const isRecord = (value: unknown): value is Record<string, unknown> =>
  recordFromUnknown(value) !== undefined

export const stringArrayFromUnknown = (value: unknown): ReadonlyArray<string> =>
  Option.getOrElse(
    decodeStringArray(value),
    () =>
      arrayFromUnknown(value)?.filter(
        (item): item is string => typeof item === 'string',
      ) ?? [],
  )

export const nestedUnknown = (
  value: Record<string, unknown> | undefined,
  path: ReadonlyArray<string>,
): unknown =>
  path.reduce<unknown>((current, key) => {
    if (Array.isArray(current) && /^\d+$/.test(key)) {
      return current[Number(key)]
    }

    return isRecord(current) ? current[key] : undefined
  }, value)

export const optionalString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

export const optionalBoolean = (value: unknown): boolean | undefined =>
  typeof value === 'boolean' ? value : undefined

export const optionalInteger = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isInteger(value)) {
    return value
  }

  if (typeof value !== 'string' || value.trim() === '') {
    return undefined
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) ? parsed : undefined
}

export const optionalNestedString = (
  value: Record<string, unknown> | undefined,
  paths: ReadonlyArray<ReadonlyArray<string>>,
): string | undefined =>
  paths
    .map(path => optionalString(nestedUnknown(value, path)))
    .find(text => text !== undefined)

export const parseJsonUnknown = (value: string): unknown => JSON.parse(value)

export const parseJsonWithSchema = <A>(
  schema: S.Decoder<A>,
  value: string,
): A => S.decodeUnknownSync(schema)(parseJsonUnknown(value))

export const decodeUnknownWithSchema = <A>(
  schema: S.Decoder<A>,
  value: unknown,
): A => S.decodeUnknownSync(schema)(value)

export const parseJsonRecord = (
  value: string | null | undefined,
): Record<string, unknown> | undefined => {
  if (value === null || value === undefined || value.trim() === '') {
    return undefined
  }

  try {
    return recordFromUnknown(parseJsonUnknown(value))
  } catch {
    return undefined
  }
}

export const parseJsonStringArray = (
  value: string | null | undefined,
): ReadonlyArray<string> => {
  if (value === null || value === undefined || value.trim() === '') {
    return []
  }

  try {
    return stringArrayFromUnknown(parseJsonUnknown(value))
  } catch {
    return []
  }
}

export const parseBase64UrlJsonRecord = (
  value: string,
): Record<string, unknown> | undefined => {
  const paddedValue = value
    .replaceAll('-', '+')
    .replaceAll('_', '/')
    .padEnd(Math.ceil(value.length / 4) * 4, '=')

  try {
    return parseJsonRecord(atob(paddedValue))
  } catch {
    return undefined
  }
}

export const safeJsonRecord = (
  value: string | null | undefined,
): Record<string, unknown> | undefined => {
  return parseJsonRecord(value)
}

export const readJsonObject = async (
  request: Request,
): Promise<Record<string, unknown>> => {
  const text = await request.text()

  if (text.trim() === '') {
    return {}
  }

  return recordFromUnknown(parseJsonUnknown(text)) ?? {}
}

export const readRequestSelector = async (
  request: Request,
): Promise<Record<string, unknown>> => {
  const url = new URL(request.url)
  const query = Object.fromEntries(url.searchParams.entries())

  if (request.method === 'GET') {
    return query
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )

  return { ...query, ...body }
}
