import { describe, expect, test } from "bun:test"
import { chooseUpdate, type UpdateManifest } from "../src/shared/update-feed"

describe("desktop update feed", () => {
  test("chooses a newer version as a full update", () => {
    const manifest = manifestFor("1.2.0")

    expect(chooseUpdate("1.1.0", [manifest])).toEqual({
      action: "full",
      manifest,
    })
  })

  test("chooses bsdiff when the newest patch starts from the current version", () => {
    const manifest: UpdateManifest = {
      ...manifestFor("1.2.0"),
      bsdiffFromVersion: "1.1.0",
      bsdiffUrl: "https://updates.openagents.com/autopilot/1.1.0-1.2.0.bsdiff",
    }

    expect(chooseUpdate("1.1.0", [manifest])).toEqual({
      action: "bsdiff",
      manifest,
    })
  })

  test("returns none when already up to date", () => {
    expect(chooseUpdate("1.2.0", [manifestFor("1.2.0")])).toEqual({
      action: "none",
    })
  })

  test("ignores older versions when choosing an update", () => {
    const older = manifestFor("1.0.0")
    const newer = manifestFor("1.2.10")

    expect(chooseUpdate("1.2.0", [older, newer])).toEqual({
      action: "full",
      manifest: newer,
    })
  })
})

function manifestFor(version: string): UpdateManifest {
  return {
    version,
    artifactUrl: `https://updates.openagents.com/autopilot/${version}.zip`,
    sha256: `${version}-sha256`,
  }
}
