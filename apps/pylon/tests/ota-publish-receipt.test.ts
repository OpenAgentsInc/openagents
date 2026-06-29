import { describe, expect, test } from "bun:test"

import {
  buildOtaPublishReceipt,
  OTA_PUBLISH_RECEIPT_SCHEMA,
  validateOtaPublishReceipt,
} from "../src/coordinator/ota-publish-receipt"

describe("OTA publish receipt", () => {
  test("builds the exact receipt shape with an origin intent ref", () => {
    const receipt = buildOtaPublishReceipt({
      runtimeVersion: "1.4.0",
      branch: "production",
      updateGroupId: "update-group.public.pylon.cl38",
      assetCount: 7,
      publishedAt: "2026-06-13T14:25:00.000Z",
      originIntentRef: "intent.public.pylon.mobile.cl38",
    })

    expect(receipt).toEqual({
      schema: OTA_PUBLISH_RECEIPT_SCHEMA,
      runtimeVersion: "1.4.0",
      branch: "production",
      updateGroupId: "update-group.public.pylon.cl38",
      assetCount: 7,
      publishedAt: "2026-06-13T14:25:00.000Z",
      originIntentRef: "intent.public.pylon.mobile.cl38",
    })
    expect(Object.keys(receipt).sort()).toEqual([
      "assetCount",
      "branch",
      "originIntentRef",
      "publishedAt",
      "runtimeVersion",
      "schema",
      "updateGroupId",
    ])
  })

  test("normalizes a missing origin intent ref to null", () => {
    expect(
      buildOtaPublishReceipt({
        runtimeVersion: "1.4.0",
        branch: "preview",
        updateGroupId: "update-group.public.pylon.preview",
        assetCount: 0,
        publishedAt: "2026-06-13T14:30:00.000Z",
      }).originIntentRef,
    ).toBeNull()
  })

  test("validates receipts produced by the builder", () => {
    const receipt = buildOtaPublishReceipt({
      runtimeVersion: "1.4.0",
      branch: "preview",
      updateGroupId: "update-group.public.pylon.valid",
      assetCount: 3,
      publishedAt: "2026-06-13T14:35:00.000Z",
    })

    expect(validateOtaPublishReceipt(receipt)).toBe(true)
  })

  test("rejects receipts with the wrong schema", () => {
    const receipt = buildOtaPublishReceipt({
      runtimeVersion: "1.4.0",
      branch: "preview",
      updateGroupId: "update-group.public.pylon.invalid_schema",
      assetCount: 1,
      publishedAt: "2026-06-13T14:40:00.000Z",
    })

    expect(validateOtaPublishReceipt({ ...receipt, schema: "openagents.pylon.other_receipt.v1" })).toBe(false)
  })

  test("rejects non-integer and negative asset counts", () => {
    const receipt = buildOtaPublishReceipt({
      runtimeVersion: "1.4.0",
      branch: "preview",
      updateGroupId: "update-group.public.pylon.invalid_assets",
      assetCount: 1,
      publishedAt: "2026-06-13T14:45:00.000Z",
    })

    expect(validateOtaPublishReceipt({ ...receipt, assetCount: 1.5 })).toBe(false)
    expect(validateOtaPublishReceipt({ ...receipt, assetCount: -1 })).toBe(false)
  })

  test("rejects extra fields so raw publish output cannot ride along", () => {
    const receipt = buildOtaPublishReceipt({
      runtimeVersion: "1.4.0",
      branch: "preview",
      updateGroupId: "update-group.public.pylon.extra",
      assetCount: 2,
      publishedAt: "2026-06-13T14:50:00.000Z",
    })

    expect(
      validateOtaPublishReceipt({
        ...receipt,
        rawEasOutput: "private publish log",
      }),
    ).toBe(false)
  })
})
