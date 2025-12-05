import { describe, expect, it } from "bun:test";
import { mkdtempSync, writeFileSync } from "fs";
import { join } from "path";
import { SettingsManager } from "./settings-manager.js";
import { getUnseenEntries, parseChangelog, showChangelogOnStart } from "./changelog.js";

const sampleChangelog = `
# OpenAgents Changelog

## 0.2.0 (2025-02-01)
- Added new status stream options
- Improved retry/backoff defaults

## 0.1.0 (2025-01-15)
- Initial release with prompt preview and session logging
`;

describe("parseChangelog", () => {
  it("parses headings and bodies", () => {
    const entries = parseChangelog(sampleChangelog);
    expect(entries).toHaveLength(2);
    expect(entries[0]).toMatchObject({
      version: "0.2.0",
      date: "2025-02-01",
    });
    expect(entries[0].body).toContain("Added new status stream options");
    expect(entries[1].version).toBe("0.1.0");
  });
});

describe("getUnseenEntries", () => {
  it("returns entries newer than last seen", () => {
    const entries = parseChangelog(sampleChangelog);
    const unseen = getUnseenEntries(entries, "0.1.0");
    expect(unseen.map((entry) => entry.version)).toEqual(["0.2.0"]);
  });
});

describe("showChangelogOnStart", () => {
  it("prints new entries once and updates last seen", () => {
    const dir = mkdtempSync("/tmp/changelog-");
    const changelogPath = join(dir, "CHANGELOG.md");
    writeFileSync(changelogPath, sampleChangelog, "utf8");

    const settingsDir = join(dir, "settings");
    const settings = new SettingsManager(settingsDir);

    const printed: string[] = [];
    const firstRun = showChangelogOnStart({
      settingsManager: settings,
      changelogPath,
      log: (line) => printed.push(line),
    });

    expect(firstRun.displayed).toHaveLength(2);
    expect(printed.join("\n")).toContain("OpenAgents Changelog");
    expect(printed.join("\n")).toContain("0.2.0");
    expect(printed.join("\n")).toContain("0.1.0");
    expect(settings.getLastSeenChangelogVersion()).toBe("0.2.0");

    const secondPrinted: string[] = [];
    const secondRun = showChangelogOnStart({
      settingsManager: settings,
      changelogPath,
      log: (line) => secondPrinted.push(line),
    });

    expect(secondRun.displayed).toHaveLength(0);
    expect(secondPrinted).toHaveLength(0);

    const updatedChangelog = `
# OpenAgents Changelog

## 0.3.0 (2025-02-15)
- Fresh release details

## 0.2.0 (2025-02-01)
- Added new status stream options

## 0.1.0 (2025-01-15)
- Initial release with prompt preview and session logging
`;
    writeFileSync(changelogPath, updatedChangelog, "utf8");

    const thirdPrinted: string[] = [];
    const thirdRun = showChangelogOnStart({
      settingsManager: settings,
      changelogPath,
      log: (line) => thirdPrinted.push(line),
    });

    expect(thirdRun.displayed.map((entry) => entry.version)).toEqual(["0.3.0"]);
    expect(thirdPrinted.join("\n")).toContain("0.3.0");
    expect(thirdPrinted.join("\n")).not.toContain("0.2.0");
    expect(settings.getLastSeenChangelogVersion()).toBe("0.3.0");
  });
});
