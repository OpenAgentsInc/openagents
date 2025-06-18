import { Effect } from "effect"
import * as path from "node:path"
import { ScreenshotService } from "./Service.js"
import type { Screenshot } from "./types.js"

export interface StorageOptions {
  readonly baseDir?: string
  readonly organize?: "flat" | "date" | "test"
}

export const createScreenshotPath = (
  name: string,
  options: StorageOptions = {}
): string => {
  const baseDir = options.baseDir ?? ".autotest/screenshots"
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-")

  switch (options.organize) {
    case "date": {
      const date = new Date()
      const year = date.getFullYear()
      const month = String(date.getMonth() + 1).padStart(2, "0")
      const day = String(date.getDate()).padStart(2, "0")
      return path.join(baseDir, year.toString(), month, day, `${name}-${timestamp}.png`)
    }
    case "test":
      return path.join(baseDir, "tests", name, `${timestamp}.png`)
    case "flat":
    default:
      return path.join(baseDir, `${name}-${timestamp}.png`)
  }
}

export const saveScreenshot = (
  name: string,
  screenshot: Screenshot,
  options: StorageOptions = {}
) =>
  Effect.gen(function*() {
    const service = yield* ScreenshotService
    const filePath = createScreenshotPath(name, options)
    yield* service.save(screenshot, filePath)
    return filePath
  })
