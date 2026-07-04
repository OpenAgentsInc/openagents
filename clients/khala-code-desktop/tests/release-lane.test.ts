import { describe, expect, test } from "bun:test"

import {
  buildKhalaCodeDesktopReleasePlan,
  validateKhalaCodeDesktopReleaseReceipts,
} from "../src/shared/release-lane"

describe("Khala Code Desktop release lane", () => {
  test("builds an RC plan on the product-specific updates feed", () => {
    expect(
      buildKhalaCodeDesktopReleasePlan({
        version: "0.1.0-rc.1",
        channel: "rc",
        artifactFileName: "Khala-Code-0.1.0-rc.1.dmg",
      }),
    ).toEqual({
      product: "khala-code-desktop",
      version: "0.1.0-rc.1",
      channel: "rc",
      artifactFileName: "Khala-Code-0.1.0-rc.1.dmg",
      githubTag: "khala-code-desktop-v0.1.0-rc.1",
      githubPrerelease: true,
      latestEligible: false,
      updateFeedUrl:
        "https://updates.openagents.com/desktop/khala-code-desktop/rc/feed.json",
      updateFeedBucketPrefix:
        "gs://openagentsgemini-oa-updates/desktop/khala-code-desktop/rc/",
      needsOwnerRef: "NEEDS_OWNER.md#khala-code-desktop-signed-release-gate",
    })
  })

  test("marks stable non-prerelease builds as latest eligible", () => {
    const plan = buildKhalaCodeDesktopReleasePlan({
      version: "0.1.0",
      channel: "stable",
      artifactFileName: "Khala-Code-0.1.0.dmg",
    })

    expect(plan.githubTag).toBe("khala-code-desktop-v0.1.0")
    expect(plan.githubPrerelease).toBe(false)
    expect(plan.latestEligible).toBe(true)
  })

  test("rejects RC builds from the stable/latest lane", () => {
    expect(() =>
      buildKhalaCodeDesktopReleasePlan({
        version: "0.1.0-rc.1",
        channel: "stable",
        artifactFileName: "Khala-Code-0.1.0-rc.1.dmg",
      }),
    ).toThrow("RC builds must not publish to stable")
  })

  test("requires release artifacts to be DMGs", () => {
    expect(() =>
      buildKhalaCodeDesktopReleasePlan({
        version: "0.1.0-rc.1",
        channel: "rc",
        artifactFileName: "Khala-Code-0.1.0-rc.1.zip",
      }),
    ).toThrow("must be a DMG")
  })

  test("requires every owner receipt before RL-1 can be called complete", () => {
    expect(validateKhalaCodeDesktopReleaseReceipts({})).toEqual({
      ok: false,
      missing: [
        "signedAppReceiptRef",
        "notarizedAppReceiptRef",
        "stapledAppReceiptRef",
        "recreatedDmgReceiptRef",
        "signedDmgReceiptRef",
        "notarizedDmgReceiptRef",
        "stapledDmgReceiptRef",
        "feedUploadReceiptRef",
        "githubReleaseReceiptRef",
        "cleanMacFirstRunSmokeReceiptRef",
      ],
    })

    expect(
      validateKhalaCodeDesktopReleaseReceipts({
        signedAppReceiptRef: "codesign://khala-app",
        notarizedAppReceiptRef: "notary://khala-app",
        stapledAppReceiptRef: "stapler://khala-app",
        recreatedDmgReceiptRef: "hdiutil://khala-dmg",
        signedDmgReceiptRef: "codesign://khala-dmg",
        notarizedDmgReceiptRef: "notary://khala-dmg",
        stapledDmgReceiptRef: "stapler://khala-dmg",
        feedUploadReceiptRef: "gs://openagentsgemini-oa-updates/desktop/khala-code-desktop/rc/",
        githubReleaseReceiptRef:
          "https://github.com/OpenAgentsInc/openagents/releases/tag/khala-code-desktop-v0.1.0-rc.1",
        cleanMacFirstRunSmokeReceiptRef:
          "evidence://khala-code-clean-mac-missing-codex",
      }),
    ).toEqual({ ok: true, missing: [] })
  })
})
