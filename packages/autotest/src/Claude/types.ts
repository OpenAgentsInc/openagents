import type { InteractionStep } from "../Testing/types.js"

export interface ClaudeScreenshotRequest {
  readonly url: string
  readonly fullPage?: boolean
  readonly outputPath?: string
  readonly interactions?: ReadonlyArray<InteractionStep>
  readonly viewport?: {
    readonly width: number
    readonly height: number
  }
}

export interface ClaudeScreenshotResult {
  readonly _tag: "ClaudeScreenshotResult"
  readonly path: string
  readonly timestamp: Date
  readonly success: boolean
  readonly error?: string
}

export interface SecurityOptions {
  readonly allowedHosts?: ReadonlyArray<string>
  readonly maxExecutionTime?: number
  readonly outputDirectory?: string
}
