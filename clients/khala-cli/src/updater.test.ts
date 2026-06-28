import { describe, expect, test } from "bun:test"
import { changelogTeaser, firstWords, formatKhalaChangelog, formatReleaseTimestamp } from "./changelog.js"
import { compareVersions, runKhalaAutoUpdate, startKhalaAutoUpdate } from "./updater.js"

describe("Khala changelog formatting", () => {
  test("includes release timestamps in the selected timezone", () => {
    expect(formatReleaseTimestamp("2026-06-26T16:38:47.676Z", "America/Chicago"))
      .toBe("Jun 26, 2026, 11:38:47 AM CDT")
    expect(formatKhalaChangelog(1, { timeZone: "America/Chicago" })).toContain("CDT")
  })

  test("extracts a short update teaser from the package readme", () => {
    const readme = [
      "## Changelog",
      "",
      "### v0.1.4 - Jun 26, 2026, 12:00:00 PM CDT",
      "",
      "- Added one two three four five six seven eight nine ten eleven.",
      "",
      "### v0.1.3 - Jun 26, 2026, 11:50:00 AM CDT",
      "",
      "- Previous.",
    ].join("\n")

    expect(changelogTeaser("0.1.4", readme)).toBe("Added one two three four five six seven eight nine ten eleven.")
    expect(firstWords(changelogTeaser("0.1.4", readme) ?? "", 10)).toBe("Added one two three four five six seven eight nine")
  })
})

describe("Khala auto-update", () => {
  test("compares semver-shaped versions", () => {
    expect(compareVersions("0.1.4", "0.1.3")).toBe(1)
    expect(compareVersions("0.1.3", "0.1.3")).toBe(0)
    expect(compareVersions("0.1.2", "0.1.3")).toBe(-1)
  })

  test("installs a newer npm package and reports a restart line", async () => {
    const commands: Array<ReadonlyArray<string>> = []
    const notices: Array<string> = []
    const fakeFetch = (async () => Response.json({
      version: "0.1.4",
      readme: [
        "## Changelog",
        "",
        "### v0.1.4 - Jun 26, 2026, 12:00:00 PM CDT",
        "",
        "- Added quiet background update downloads for terminal sessions.",
      ].join("\n"),
    })) as unknown as typeof fetch

    const result = await runKhalaAutoUpdate({
      currentVersion: "0.1.3",
      fetch: fakeFetch,
      spawnInstall: command => {
        commands.push(command)
        return { exited: Promise.resolve(0) }
      },
    })

    expect(result).toEqual({
      kind: "installed",
      latestVersion: "0.1.4",
      summary: "Added quiet background update downloads for terminal sessions.",
    })
    expect(commands).toEqual([["npm", "i", "-g", "@openagentsinc/khala@latest"]])

    startKhalaAutoUpdate({
      currentVersion: "0.1.3",
      fetch: fakeFetch,
      notify: line => notices.push(line),
      spawnInstall: command => {
        commands.push(command)
        return { exited: Promise.resolve(0) }
      },
    })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(notices).toEqual([
      "update added - Added quiet background update downloads for terminal sessions. - restart to apply",
    ])
  })

  test("defers update notices until the interactive CLI flushes them", async () => {
    const notices: Array<string> = []
    const fakeFetch = (async () => Response.json({
      version: "0.1.4",
      readme: [
        "## Changelog",
        "",
        "### v0.1.4 - Jun 26, 2026, 12:00:00 PM CDT",
        "",
        "- Added quiet background update downloads for terminal sessions.",
      ].join("\n"),
    })) as unknown as typeof fetch

    const handle = startKhalaAutoUpdate({
      currentVersion: "0.1.3",
      fetch: fakeFetch,
      notify: line => notices.push(line),
      notifyMode: "defer",
      spawnInstall: () => ({ exited: Promise.resolve(0) }),
    })

    await handle.done

    expect(notices).toEqual([])
    expect(handle.pendingNotificationCount).toBe(1)
    expect(handle.flushNotifications()).toBe(1)
    expect(notices).toEqual([
      "update added - Added quiet background update downloads for terminal sessions. - restart to apply",
    ])
    expect(handle.pendingNotificationCount).toBe(0)
    expect(handle.flushNotifications()).toBe(0)
  })

  test("does nothing when auto-update is disabled", async () => {
    const result = await runKhalaAutoUpdate({
      currentVersion: "0.1.3",
      env: { KHALA_NO_AUTO_UPDATE: "1" },
      fetch: (async () => {
        throw new Error("should not fetch")
      }) as unknown as typeof fetch,
    })

    expect(result).toEqual({ kind: "disabled" })
  })
})
