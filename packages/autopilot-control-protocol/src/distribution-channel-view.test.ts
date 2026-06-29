import { describe, expect, test } from "bun:test"

import { projectDistributionChannels } from "./distribution-channel-view.js"

describe("distribution channel view projection", () => {
  test("projects direct channel arrays", () => {
    expect(
      projectDistributionChannels([
        { target: "desktop", latestVersion: "1.2.3", state: "published" },
        { target: "mobile", latestVersion: "2.0.0", state: "pending_store" },
        { target: "ota", latestVersion: "2026.06.13", state: "ready" },
      ]),
    ).toEqual({
      channels: [
        { target: "desktop", latestVersion: "1.2.3", state: "published" },
        { target: "mobile", latestVersion: "2.0.0", state: "pending_store" },
        { target: "ota", latestVersion: "2026.06.13", state: "ready" },
      ],
      total: 3,
    })
  })

  test("reads wrapped channel rows and snake case aliases", () => {
    expect(
      projectDistributionChannels({
        distribution_channels: [
          { distribution_target: "desktop", latest_version: " 1.2.4 ", state: " stable " },
          { platform: "ota", current_version: "abc123", state: "published" },
        ],
      }),
    ).toEqual({
      channels: [
        { target: "desktop", latestVersion: "1.2.4", state: "stable" },
        { target: "ota", latestVersion: "abc123", state: "published" },
      ],
      total: 2,
    })
  })

  test("reads target-keyed channel records", () => {
    expect(
      projectDistributionChannels({
        desktop: { version: "3.0.0", state: "published" },
        mobile: { releaseVersion: "3.0.1", state: "store_review" },
        ota: { currentVersion: "bundle-7", state: "live" },
      }),
    ).toEqual({
      channels: [
        { target: "desktop", latestVersion: "3.0.0", state: "published" },
        { target: "mobile", latestVersion: "3.0.1", state: "store_review" },
        { target: "ota", latestVersion: "bundle-7", state: "live" },
      ],
      total: 3,
    })
  })

  test("skips malformed rows and unknown targets", () => {
    expect(
      projectDistributionChannels({
        channels: [
          null,
          { target: "web", latestVersion: "1.0.0", state: "published" },
          { target: 42, latestVersion: "1.0.0", state: "published" },
          { target: "MOBILE", latestVersion: "1.0.1", state: "published" },
        ],
      }),
    ).toEqual({
      channels: [{ target: "mobile", latestVersion: "1.0.1", state: "published" }],
      total: 1,
    })
  })

  test("uses stable fallbacks for missing versions and states", () => {
    expect(
      projectDistributionChannels([
        { target: "desktop", latestVersion: "", state: "" },
        { target: "ota", latestVersion: 12, state: false },
      ]),
    ).toEqual({
      channels: [
        { target: "desktop", latestVersion: null, state: "unknown" },
        { target: "ota", latestVersion: null, state: "unknown" },
      ],
      total: 2,
    })
  })

  test("returns an empty projection for non-list input", () => {
    expect(projectDistributionChannels("not channels")).toEqual({
      channels: [],
      total: 0,
    })
  })

  test("does not mutate the provided rows", () => {
    const raw = {
      desktop: { latestVersion: "1.0.0", state: "published" },
      channels: [{ target: "mobile", latestVersion: "1.0.1", state: "published" }],
    }

    projectDistributionChannels(raw)

    expect(raw).toEqual({
      desktop: { latestVersion: "1.0.0", state: "published" },
      channels: [{ target: "mobile", latestVersion: "1.0.1", state: "published" }],
    })
  })
})
