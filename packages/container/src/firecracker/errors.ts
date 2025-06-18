import { Schema } from "@effect/schema"

export class FirecrackerError extends Schema.TaggedError<FirecrackerError>()("FirecrackerError", {
  message: Schema.String
}) {}

export class VMNotFoundError extends Schema.TaggedError<VMNotFoundError>()("VMNotFoundError", {
  vmId: Schema.String
}) {}

export class VMAlreadyExistsError extends Schema.TaggedError<VMAlreadyExistsError>()("VMAlreadyExistsError", {
  vmId: Schema.String
}) {}

export class NetworkSetupError extends Schema.TaggedError<NetworkSetupError>()("NetworkSetupError", {
  message: Schema.String,
  interface: Schema.optionalWith(Schema.String, { exact: true })
}) {}

export class FirecrackerBinaryNotFoundError extends Schema.TaggedError<FirecrackerBinaryNotFoundError>()(
  "FirecrackerBinaryNotFoundError",
  {
    path: Schema.String
  }
) {}

export class VMStartupTimeoutError extends Schema.TaggedError<VMStartupTimeoutError>()("VMStartupTimeoutError", {
  vmId: Schema.String,
  timeoutMs: Schema.Number
}) {}
