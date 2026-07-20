import { Schema } from "effect"

import {
  IdePortableDestinationActivationReceiptSchema,
  type IdePortableDestinationActivationReceipt,
  type IdePortableDestinationHelperKind,
} from "./ide13-contract.js"

const HELPER_KINDS: ReadonlyArray<IdePortableDestinationHelperKind> = [
  "pty", "lsp", "dap", "watcher", "native",
]

export type IdePortableDestinationActivationExpectation = Readonly<{
  operationRef: string
  sessionRef: string
  checkpointRef: string
  destinationTargetRef: string
  destinationAttachmentRef: string
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
  const receipt = Schema.decodeUnknownSync(IdePortableDestinationActivationReceiptSchema)(value)
  if (receipt.operationRef !== expected.operationRef ||
      receipt.sessionRef !== expected.sessionRef ||
      receipt.checkpointRef !== expected.checkpointRef ||
      receipt.destinationTargetRef !== expected.destinationTargetRef ||
      receipt.destinationAttachmentRef !== expected.destinationAttachmentRef ||
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
  if (receipt.authentication.expiresAt !== null &&
      Date.parse(receipt.authentication.expiresAt) <= (expected.now ?? new Date()).getTime()) {
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
