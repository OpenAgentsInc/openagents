import { Context, Effect, Layer } from "effect"
import * as fs from "node:fs/promises"
import * as path from "node:path"
import { FileError, ScreenshotError } from "./errors.js"
import type { ComparisonError } from "./errors.js"
import type { DiffResult, Screenshot, ScreenshotOptions } from "./types.js"

export class ScreenshotService extends Context.Tag("@openagentsinc/autotest/ScreenshotService")<
  ScreenshotService,
  {
    readonly capture: (options: ScreenshotOptions) => Effect.Effect<Screenshot, ScreenshotError>
    readonly save: (screenshot: Screenshot, filePath: string) => Effect.Effect<void, FileError>
    readonly load: (filePath: string) => Effect.Effect<Screenshot, FileError>
    readonly compare: (
      baseline: Screenshot,
      current: Screenshot,
      threshold?: number
    ) => Effect.Effect<DiffResult, ComparisonError>
  }
>() {}

export const ScreenshotServiceLive = Layer.succeed(
  ScreenshotService,
  ScreenshotService.of({
    capture: (options: ScreenshotOptions) =>
      Effect.tryPromise({
        try: async () => {
          const screenshotOptions: any = {
            fullPage: options.fullPage ?? false,
            omitBackground: options.omitBackground ?? false,
            encoding: "binary" as const,
            type: options.type ?? "png"
          }

          if (options.clip) {
            screenshotOptions.clip = options.clip
          }
          if (options.quality !== undefined) {
            screenshotOptions.quality = options.quality
          }

          const buffer = Buffer.from(await options.page.instance.screenshot(screenshotOptions))

          // Get viewport size for dimensions
          const viewport = options.page.instance.viewport()
          const width = options.clip?.width ?? viewport?.width ?? 1920
          const height = options.clip?.height ?? viewport?.height ?? 1080

          return {
            _tag: "Screenshot",
            buffer,
            width,
            height,
            timestamp: new Date()
          } as const
        },
        catch: (error) =>
          new ScreenshotError({
            message: `Failed to capture screenshot: ${error}`,
            cause: error
          })
      }),

    save: (screenshot: Screenshot, filePath: string) =>
      Effect.tryPromise({
        try: async () => {
          const dir = path.dirname(filePath)
          await fs.mkdir(dir, { recursive: true })
          await fs.writeFile(filePath, screenshot.buffer)
        },
        catch: (error) =>
          new FileError({
            path: filePath,
            message: `Failed to save screenshot: ${error}`,
            cause: error
          })
      }),

    load: (filePath: string) =>
      Effect.tryPromise({
        try: async () => {
          const buffer = await fs.readFile(filePath)
          const stats = await fs.stat(filePath)

          // For loaded screenshots, we don't have the original dimensions
          // This is a simplified implementation
          return {
            _tag: "Screenshot",
            buffer,
            width: 0, // Would need image parsing to get actual dimensions
            height: 0,
            timestamp: stats.mtime
          } as const
        },
        catch: (error) =>
          new FileError({
            path: filePath,
            message: `Failed to load screenshot: ${error}`,
            cause: error
          })
      }),

    compare: (baseline: Screenshot, current: Screenshot, _threshold = 0.1) =>
      Effect.sync(() => {
        // Simplified comparison - in a real implementation we'd use pixelmatch
        const baselineSize = baseline.buffer.length
        const currentSize = current.buffer.length

        if (baselineSize !== currentSize) {
          const difference = Math.abs(baselineSize - currentSize) / baselineSize
          return {
            _tag: "DiffResult",
            match: false,
            difference
          } as const
        }

        // For now, just compare buffer sizes
        // In production, we'd use pixelmatch for pixel-by-pixel comparison
        return {
          _tag: "DiffResult",
          match: true,
          difference: 0
        } as const
      })
  })
)
