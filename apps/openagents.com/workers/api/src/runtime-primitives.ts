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

export type TimezoneDayBounds = Readonly<{
  todayStartIso: string
  yesterdayStartIso: string
}>

type TimezoneDayParts = Readonly<{
  day: number
  month: number
  year: number
}>

const dateFormatterForTimezone = (timezone: string): Intl.DateTimeFormat =>
  new Intl.DateTimeFormat('en-US', {
    calendar: 'iso8601',
    day: '2-digit',
    month: '2-digit',
    timeZone: timezone,
    year: 'numeric',
  })

const dateTimeFormatterForTimezone = (timezone: string): Intl.DateTimeFormat =>
  new Intl.DateTimeFormat('en-US', {
    calendar: 'iso8601',
    day: '2-digit',
    hour: '2-digit',
    hour12: false,
    hourCycle: 'h23',
    minute: '2-digit',
    month: '2-digit',
    second: '2-digit',
    timeZone: timezone,
    year: 'numeric',
  })

const numberPart = (
  parts: ReadonlyArray<Intl.DateTimeFormatPart>,
  type: Intl.DateTimeFormatPartTypes,
): number => Number(parts.find(part => part.type === type)?.value ?? '0')

const dayPartsInTimezone = (
  timestamp: string,
  timezone: string,
): TimezoneDayParts | undefined => {
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) {
    return undefined
  }

  const parts = dateFormatterForTimezone(timezone).formatToParts(date)

  return {
    day: numberPart(parts, 'day'),
    month: numberPart(parts, 'month'),
    year: numberPart(parts, 'year'),
  }
}

const dayPartsToKey = (parts: TimezoneDayParts): string =>
  `${String(parts.year).padStart(4, '0')}-${String(parts.month).padStart(
    2,
    '0',
  )}-${String(parts.day).padStart(2, '0')}`

export const dayKeyInTimezone = (
  timestamp: string,
  timezone: string,
): string | undefined => {
  const parts = dayPartsInTimezone(timestamp, timezone)

  return parts === undefined ? undefined : dayPartsToKey(parts)
}

const addCalendarDays = (
  parts: TimezoneDayParts,
  days: number,
): TimezoneDayParts => {
  const shifted = new Date(
    Date.UTC(parts.year, parts.month - 1, parts.day + days),
  )

  return {
    day: shifted.getUTCDate(),
    month: shifted.getUTCMonth() + 1,
    year: shifted.getUTCFullYear(),
  }
}

const offsetMillisAt = (
  epochMillis: number,
  formatter: Intl.DateTimeFormat,
): number => {
  const parts = formatter.formatToParts(new Date(epochMillis))
  const hour = numberPart(parts, 'hour')
  const localMillis = Date.UTC(
    numberPart(parts, 'year'),
    numberPart(parts, 'month') - 1,
    numberPart(parts, 'day'),
    hour === 24 ? 0 : hour,
    numberPart(parts, 'minute'),
    numberPart(parts, 'second'),
  )

  return localMillis - epochMillis
}

const startOfDayIsoForParts = (
  parts: TimezoneDayParts,
  timezone: string,
): string => {
  const localMidnightAsEpoch = Date.UTC(parts.year, parts.month - 1, parts.day)
  const formatter = dateTimeFormatterForTimezone(timezone)
  let epochMillis = localMidnightAsEpoch

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const nextEpochMillis =
      localMidnightAsEpoch - offsetMillisAt(epochMillis, formatter)
    if (Math.abs(nextEpochMillis - epochMillis) < 1) break
    epochMillis = nextEpochMillis
  }

  return epochMillisToIsoTimestamp(epochMillis)
}

export const startOfDayIsoTimestampInTimezone = (
  timestamp: string,
  timezone: string,
): string => {
  const parts = dayPartsInTimezone(timestamp, timezone)

  return parts === undefined
    ? utcStartOfDayIsoTimestamp(timestamp)
    : startOfDayIsoForParts(parts, timezone)
}

export const todayAndYesterdayBoundsInTimezone = (
  timestamp: string,
  timezone: string,
): TimezoneDayBounds => {
  const today = dayPartsInTimezone(timestamp, timezone)
  const todayStartIso =
    today === undefined
      ? utcStartOfDayIsoTimestamp(timestamp)
      : startOfDayIsoForParts(today, timezone)
  const yesterday =
    today === undefined
      ? dayPartsInTimezone(
          isoTimestampAfterIso(todayStartIso, -24 * 60 * 60 * 1000),
          timezone,
        )
      : addCalendarDays(today, -1)

  return {
    todayStartIso,
    yesterdayStartIso:
      yesterday === undefined
        ? isoTimestampAfterIso(todayStartIso, -24 * 60 * 60 * 1000)
        : startOfDayIsoForParts(yesterday, timezone),
  }
}
