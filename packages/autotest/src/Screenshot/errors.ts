import { Data } from "effect"

export class ScreenshotError extends Data.TaggedError("ScreenshotError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

export class FileError extends Data.TaggedError("FileError")<{
  readonly path: string
  readonly message: string
  readonly cause?: unknown
}> {}

export class ComparisonError extends Data.TaggedError("ComparisonError")<{
  readonly message: string
  readonly threshold: number
  readonly difference: number
}> {}
