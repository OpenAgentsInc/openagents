import type { Page } from "../Browser/types.js"

export interface Screenshot {
  readonly _tag: "Screenshot"
  readonly buffer: Buffer
  readonly width: number
  readonly height: number
  readonly timestamp: Date
}

export interface ScreenshotOptions {
  readonly page: Page
  readonly fullPage?: boolean
  readonly clip?: {
    readonly x: number
    readonly y: number
    readonly width: number
    readonly height: number
  }
  readonly omitBackground?: boolean
  readonly encoding?: "base64" | "binary"
  readonly quality?: number
  readonly type?: "jpeg" | "png" | "webp"
}

export interface DiffResult {
  readonly _tag: "DiffResult"
  readonly match: boolean
  readonly difference: number
  readonly diffImage?: Buffer
}
