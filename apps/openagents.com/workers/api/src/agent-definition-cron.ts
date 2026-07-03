import { epochMillisToIsoTimestamp } from './runtime-primitives'

type CronField = Readonly<{
  values: ReadonlySet<number>
  wildcard: boolean
}>

type ParsedCron = Readonly<{
  minutes: CronField
  hours: CronField
  daysOfMonth: CronField
  months: CronField
  daysOfWeek: CronField
}>

type LocalMinute = Readonly<{
  minute: number
  hour: number
  day: number
  month: number
  weekday: number
}>

export type ComputeNextCronRunAtInput = Readonly<{
  expr: string
  tz: string
  afterIso: string
}>

export class CronScheduleError extends Error {
  override readonly name = 'CronScheduleError'
}

const minuteMs = 60 * 1000
const maxSearchMinutes = 366 * 24 * 60

const weekdayNumbers: Readonly<Record<string, number>> = {
  Sun: 0,
  Mon: 1,
  Tue: 2,
  Wed: 3,
  Thu: 4,
  Fri: 5,
  Sat: 6,
}

const parseCronNumber = (
  token: string,
  fieldName: string,
  minimum: number,
  maximum: number,
): number => {
  const value = Number(token)

  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    throw new CronScheduleError(`Invalid ${fieldName} cron value: ${token}`)
  }

  return value
}

const cronRange = (
  start: number,
  end: number,
  step: number,
  normalize: (value: number) => number,
): ReadonlyArray<number> => {
  if (step <= 0) {
    throw new CronScheduleError('Cron step must be greater than zero.')
  }

  if (start > end) {
    throw new CronScheduleError('Cron range start must be before range end.')
  }

  return Array.from(
    { length: Math.floor((end - start) / step) + 1 },
    (_unused, index) => normalize(start + (index * step)),
  )
}

const parseCronField = (
  rawField: string,
  fieldName: string,
  minimum: number,
  maximum: number,
  normalize: (value: number) => number = value => value,
): CronField => {
  const values = new Set<number>()
  const rawParts = rawField.split(',').map(part => part.trim())

  if (rawParts.some(part => part === '')) {
    throw new CronScheduleError(`Invalid empty ${fieldName} cron field.`)
  }

  for (const rawPart of rawParts) {
    const stepParts = rawPart.split('/')

    if (stepParts.length > 2) {
      throw new CronScheduleError(`Invalid ${fieldName} cron step syntax.`)
    }

    const [rangeToken, stepToken] = stepParts
    const step = stepToken === undefined
      ? 1
      : parseCronNumber(stepToken, `${fieldName} step`, 1, maximum)

    if (rangeToken === undefined || rangeToken === '') {
      throw new CronScheduleError(`Invalid ${fieldName} cron field.`)
    }

    const rangeParts = rangeToken === '*'
      ? [String(minimum), String(maximum)]
      : rangeToken.split('-')

    if (rangeParts.length > 2) {
      throw new CronScheduleError(`Invalid ${fieldName} cron range syntax.`)
    }

    const [startToken, endToken] = rangeParts

    if (startToken === undefined || startToken === '') {
      throw new CronScheduleError(`Invalid ${fieldName} cron range syntax.`)
    }

    const start = parseCronNumber(startToken, fieldName, minimum, maximum)
    const end = endToken === undefined
      ? start
      : parseCronNumber(endToken, fieldName, minimum, maximum)

    for (const value of cronRange(start, end, step, normalize)) {
      values.add(value)
    }
  }

  return {
    values,
    wildcard: rawField.trim() === '*',
  }
}

const parseCron = (expr: string): ParsedCron => {
  const fields = expr.trim().split(/\s+/)

  if (fields.length !== 5) {
    throw new CronScheduleError('Cron expression must contain five fields.')
  }

  return {
    minutes: parseCronField(fields[0] ?? '', 'minute', 0, 59),
    hours: parseCronField(fields[1] ?? '', 'hour', 0, 23),
    daysOfMonth: parseCronField(fields[2] ?? '', 'day-of-month', 1, 31),
    months: parseCronField(fields[3] ?? '', 'month', 1, 12),
    daysOfWeek: parseCronField(
      fields[4] ?? '',
      'day-of-week',
      0,
      7,
      value => value === 7 ? 0 : value,
    ),
  }
}

const formatterForTimeZone = (tz: string): Intl.DateTimeFormat => {
  try {
    return new Intl.DateTimeFormat('en-US', {
      day: '2-digit',
      hour: '2-digit',
      hourCycle: 'h23',
      minute: '2-digit',
      month: '2-digit',
      timeZone: tz,
      weekday: 'short',
      year: 'numeric',
    })
  } catch (error) {
    throw new CronScheduleError(`Invalid cron timezone: ${tz}`)
  }
}

const localMinuteFor = (
  formatter: Intl.DateTimeFormat,
  epochMillis: number,
): LocalMinute => {
  const parts = Object.fromEntries(
    formatter
      .formatToParts(epochMillis)
      .filter(part => part.type !== 'literal')
      .map(part => [part.type, part.value]),
  )
  const weekday = weekdayNumbers[parts.weekday ?? '']

  if (weekday === undefined) {
    throw new CronScheduleError(
      'Unable to resolve cron weekday in the requested timezone.',
    )
  }

  return {
    minute: Number(parts.minute),
    hour: Number(parts.hour),
    day: Number(parts.day),
    month: Number(parts.month),
    weekday,
  }
}

const cronDayMatches = (
  parsedCron: ParsedCron,
  localMinute: LocalMinute,
): boolean => {
  const dayOfMonthMatches = parsedCron.daysOfMonth.values.has(localMinute.day)
  const dayOfWeekMatches = parsedCron.daysOfWeek.values.has(localMinute.weekday)

  if (parsedCron.daysOfMonth.wildcard && parsedCron.daysOfWeek.wildcard) {
    return true
  }

  if (parsedCron.daysOfMonth.wildcard) {
    return dayOfWeekMatches
  }

  if (parsedCron.daysOfWeek.wildcard) {
    return dayOfMonthMatches
  }

  return dayOfMonthMatches || dayOfWeekMatches
}

const cronMinuteMatches = (
  parsedCron: ParsedCron,
  localMinute: LocalMinute,
): boolean =>
  parsedCron.minutes.values.has(localMinute.minute) &&
  parsedCron.hours.values.has(localMinute.hour) &&
  parsedCron.months.values.has(localMinute.month) &&
  cronDayMatches(parsedCron, localMinute)

export const computeNextCronRunAt = ({
  expr,
  tz,
  afterIso,
}: ComputeNextCronRunAtInput): string => {
  const parsedCron = parseCron(expr)
  const formatter = formatterForTimeZone(tz)
  const afterMs = Date.parse(afterIso)

  if (!Number.isFinite(afterMs)) {
    throw new CronScheduleError(`Invalid cron after timestamp: ${afterIso}`)
  }

  const firstCandidateMs = (Math.floor(afterMs / minuteMs) * minuteMs) +
    minuteMs

  for (let offset = 0; offset <= maxSearchMinutes; offset += 1) {
    const candidateMs = firstCandidateMs + (offset * minuteMs)

    if (cronMinuteMatches(parsedCron, localMinuteFor(formatter, candidateMs))) {
      return epochMillisToIsoTimestamp(candidateMs)
    }
  }

  throw new CronScheduleError(
    'No cron run was found within the bounded search window.',
  )
}
