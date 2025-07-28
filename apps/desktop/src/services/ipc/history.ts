import { Effect } from "effect"
import { createCommand } from "./command"
import { HistoryError } from "./errors"

// Types
export interface HistoryEntry {
  id: string
  project: string
  title: string
  created_at: string
  last_active: string
  message_count: number
  is_active: boolean
}

export interface UnifiedHistoryEntry {
  id: string
  project: string
  title: string
  created_at: string
  last_active: string
  message_count: number
  is_active: boolean
  source: "cli" | "sdk"
}

// History Commands
export const HistoryCommands = {
  getHistory: (limit: number = 50) =>
    createCommand<{ limit: number }, HistoryEntry[]>("get_history")
      .invoke({ limit })
      .pipe(
        Effect.mapError((error) => new HistoryError({
          operation: "get",
          limit,
          message: `Failed to get history with limit ${limit}`,
          cause: error
        })),
        Effect.map((entries) => entries || [])
      ),
  
  getUnifiedHistory: (limit: number = 50) =>
    createCommand<{ limit: number }, UnifiedHistoryEntry[]>("get_unified_history")
      .invoke({ limit })
      .pipe(
        Effect.mapError((error) => new HistoryError({
          operation: "get_unified",
          limit,
          message: `Failed to get unified history with limit ${limit}`,
          cause: error
        })),
        Effect.map((entries) => entries || [])
      )
}

// Helper functions
export const filterActiveHistory = (entries: HistoryEntry[]) =>
  entries.filter((entry) => entry.is_active)

export const sortByLastActive = (entries: HistoryEntry[]) =>
  [...entries].sort((a, b) => 
    new Date(b.last_active).getTime() - new Date(a.last_active).getTime()
  )

export const groupByProject = (entries: HistoryEntry[]) =>
  entries.reduce((acc, entry) => {
    const project = entry.project || "Unknown"
    if (!acc[project]) acc[project] = []
    acc[project].push(entry)
    return acc
  }, {} as Record<string, HistoryEntry[]>)