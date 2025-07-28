// Re-export all commands and types
export * from "./errors"
export * from "./command"
export * from "./session"
export * from "./apm"
export * from "./history"
export * from "./system"
export * from "./convex"

// Combined IPC namespace for convenience
import { SessionCommands } from "./session"
import { APMCommands } from "./apm"
import { HistoryCommands } from "./history"
import { SystemCommands } from "./system"
import { ConvexCommands } from "./convex"

export const IPC = {
  session: SessionCommands,
  apm: APMCommands,
  history: HistoryCommands,
  system: SystemCommands,
  convex: ConvexCommands
} as const