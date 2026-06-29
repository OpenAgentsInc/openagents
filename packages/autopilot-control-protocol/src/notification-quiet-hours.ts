export type QuietHoursInput = {
  hour: number
  startHour: number
  endHour: number
}

export type QuietHoursNotification = {
  priority: "low" | "normal" | "high"
}

function clampHour(hour: number): number {
  if (!Number.isFinite(hour)) {
    return 0
  }

  return Math.min(23, Math.max(0, Math.trunc(hour)))
}

export function inQuietHours(input: QuietHoursInput): boolean {
  const hour = clampHour(input.hour)
  const startHour = clampHour(input.startHour)
  const endHour = clampHour(input.endHour)

  if (startHour === endHour) {
    return false
  }

  if (startHour < endHour) {
    return hour >= startHour && hour < endHour
  }

  return hour >= startHour || hour < endHour
}

export function filterByQuietHours<T extends QuietHoursNotification>(
  items: T[],
  quiet: boolean,
): T[] {
  if (!quiet) {
    return items.slice()
  }

  return items.filter((item) => item.priority === "high")
}
