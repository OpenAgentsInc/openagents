/**
 * Minimal owned cron-expression matcher (CFG-2, issue #8517).
 *
 * Five fields — minute, hour, day-of-month, month, day-of-week — evaluated
 * in UTC. Supported per field: `*`, numbers, names (months/weekdays),
 * ranges `a-b`, steps (`*` or `a-b` followed by `/n`), and comma lists.
 * Day-of-month and
 * day-of-week combine with OR when BOTH are restricted (standard cron
 * behavior). No vendor library, no seconds field, no `L/W/#` extensions.
 */

export interface CronSchedule {
  readonly expression: string
  readonly minutes: ReadonlySet<number>
  readonly hours: ReadonlySet<number>
  readonly daysOfMonth: ReadonlySet<number>
  readonly months: ReadonlySet<number>
  readonly daysOfWeek: ReadonlySet<number>
  /** True when the field was `*` (matters for the DOM/DOW OR rule). */
  readonly domIsWildcard: boolean
  readonly dowIsWildcard: boolean
}

export class CronParseError extends Error {
  readonly _tag = "CronParseError"
  constructor(
    readonly expression: string,
    readonly reason: string,
  ) {
    super(`invalid cron expression ${JSON.stringify(expression)}: ${reason}`)
  }
}

const MONTH_NAMES: Record<string, number> = {
  jan: 1, feb: 2, mar: 3, apr: 4, may: 5, jun: 6,
  jul: 7, aug: 8, sep: 9, oct: 10, nov: 11, dec: 12,
}

const DOW_NAMES: Record<string, number> = {
  sun: 0, mon: 1, tue: 2, wed: 3, thu: 4, fri: 5, sat: 6,
}

interface FieldSpec {
  readonly min: number
  readonly max: number
  readonly names?: Record<string, number>
  /** Normalize values after parse (7 -> 0 for Sunday). */
  readonly normalize?: (value: number) => number
}

const FIELDS: ReadonlyArray<FieldSpec> = [
  { min: 0, max: 59 }, // minute
  { min: 0, max: 23 }, // hour
  { min: 1, max: 31 }, // day of month
  { min: 1, max: 12, names: MONTH_NAMES }, // month
  { min: 0, max: 7, names: DOW_NAMES, normalize: (v) => (v === 7 ? 0 : v) }, // day of week
]

const parseValue = (raw: string, spec: FieldSpec, expression: string): number => {
  const named = spec.names?.[raw.toLowerCase()]
  const value = named ?? Number(raw)
  if (!Number.isInteger(value)) {
    throw new CronParseError(expression, `not a number: ${JSON.stringify(raw)}`)
  }
  if (value < spec.min || value > spec.max) {
    throw new CronParseError(
      expression,
      `value ${value} out of range ${spec.min}-${spec.max}`,
    )
  }
  return value
}

const parseField = (
  raw: string,
  spec: FieldSpec,
  expression: string,
): ReadonlySet<number> => {
  const out = new Set<number>()
  const add = (value: number) => out.add(spec.normalize?.(value) ?? value)
  for (const part of raw.split(",")) {
    if (part === "") throw new CronParseError(expression, "empty list item")
    const [rangeRaw, stepRaw, ...extra] = part.split("/")
    if (extra.length > 0 || rangeRaw === undefined || rangeRaw === "") {
      throw new CronParseError(expression, `malformed part ${JSON.stringify(part)}`)
    }
    const step = stepRaw === undefined ? 1 : Number(stepRaw)
    if (!Number.isInteger(step) || step < 1) {
      throw new CronParseError(expression, `invalid step ${JSON.stringify(stepRaw)}`)
    }
    let low: number
    let high: number
    if (rangeRaw === "*") {
      low = spec.min
      high = spec.max
    } else if (rangeRaw.includes("-")) {
      const [a, b, ...rest] = rangeRaw.split("-")
      if (rest.length > 0 || a === undefined || b === undefined || a === "" || b === "") {
        throw new CronParseError(expression, `malformed range ${JSON.stringify(rangeRaw)}`)
      }
      low = parseValue(a, spec, expression)
      high = parseValue(b, spec, expression)
      if (low > high) {
        throw new CronParseError(expression, `descending range ${JSON.stringify(rangeRaw)}`)
      }
    } else {
      if (stepRaw !== undefined) {
        throw new CronParseError(expression, "step requires a range or *")
      }
      add(parseValue(rangeRaw, spec, expression))
      continue
    }
    for (let value = low; value <= high; value += step) add(value)
  }
  return out
}

/** Parse a 5-field cron expression. Throws `CronParseError` — invalid cron in a dispatch table is a config bug, not a runtime condition. */
export const parseCron = (expression: string): CronSchedule => {
  const fields = expression.trim().split(/\s+/)
  if (fields.length !== 5) {
    throw new CronParseError(
      expression,
      `expected 5 fields (minute hour day-of-month month day-of-week), got ${fields.length}`,
    )
  }
  const [minute, hour, dom, month, dow] = fields as [string, string, string, string, string]
  const specs = FIELDS as [FieldSpec, FieldSpec, FieldSpec, FieldSpec, FieldSpec]
  return {
    expression,
    minutes: parseField(minute, specs[0], expression),
    hours: parseField(hour, specs[1], expression),
    daysOfMonth: parseField(dom, specs[2], expression),
    months: parseField(month, specs[3], expression),
    daysOfWeek: parseField(dow, specs[4], expression),
    domIsWildcard: dom === "*",
    dowIsWildcard: dow === "*",
  }
}

/** Does the schedule fire at this instant (minute resolution, UTC)? */
export const cronScheduleMatches = (schedule: CronSchedule, date: Date): boolean => {
  if (!schedule.minutes.has(date.getUTCMinutes())) return false
  if (!schedule.hours.has(date.getUTCHours())) return false
  if (!schedule.months.has(date.getUTCMonth() + 1)) return false
  const domMatch = schedule.daysOfMonth.has(date.getUTCDate())
  const dowMatch = schedule.daysOfWeek.has(date.getUTCDay())
  // Standard cron: both restricted -> OR; otherwise both must match
  // (a wildcard side always matches).
  if (!schedule.domIsWildcard && !schedule.dowIsWildcard) return domMatch || dowMatch
  return domMatch && dowMatch
}

/** Convenience: parse + match. Throws `CronParseError` on a bad expression. */
export const cronMatches = (expression: string, date: Date): boolean =>
  cronScheduleMatches(parseCron(expression), date)
