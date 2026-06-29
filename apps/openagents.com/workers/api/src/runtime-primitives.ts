export const currentDate = (): Date => new Date()

export const currentIsoTimestamp = (): string => currentDate().toISOString()

export const currentEpochMillis = (): number => Date.now()

export const currentEpochSeconds = (): number =>
  Math.floor(currentEpochMillis() / 1000)

export const randomUuid = (): string => crypto.randomUUID()

export const compactRandomId = (prefix: string): string =>
  `${prefix}_${randomUuid().replaceAll('-', '')}`

export const dashedRandomId = (prefix: string): string =>
  `${prefix}_${randomUuid()}`

export const epochMillisToIsoTimestamp = (timestamp: number): string =>
  new Date(timestamp).toISOString()

export const isoTimestampToDate = (timestamp: string): Date =>
  new Date(timestamp)

export const isoTimestampAfterIso = (
  timestamp: string,
  milliseconds: number,
): string => epochMillisToIsoTimestamp(Date.parse(timestamp) + milliseconds)

export const utcStartOfDayIsoTimestamp = (timestamp: string): string => {
  const date = new Date(timestamp)
  date.setUTCHours(0, 0, 0, 0)

  return date.toISOString()
}

export const utcStartOfHourIsoTimestamp = (timestamp: string): string => {
  const date = new Date(timestamp)
  date.setUTCMinutes(0, 0, 0)

  return date.toISOString()
}

export const isoTimestampAfter = (date: Date, milliseconds: number): string =>
  epochMillisToIsoTimestamp(date.getTime() + milliseconds)

export const normalizeIsoTimestamp = (timestamp: string): string =>
  new Date(timestamp).toISOString()
