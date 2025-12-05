import { describe, expect, it } from "bun:test";
import { mkdtempSync, readFileSync } from "fs";
import { join } from "path";
import { SettingsManager } from "./settings-manager.js";

describe("SettingsManager", () => {
  it("persists defaults to settings.json", () => {
    const dir = mkdtempSync("/tmp/settings-");
    const mgr = new SettingsManager(dir);
    mgr.setDefaultModelAndProvider("openai", "gpt-5");
    mgr.setQueueMode("all");
    mgr.setDefaultThinkingLevel("medium");
    mgr.setTheme("dark");
    mgr.setLastSeenChangelogVersion("1.2.3");

    const raw = readFileSync(join(dir, "settings.json"), "utf8");
    expect(raw).toContain("gpt-5");

    const reloaded = new SettingsManager(dir);
    expect(reloaded.getDefaultProvider()).toBe("openai");
    expect(reloaded.getDefaultModel()).toBe("gpt-5");
    expect(reloaded.getQueueMode()).toBe("all");
    expect(reloaded.getDefaultThinkingLevel()).toBe("medium");
    expect(reloaded.getTheme()).toBe("dark");
    expect(reloaded.getLastSeenChangelogVersion()).toBe("1.2.3");
  });
});
