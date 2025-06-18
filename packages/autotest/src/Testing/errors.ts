import { Data } from "effect"

export class InteractionError extends Data.TaggedError("InteractionError")<{
  readonly action: string
  readonly selector?: string | undefined
  readonly message: string
  readonly cause?: unknown
}> {}

export class ValidationError extends Data.TaggedError("ValidationError")<{
  readonly expected: string
  readonly actual: string
  readonly message: string
}> {}

export class WaitError extends Data.TaggedError("WaitError")<{
  readonly condition: string
  readonly timeout: number
  readonly message: string
}> {}
