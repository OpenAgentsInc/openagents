import { describe, expect, test } from "bun:test"

import {
  buildSubmitReceipt,
  validateBuildSubmitReceipt,
  type BuildSubmitReceipt,
} from "../src/coordinator/build-submit-receipt"

const builtReceipt: BuildSubmitReceipt = {
  ipaRef: "artifact.ipa.local.pylon-ios-20260613",
  buildVersion: "2026.06.13.1",
  submissionId: null,
  outcome: "built",
  observedAt: "2026-06-13T14:00:00.000Z",
}

describe("build submit receipt", () => {
  test("builds a normalized built receipt without a submission id", () => {
    const receipt = buildSubmitReceipt({
      ipaRef: " artifact.ipa.local.pylon-ios-20260613 ",
      buildVersion: " 2026.06.13.1 ",
      submissionId: null,
      outcome: "built",
      observedAt: " 2026-06-13T14:00:00.000Z ",
    })

    expect(receipt).toEqual(builtReceipt)
    expect(validateBuildSubmitReceipt(receipt)).toBe(true)
  })

  test("builds a submitted receipt with submission evidence and origin intent ref", () => {
    const receipt = buildSubmitReceipt({
      ipaRef: "artifact.ipa.local.pylon-ios-20260613",
      buildVersion: "2026.06.13.1",
      submissionId: " asc.submission.123 ",
      outcome: "submitted",
      observedAt: "2026-06-13T14:05:00.000Z",
      originIntentRef: " intent.cl39.auto-build-submit ",
    })

    expect(receipt).toEqual({
      ipaRef: "artifact.ipa.local.pylon-ios-20260613",
      buildVersion: "2026.06.13.1",
      submissionId: "asc.submission.123",
      outcome: "submitted",
      observedAt: "2026-06-13T14:05:00.000Z",
      originIntentRef: "intent.cl39.auto-build-submit",
    })
    expect(validateBuildSubmitReceipt(receipt)).toBe(true)
  })

  test("builds a failed receipt with a null submission id", () => {
    const receipt = buildSubmitReceipt({
      ipaRef: "artifact.ipa.local.pylon-ios-20260613",
      buildVersion: "2026.06.13.1",
      submissionId: null,
      outcome: "failed",
      observedAt: "2026-06-13T14:10:00.000Z",
      originIntentRef: "",
    })

    expect(receipt).toEqual({
      ipaRef: "artifact.ipa.local.pylon-ios-20260613",
      buildVersion: "2026.06.13.1",
      submissionId: null,
      outcome: "failed",
      observedAt: "2026-06-13T14:10:00.000Z",
    })
    expect(validateBuildSubmitReceipt(receipt)).toBe(true)
  })

  test("rejects receipts with missing required fields", () => {
    expect(validateBuildSubmitReceipt({ ...builtReceipt, ipaRef: undefined })).toBe(false)
    expect(validateBuildSubmitReceipt({ ...builtReceipt, buildVersion: "" })).toBe(false)
    expect(validateBuildSubmitReceipt({ ...builtReceipt, observedAt: " " })).toBe(false)
  })

  test("rejects invalid outcome and empty submission ids", () => {
    expect(validateBuildSubmitReceipt({ ...builtReceipt, outcome: "uploaded" })).toBe(false)
    expect(validateBuildSubmitReceipt({ ...builtReceipt, submissionId: "" })).toBe(false)
    expect(validateBuildSubmitReceipt({ ...builtReceipt, originIntentRef: " " })).toBe(false)
  })
})
