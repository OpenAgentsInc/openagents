export const unixEpochIsoTimestamp = '1970-01-01T00:00:00.000Z'

export const currentUnixMs = (): number => Date.now()

export const currentIsoTimestamp = (): string =>
  new Date(currentUnixMs()).toISOString()

export const formatIsoDateTime = (timestamp: string): string =>
  new Date(timestamp).toLocaleString()

export const friendlyRelativeTime = (
  iso: string | null | undefined,
): string => {
  if (iso === null || iso === undefined) {
    return 'Recently'
  }

  const time = Date.parse(iso)

  if (!Number.isFinite(time)) {
    return iso
  }

  const elapsedMs = Math.max(0, currentUnixMs() - time)
  const minuteMs = 60_000
  const hourMs = 60 * minuteMs
  const dayMs = 24 * hourMs

  if (elapsedMs < minuteMs) {
    return 'Just now'
  }

  if (elapsedMs < hourMs) {
    const minutes = Math.floor(elapsedMs / minuteMs)

    return minutes === 1 ? '1 minute ago' : `${minutes} minutes ago`
  }

  if (elapsedMs < dayMs) {
    const hours = Math.floor(elapsedMs / hourMs)

    return hours === 1 ? '1 hour ago' : `${hours} hours ago`
  }

  if (elapsedMs < 2 * dayMs) {
    return 'Yesterday'
  }

  const days = Math.floor(elapsedMs / dayMs)

  return `${days} days ago`
}
