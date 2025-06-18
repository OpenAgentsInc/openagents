import { Effect, Schema } from "effect"
import * as path from "node:path"
import { BrowserService } from "../Browser/Service.js"
import { ScreenshotService } from "../Screenshot/Service.js"
import { navigate, performInteractions } from "../Testing/Interactions.js"
import { InvalidRequestError, SecurityError } from "./errors.js"
import type { ClaudeScreenshotRequest, SecurityOptions } from "./types.js"

const DEFAULT_ALLOWED_HOSTS = ["localhost", "127.0.0.1", "0.0.0.0"]
const DEFAULT_MAX_EXECUTION_TIME = 30000
const DEFAULT_OUTPUT_DIR = ".autotest/screenshots"

// Schema for validating Claude requests
const ClaudeRequestSchema = Schema.Struct({
  url: Schema.String,
  fullPage: Schema.optional(Schema.Boolean),
  outputPath: Schema.optional(Schema.String),
  interactions: Schema.optional(Schema.Array(
    Schema.Struct({
      action: Schema.Literal("click", "fill", "select", "wait", "navigate"),
      selector: Schema.optional(Schema.String),
      value: Schema.optional(Schema.String),
      timeout: Schema.optional(Schema.Number)
    })
  )),
  viewport: Schema.optional(Schema.Struct({
    width: Schema.Number,
    height: Schema.Number
  }))
})

export const validateRequest = (request: unknown) =>
  Effect.gen(function*() {
    const parseResult = yield* Schema.decodeUnknown(ClaudeRequestSchema)(request)
    return parseResult as ClaudeScreenshotRequest
  }).pipe(
    Effect.mapError((error) =>
      new InvalidRequestError({
        field: "request",
        value: request,
        message: `Invalid request format: ${error}`
      })
    )
  )

export const validateSecurity = (
  url: string,
  outputPath: string | undefined,
  options: SecurityOptions = {}
) =>
  Effect.gen(function*() {
    const allowedHosts = options.allowedHosts ?? DEFAULT_ALLOWED_HOSTS
    const outputDir = options.outputDirectory ?? DEFAULT_OUTPUT_DIR

    // Validate URL
    const parsedUrl = yield* Effect.try({
      try: () => new URL(url),
      catch: () => new SecurityError({ url, message: "Invalid URL format" })
    })

    if (!allowedHosts.includes(parsedUrl.hostname)) {
      yield* Effect.fail(
        new SecurityError({
          url,
          message: `Host '${parsedUrl.hostname}' not in allowed list: ${allowedHosts.join(", ")}`
        })
      )
    }

    // Validate output path
    if (outputPath) {
      const resolvedPath = path.resolve(outputPath)
      const resolvedOutputDir = path.resolve(outputDir)

      if (!resolvedPath.startsWith(resolvedOutputDir)) {
        yield* Effect.fail(
          new SecurityError({
            url: outputPath,
            message: `Output path must be within ${outputDir}`
          })
        )
      }
    }

    return { url: parsedUrl.href, outputPath }
  })

export const captureScreenshot = (
  request: ClaudeScreenshotRequest,
  options: SecurityOptions = {}
) =>
  Effect.gen(function*() {
    const browser = yield* BrowserService
    const screenshot = yield* ScreenshotService

    // Validate security constraints
    const { url } = yield* validateSecurity(request.url, request.outputPath, options)

    // Use race to implement timeout
    const timeout = options.maxExecutionTime ?? DEFAULT_MAX_EXECUTION_TIME

    const operation = Effect.gen(function*() {
      // Launch browser and create page
      const browserInstance = yield* browser.launch({ headless: true })
      const page = yield* browser.newPage(browserInstance)

      // Set viewport if specified
      if (request.viewport) {
        yield* Effect.tryPromise({
          try: () =>
            page.instance.setViewport({
              width: request.viewport!.width,
              height: request.viewport!.height
            }),
          catch: (error) =>
            new InvalidRequestError({
              field: "viewport",
              value: request.viewport,
              message: `Failed to set viewport: ${error}`
            })
        })
      }

      // Navigate to URL
      yield* navigate(page, url)

      // Perform interactions if specified
      if (request.interactions && request.interactions.length > 0) {
        yield* performInteractions(page, request.interactions)
      }

      // Capture screenshot
      const screenshotData = yield* screenshot.capture({
        page,
        fullPage: request.fullPage ?? false
      })

      // Save screenshot
      const outputPath = request.outputPath ??
        path.join(DEFAULT_OUTPUT_DIR, `claude-${Date.now()}.png`)

      yield* screenshot.save(screenshotData, outputPath)

      // Clean up
      yield* browser.closePage(page)
      yield* browser.close(browserInstance)

      return {
        _tag: "ClaudeScreenshotResult",
        path: outputPath,
        timestamp: screenshotData.timestamp,
        success: true
      } as const
    })

    // Race against timeout
    return yield* Effect.race(
      operation,
      Effect.sleep(timeout).pipe(
        Effect.flatMap(() =>
          Effect.fail(
            new SecurityError({
              url,
              message: `Execution timeout after ${timeout}ms`
            })
          )
        )
      )
    )
  })
