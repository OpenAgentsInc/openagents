import type { KhalaEntry } from "./khala-core"

export const MOBILE_TRANSCRIPT_PAGE_SIZE = 60

export const initialMobileTranscriptVisibleCount = (
  entryCount: number,
): number => Math.min(Math.max(0, entryCount), MOBILE_TRANSCRIPT_PAGE_SIZE)

export const nextMobileTranscriptVisibleCount = (
  current: number,
  entryCount: number,
): number => Math.min(
  Math.max(0, entryCount),
  Math.max(MOBILE_TRANSCRIPT_PAGE_SIZE, current + MOBILE_TRANSCRIPT_PAGE_SIZE),
)

export const visibleMobileTranscriptEntries = (
  entries: ReadonlyArray<KhalaEntry>,
  visibleCount: number,
): ReadonlyArray<KhalaEntry> => entries.slice(-Math.max(0, visibleCount))

export const mobileTranscriptUnreadBoundaryIndex = (
  visibleEntryCount: number,
  unreadCount: number,
): number | null => {
  if (unreadCount <= 0 || visibleEntryCount <= 0) return null
  return Math.max(0, visibleEntryCount - Math.min(unreadCount, visibleEntryCount))
}

export const newlyConfirmedTranscriptEntryCount = (
  before: ReadonlyArray<KhalaEntry>,
  after: ReadonlyArray<KhalaEntry>,
): number => {
  const previous = new Set(before.map(entry => entry.key))
  return after.reduce((count, entry) => count + (previous.has(entry.key) ? 0 : 1), 0)
}
