import type { MessageRowTs, ThreadSummaryTs } from 'tricoder/types'

export function useTinyvex() {
  return { messagesByThread: {} as Record<string, MessageRowTs[]> }
}
// Type-only export: the Expo component imports `type ThreadRow` from this path.
// We re-export the matching type from tricoder/types to satisfy TypeScript without bringing runtime code.
export type { ThreadSummaryTs as ThreadRow }
