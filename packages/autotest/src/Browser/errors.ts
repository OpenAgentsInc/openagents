import { Data } from "effect"

export class BrowserError extends Data.TaggedError("BrowserError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

export class PageError extends Data.TaggedError("PageError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

export class NavigationError extends Data.TaggedError("NavigationError")<{
  readonly url: string
  readonly message: string
  readonly cause?: unknown
}> {}

export class TimeoutError extends Data.TaggedError("TimeoutError")<{
  readonly operation: string
  readonly timeout: number
  readonly message: string
}> {}
