import { Effect } from "effect"
import * as fs from "node:fs/promises"
import * as path from "node:path"
import { FileError } from "../Screenshot/errors.js"

export interface ScreenshotMetadata {
  readonly path: string
  readonly timestamp: Date
  readonly size: number
  readonly dimensions?: {
    readonly width: number
    readonly height: number
  }
}

export const listScreenshots = (directory = ".autotest/screenshots") =>
  Effect.tryPromise({
    try: async () => {
      const files = await fs.readdir(directory, { recursive: true })
      const pngFiles = files
        .filter((file) => typeof file === "string" && file.endsWith(".png"))
        .map((file) => path.join(directory, file))

      const metadata = await Promise.all(
        pngFiles.map(async (filePath) => {
          const stats = await fs.stat(filePath)
          return {
            path: filePath,
            timestamp: stats.mtime,
            size: stats.size
          } as ScreenshotMetadata
        })
      )

      // Sort by timestamp, newest first
      return metadata.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime())
    },
    catch: (error) =>
      new FileError({
        path: directory,
        message: `Failed to list screenshots: ${error}`,
        cause: error
      })
  })

export const getLatestScreenshot = (directory = ".autotest/screenshots") =>
  Effect.gen(function*() {
    const screenshots = yield* listScreenshots(directory)

    if (screenshots.length === 0) {
      yield* Effect.fail(
        new FileError({
          path: directory,
          message: "No screenshots found"
        })
      )
    }

    return screenshots[0]
  })

export const cleanupOldScreenshots = (
  directory = ".autotest/screenshots",
  maxAge = 7 * 24 * 60 * 60 * 1000 // 7 days
) =>
  Effect.gen(function*() {
    const screenshots = yield* listScreenshots(directory)
    const now = Date.now()
    const toDelete = screenshots.filter(
      (screenshot) => now - screenshot.timestamp.getTime() > maxAge
    )

    yield* Effect.forEach(
      toDelete,
      (screenshot) =>
        Effect.tryPromise({
          try: () => fs.unlink(screenshot.path),
          catch: (error) =>
            new FileError({
              path: screenshot.path,
              message: `Failed to delete screenshot: ${error}`,
              cause: error
            })
        }),
      { concurrency: 5 }
    )

    return {
      deleted: toDelete.length,
      remaining: screenshots.length - toDelete.length
    }
  })

export const organizeScreenshots = (
  sourceDir = ".autotest/screenshots",
  organize: "date" | "test" = "date"
) =>
  Effect.gen(function*() {
    const screenshots = yield* listScreenshots(sourceDir)

    yield* Effect.forEach(
      screenshots,
      (screenshot) =>
        Effect.gen(function*() {
          const basename = path.basename(screenshot.path)
          let newDir: string

          if (organize === "date") {
            const date = screenshot.timestamp
            const year = date.getFullYear()
            const month = String(date.getMonth() + 1).padStart(2, "0")
            const day = String(date.getDate()).padStart(2, "0")
            newDir = path.join(sourceDir, year.toString(), month, day)
          } else {
            // Extract test name from filename if possible
            const match = basename.match(/^(.+?)-\d{4}/)
            const testName = match?.[1] ?? "unknown"
            newDir = path.join(sourceDir, "tests", testName)
          }

          const newPath = path.join(newDir, basename)

          if (newPath !== screenshot.path) {
            yield* Effect.tryPromise({
              try: async () => {
                await fs.mkdir(newDir, { recursive: true })
                await fs.rename(screenshot.path, newPath)
              },
              catch: (error) =>
                new FileError({
                  path: screenshot.path,
                  message: `Failed to organize screenshot: ${error}`,
                  cause: error
                })
            })
          }
        }),
      { concurrency: 5 }
    )

    return { organized: screenshots.length }
  })
