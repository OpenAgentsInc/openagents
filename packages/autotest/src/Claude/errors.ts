import { Data } from "effect"

export class SecurityError extends Data.TaggedError("SecurityError")<{
  readonly url: string
  readonly message: string
}> {}

export class InvalidRequestError extends Data.TaggedError("InvalidRequestError")<{
  readonly field: string
  readonly value: unknown
  readonly message: string
}> {}
