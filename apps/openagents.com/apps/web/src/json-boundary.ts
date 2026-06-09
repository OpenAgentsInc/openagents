import { Option, Schema as S } from 'effect'

const JsonRecord = S.Record(S.String, S.Unknown)
const UnknownArray = S.Array(S.Unknown)

const decodeJsonRecord = S.decodeUnknownOption(JsonRecord)
const decodeUnknownArray = S.decodeUnknownOption(UnknownArray)

export const recordFromUnknown = (
  value: unknown,
): Record<string, unknown> | undefined =>
  Option.getOrUndefined(decodeJsonRecord(value))

export const arrayFromUnknown = (
  value: unknown,
): ReadonlyArray<unknown> | undefined =>
  Option.getOrUndefined(decodeUnknownArray(value))

export const textFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

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
    return recordFromUnknown(JSON.parse(value))
  } catch {
    return undefined
  }
}

export const nestedUnknown = (
  value: Record<string, unknown> | undefined,
  path: ReadonlyArray<string>,
): unknown =>
  path.reduce<unknown>((current, key) => {
    const array = arrayFromUnknown(current)
    if (array !== undefined && /^\d+$/.test(key)) {
      return array[Number(key)]
    }

    return recordFromUnknown(current)?.[key]
  }, value)
