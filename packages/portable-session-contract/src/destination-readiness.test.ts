import { describe, expect, test } from "vite-plus/test"

import { validateIdePortableDestinationActivationReceipt } from "./destination-readiness.js"

const expectation = {
  operationRef: "operation.destination.activate.2",
  sessionRef: "session.destination.2",
  checkpointRef: "checkpoint.destination.2",
  destinationTargetRef: "target.destination.2",
  destinationAttachmentRef: "attachment.destination.2",
  destinationRunnerSessionReservationRef: "reservation.destination.runner.2",
  destinationGeneration: 2,
  authenticationPolicyRef: "policy.destination.owner_local.v1",
  now: new Date("2026-07-20T08:30:00.000Z"),
}

const receipt = () => ({
  schema: "openagents.ide_portable_destination_activation.v1",
  receiptRef: "receipt.destination.activation.2",
  operationRef: expectation.operationRef,
  sessionRef: expectation.sessionRef,
  checkpointRef: expectation.checkpointRef,
  destinationTargetRef: expectation.destinationTargetRef,
  destinationAttachmentRef: expectation.destinationAttachmentRef,
  destinationRunnerSessionReservationRef: expectation.destinationRunnerSessionReservationRef,
  destinationGeneration: expectation.destinationGeneration,
  authentication: {
    state: "reauthenticated",
    policyRef: expectation.authenticationPolicyRef,
    evidenceRef: "evidence.destination.authentication.2",
    observedAt: "2026-07-20T08:00:00.000Z",
    expiresAt: "2026-07-20T09:00:00.000Z",
  },
  helpersObservedAt: "2026-07-20T08:29:30.000Z",
  helpers: ["pty", "lsp", "dap", "watcher", "native"].map(kind => ({
    kind,
    readiness: "unsupported",
    instanceRef: null,
    versionRef: null,
    omissionRef: `omission.destination.${kind}`,
    evidenceRefs: [],
  })),
  activatedAgentRefs: ["agent.destination.root"],
  acceptedWorkRefs: [],
  evidenceRefs: ["evidence.destination.authentication.2"],
})

describe("IDE destination readiness validation", () => {
  test("accepts an exact fresh receipt with explicit helper omissions", () => {
    expect(validateIdePortableDestinationActivationReceipt(receipt(), expectation).destinationGeneration).toBe(2)
  })

  test("rejects replay, stale generation, expired or revoked authentication, and incomplete helpers", () => {
    expect(() => validateIdePortableDestinationActivationReceipt({ ...receipt(), sessionRef: "session.replayed" }, expectation)).toThrow("does not match")
    expect(() => validateIdePortableDestinationActivationReceipt({ ...receipt(), destinationGeneration: 1 }, expectation)).toThrow("does not match")
    expect(() => validateIdePortableDestinationActivationReceipt({
      ...receipt(), authentication: { ...receipt().authentication, expiresAt: "2026-07-20T08:00:00.000Z" },
    }, expectation)).toThrow("expired")
    expect(() => validateIdePortableDestinationActivationReceipt({
      ...receipt(), authentication: { ...receipt().authentication, state: "revoked" },
    }, expectation)).toThrow("not active")
    expect(() => validateIdePortableDestinationActivationReceipt({ ...receipt(), helpers: receipt().helpers.slice(1) }, expectation)).toThrow("incomplete")
  })

  test("rejects a receipt for a different destination runner reservation", () => {
    expect(() => validateIdePortableDestinationActivationReceipt({
      ...receipt(),
      destinationRunnerSessionReservationRef: "reservation.destination.runner.replayed",
    }, expectation)).toThrow("does not match")
  })

  test("rejects stale and future helper observations", () => {
    expect(() => validateIdePortableDestinationActivationReceipt({
      ...receipt(),
      helpersObservedAt: "2026-07-20T08:28:59.999Z",
    }, expectation)).toThrow("stale, invalid, or in the future")
    expect(() => validateIdePortableDestinationActivationReceipt({
      ...receipt(),
      helpersObservedAt: "2026-07-20T08:30:05.001Z",
    }, expectation)).toThrow("stale, invalid, or in the future")
  })

  test("rejects omitted receipt bindings and excess receipt properties", () => {
    const { destinationRunnerSessionReservationRef: _reservationRef, ...withoutReservation } = receipt()
    const { helpersObservedAt: _helpersObservedAt, ...withoutObservation } = receipt()
    expect(() => validateIdePortableDestinationActivationReceipt(withoutReservation, expectation)).toThrow()
    expect(() => validateIdePortableDestinationActivationReceipt(withoutObservation, expectation)).toThrow()
    expect(() => validateIdePortableDestinationActivationReceipt({
      ...receipt(),
      rawDestinationHandle: "forbidden",
    }, expectation)).toThrow()
  })

  test("rejects a helper that mixes a fresh instance with an unsupported omission", () => {
    const helpers = receipt().helpers.map((helper, index) => index === 0
      ? { ...helper, instanceRef: "instance.replayed.pty" }
      : helper)
    expect(() => validateIdePortableDestinationActivationReceipt({ ...receipt(), helpers }, expectation)).toThrow("readiness state")
  })
})
