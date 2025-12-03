import { describe, expect, it } from "bun:test";
import { cacheApiKey, resolveApiKey } from "./api-key.js";
import { mkdtempSync, readFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";

describe("api key resolution", () => {
  it("prefers explicit apiKey", () => {
    const key = resolveApiKey("openai", "explicit-key");
    expect(key).toBe("explicit-key");
  });

  it("reads cached key", () => {
    const dir = mkdtempSync(join(tmpdir(), "keys-"));
    process.env.PI_CODING_AGENT_DIR = dir;
    cacheApiKey("openai", "cached-key");
    const raw = readFileSync(join(dir, "keys.json"), "utf8");
    expect(raw).toContain("cached-key");
    const key = resolveApiKey("openai");
    expect(key).toBe("cached-key");
    delete process.env.PI_CODING_AGENT_DIR;
  });
});
