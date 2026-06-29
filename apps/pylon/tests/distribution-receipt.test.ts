import { describe, expect, test } from "bun:test"

import {
  buildDistributionReceipt,
  validateDistributionReceipt,
  type DistributionReceipt,
} from "../src/coordinator/distribution-receipt"

const desktopReceipt: DistributionReceipt = {
  target: "desktop",
  artifactRef: "artifact.desktop.pylon-20260613",
  version: "2026.06.13.1",
  distributedAt: "2026-06-13T16:00:00.000Z",
}

describe("distribution receipt", () => {
  test("builds a normalized desktop receipt", () => {
    const receipt = buildDistributionReceipt({
      target: "desktop",
      artifactRef: " artifact.desktop.pylon-20260613 ",
      version: " 2026.06.13.1 ",
      distributedAt: " 2026-06-13T16:00:00.000Z ",
    })

    expect(receipt).toEqual(desktopReceipt)
    expect(validateDistributionReceipt(receipt)).toBe(true)
  })

  test("builds valid mobile and OTA receipts", () => {
    const mobileReceipt = buildDistributionReceipt({
      target: "mobile",
      artifactRef: "artifact.mobile.pylon-ios-20260613",
      version: "2026.06.13.2",
      distributedAt: "2026-06-13T16:05:00.000Z",
    })
    const otaReceipt = buildDistributionReceipt({
      target: "ota",
      artifactRef: "artifact.ota.update-group.public.pylon",
      version: "1.4.0",
      distributedAt: "2026-06-13T16:10:00.000Z",
    })

    expect(validateDistributionReceipt(mobileReceipt)).toBe(true)
    expect(validateDistributionReceipt(otaReceipt)).toBe(true)
  })

  test("rejects invalid targets", () => {
    expect(validateDistributionReceipt({ ...desktopReceipt, target: "web" })).toBe(false)
    expect(validateDistributionReceipt({ ...desktopReceipt, target: undefined })).toBe(false)
  })

  test("rejects blank required strings", () => {
    expect(validateDistributionReceipt({ ...desktopReceipt, artifactRef: "" })).toBe(false)
    expect(validateDistributionReceipt({ ...desktopReceipt, version: " " })).toBe(false)
    expect(validateDistributionReceipt({ ...desktopReceipt, distributedAt: "" })).toBe(false)
  })

  test("rejects missing fields", () => {
    expect(validateDistributionReceipt({ ...desktopReceipt, artifactRef: undefined })).toBe(false)
    expect(validateDistributionReceipt({ ...desktopReceipt, version: undefined })).toBe(false)
    expect(validateDistributionReceipt({ ...desktopReceipt, distributedAt: undefined })).toBe(false)
  })

  test("rejects arrays and extra fields", () => {
    expect(validateDistributionReceipt([desktopReceipt])).toBe(false)
    expect(
      validateDistributionReceipt({
        ...desktopReceipt,
        rawDistributorOutput: "private deployment log",
      }),
    ).toBe(false)
  })
})
