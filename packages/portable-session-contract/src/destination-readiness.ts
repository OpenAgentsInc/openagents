import { Schema } from "effect"

import {
  IdePortableDestinationActivationReceiptSchema,
  type IdePortableDestinationActivationReceipt,
  type IdePortableDestinationHelperKind,
} from "./ide13-contract.js"

const HELPER_KINDS: ReadonlyArray<IdePortableDestinationHelperKind> = [
  "pty", "lsp", "dap", "watcher", "native",
]
const decodeDestinationActivationReceipt = Schema.decodeUnknownSync(
  IdePortableDestinationActivationReceiptSchema,
)

export const IDE_PORTABLE_HELPER_OBSERVATION_MAXIMUM_AGE_MS = 60_000
export const IDE_PORTABLE_HELPER_OBSERVATION_MAXIMUM_FUTURE_SKEW_MS = 5_000

export type IdePortableDestinationActivationExpectation = Readonly<{
  operationRef: string
  sessionRef: string
  checkpointRef: string
  destinationTargetRef: string
  destinationAttachmentRef: string
  destinationRunnerSessionReservationRef: string
  destinationGeneration: number
  authenticationPolicyRef: string
  now?: Date
}>

export class IdePortableDestinationReceiptError extends Error {
  readonly _tag = "IdePortableDestinationReceiptError"
  override readonly name = "IdePortableDestinationReceiptError"

  constructor(
    readonly reason:
      | "binding_mismatch"
      | "authentication_expired"
      | "authentication_rejected"
      | "helper_observation_invalid"
      | "helper_inventory_invalid",
    message: string,
  ) {
    super(message)
  }
}

/** Decode and validate a destination receipt before the caller admits work. */
export const validateIdePortableDestinationActivationReceipt = (
  value: unknown,
  expected: IdePortableDestinationActivationExpectation,
): IdePortableDestinationActivationReceipt => {
  const receipt = decodeDestinationActivationReceipt(value, { onExcessProperty: "error" })
  if (receipt.operationRef !== expected.operationRef ||
      receipt.sessionRef !== expected.sessionRef ||
      receipt.checkpointRef !== expected.checkpointRef ||
      receipt.destinationTargetRef !== expected.destinationTargetRef ||
      receipt.destinationAttachmentRef !== expected.destinationAttachmentRef ||
      receipt.destinationRunnerSessionReservationRef !== expected.destinationRunnerSessionReservationRef ||
      receipt.destinationGeneration !== expected.destinationGeneration ||
      receipt.authentication.policyRef !== expected.authenticationPolicyRef) {
    throw new IdePortableDestinationReceiptError(
      "binding_mismatch",
      "destination readiness receipt does not match the requested activation",
    )
  }
  if (receipt.authentication.state !== "reauthenticated") {
    throw new IdePortableDestinationReceiptError(
      "authentication_rejected",
      "destination authentication is not active",
    )
  }
  const now = (expected.now ?? new Date()).getTime()
  const helpersObservedAt = Date.parse(receipt.helpersObservedAt)
  if (!Number.isFinite(now) ||
      !Number.isFinite(helpersObservedAt) ||
      helpersObservedAt < now - IDE_PORTABLE_HELPER_OBSERVATION_MAXIMUM_AGE_MS ||
      helpersObservedAt > now + IDE_PORTABLE_HELPER_OBSERVATION_MAXIMUM_FUTURE_SKEW_MS) {
    throw new IdePortableDestinationReceiptError(
      "helper_observation_invalid",
      "destination helper observation is stale, invalid, or in the future",
    )
  }
  if (receipt.authentication.expiresAt !== null &&
      Date.parse(receipt.authentication.expiresAt) <= now) {
    throw new IdePortableDestinationReceiptError(
      "authentication_expired",
      "destination authentication expired before activation",
    )
  }
  const helpers = new Map(receipt.helpers.map(helper => [helper.kind, helper]))
  if (helpers.size !== HELPER_KINDS.length || HELPER_KINDS.some(kind => !helpers.has(kind))) {
    throw new IdePortableDestinationReceiptError(
      "helper_inventory_invalid",
      "destination helper inventory is incomplete or duplicated",
    )
  }
  for (const helper of receipt.helpers) {
    const ready = helper.readiness === "ready"
    if ((ready && (helper.instanceRef === null || helper.versionRef === null || helper.omissionRef !== null)) ||
        (!ready && (helper.instanceRef !== null || helper.versionRef !== null || helper.omissionRef === null))) {
      throw new IdePortableDestinationReceiptError(
        "helper_inventory_invalid",
        "destination helper fact does not match its readiness state",
      )
    }
  }
  return receipt
}
