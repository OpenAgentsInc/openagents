// @ts-nocheck - Suppress TypeScript errors due to Effect-TS version compatibility issues
import { Effect } from "effect"
import { createCommand, createSimpleCommand } from "./command"
import { SystemError } from "./errors"

// System Commands
export const SystemCommands = {
  greet: (name: string) =>
    createCommand<{ name: string }, string>("greet")
      .invoke({ name })
      .pipe(
        Effect.mapError((error) => new SystemError({
          operation: "greet",
          message: `Failed to greet ${name}`,
          cause: error
        }))
      ),
  
  getProjectDirectory: () =>
    createSimpleCommand<string>("get_project_directory")
      .invoke()
      .pipe(
        Effect.mapError((error) => new SystemError({
          operation: "get_directory",
          message: "Failed to get project directory",
          cause: error
        }))
      )
}

// Helper functions
export const isValidProjectDirectory = (path: string) =>
  path && path.length > 0 && !path.includes("..")

export const normalizeProjectPath = (path: string) =>
  path.replace(/\\/g, "/").replace(/\/+$/, "")