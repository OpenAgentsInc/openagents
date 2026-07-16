import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, test } from "vite-plus/test";

import { openAcpProviderPathStore } from "./acp-provider-path-store.ts";

describe("ACP alternate executable path store", () => {
  test("persists only absolute bounded provider paths in a private versioned file", async () => {
    const root = await mkdtemp(join(tmpdir(), "openagents-acp-paths-"));
    try {
      const file = join(root, "private", "paths.json");
      const store = openAcpProviderPathStore(file);
      await store.save("grok", "/opt/xai/grok");
      expect((await stat(file)).mode & 0o777).toBe(0o600);
      expect(await readFile(file, "utf8")).toContain("openagents.desktop.acp-paths.v1");
      const reopened = openAcpProviderPathStore(file);
      await reopened.load();
      expect(reopened.get("grok")).toBe("/opt/xai/grok");
      await expect(reopened.save("cursor", "relative/agent")).rejects.toThrow("invalid alternate");
      await reopened.clear("grok");
      expect(reopened.get("grok")).toBeUndefined();
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("drops malformed and relative persisted candidates", async () => {
    const store = openAcpProviderPathStore("/path/that/does/not/exist/provider-paths.json");
    await store.load();
    expect(store.get("grok")).toBeUndefined();
    expect(store.get("cursor")).toBeUndefined();
  });
});
