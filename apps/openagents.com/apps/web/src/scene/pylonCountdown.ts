import { Duration, Effect, Fiber } from 'effect'
import { slotText } from 'slot-text'

import { currentUnixMs } from '../time-format'

// Drives a centered countdown to the next 1 PM America/Chicago target using
// slot-text for digit animation and Effect for the once-a-second tick loop.

const CENTRAL_TIME_ZONE = 'America/Chicago'
const PYLON_TARGET_HOUR_CT = 13

const centralFormatter = new Intl.DateTimeFormat('en-US', {
  day: '2-digit',
  hour: '2-digit',
  hourCycle: 'h23',
  minute: '2-digit',
  month: '2-digit',
  second: '2-digit',
  timeZone: CENTRAL_TIME_ZONE,
  year: 'numeric',
})

type CentralTimeParts = Readonly<{
  day: number
  hour: number
  minute: number
  month: number
  second: number
  year: number
}>

// Launch handoff for the June 15, 2026 live download/instructions switch.
// Remove this fixed-deadline gate after the landing page no longer needs to
// preserve pre-launch countdown behavior for already-open browser sessions.
const PYLON_LAUNCH_DEADLINE_CT: CentralTimeParts = {
  day: 15,
  hour: PYLON_TARGET_HOUR_CT,
  minute: 0,
  month: 6,
  second: 0,
  year: 2026,
}

const pad = (value: number): string => value.toString().padStart(2, '0')

const numberPart = (
  parts: ReadonlyArray<Intl.DateTimeFormatPart>,
  type: Intl.DateTimeFormatPartTypes,
): number => Number(parts.find(part => part.type === type)?.value ?? '0')

const centralTimeParts = (timestampMs: number): CentralTimeParts => {
  const parts = centralFormatter.formatToParts(timestampMs)

  return {
    day: numberPart(parts, 'day'),
    hour: numberPart(parts, 'hour'),
    minute: numberPart(parts, 'minute'),
    month: numberPart(parts, 'month'),
    second: numberPart(parts, 'second'),
    year: numberPart(parts, 'year'),
  }
}

const centralWallTimestampMs = (parts: CentralTimeParts): number =>
  Date.UTC(
    parts.year,
    parts.month - 1,
    parts.day,
    parts.hour,
    parts.minute,
    parts.second,
  )

const centralOffsetMsAt = (timestampMs: number): number =>
  centralWallTimestampMs(centralTimeParts(timestampMs)) - timestampMs

const timestampForCentralWallTime = (parts: CentralTimeParts): number => {
  const utcGuess = centralWallTimestampMs(parts)
  const firstCandidate = utcGuess - centralOffsetMsAt(utcGuess)

  return utcGuess - centralOffsetMsAt(firstCandidate)
}

const centralDateAfterDays = (
  parts: CentralTimeParts,
  days: number,
): CentralTimeParts =>
  centralTimeParts(
    Date.UTC(parts.year, parts.month - 1, parts.day + days, 12, 0, 0),
  )

export const formatRemaining = (ms: number): string => {
  const total = Math.floor(Math.max(0, ms) / 1000)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = total % 60
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`
}

export const nextPylonCountdownDeadlineMs = (nowMs: number): number => {
  const nowCentral = centralTimeParts(nowMs)
  const todayTarget = timestampForCentralWallTime({
    ...nowCentral,
    hour: PYLON_TARGET_HOUR_CT,
    minute: 0,
    second: 0,
  })

  if (todayTarget > nowMs) {
    return todayTarget
  }

  const tomorrowCentral = centralDateAfterDays(nowCentral, 1)

  return timestampForCentralWallTime({
    ...tomorrowCentral,
    hour: PYLON_TARGET_HOUR_CT,
    minute: 0,
    second: 0,
  })
}

export const remainingToPylonCountdownDeadlineMs = (nowMs: number): number =>
  Math.max(0, nextPylonCountdownDeadlineMs(nowMs) - nowMs)

export const pylonLaunchDeadlineMs = (): number =>
  timestampForCentralWallTime(PYLON_LAUNCH_DEADLINE_CT)

export const remainingToPylonLaunchDeadlineMs = (nowMs: number): number =>
  Math.max(0, pylonLaunchDeadlineMs() - nowMs)

export const isPylonLaunchDeadlinePassed = (nowMs: number): boolean =>
  remainingToPylonLaunchDeadlineMs(nowMs) <= 0

export type PylonCountdownHandle = Readonly<{
  dispose: () => void
}>

export type PylonCountdownOptions = Readonly<{
  deadlineMs?: number
  nowMs?: () => number
  onComplete?: () => void
}>

export const mountPylonCountdown = (
  target: HTMLElement,
  options: PylonCountdownOptions = {},
): PylonCountdownHandle => {
  const nowMs = options.nowMs ?? currentUnixMs
  const deadline = options.deadlineMs ?? nextPylonCountdownDeadlineMs(nowMs())
  const controller = slotText(target, formatRemaining(deadline - nowMs()), {
    direction: 'down',
  })

  let disposed = false

  // Effect-driven tick: sleep one second, render the remaining time (only the
  // digits that changed roll), and stop once the countdown reaches zero.
  const loop = Effect.gen(function* () {
    while (true) {
      yield* Effect.sleep(Duration.seconds(1))
      if (disposed) return
      const remaining = Math.max(0, deadline - nowMs())
      controller.set(formatRemaining(remaining))
      if (remaining <= 0) {
        options.onComplete?.()
        return
      }
    }
  })

  const fiber = Effect.runFork(loop)

  return {
    dispose: () => {
      if (disposed) return
      disposed = true
      Effect.runFork(Fiber.interrupt(fiber))
      controller.destroy()
    },
  }
}
