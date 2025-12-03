import { describe, expect, it } from "bun:test";
import { mkdtempSync, readFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { loadOAuthToken, resolveAnthropicAuth, saveOAuthToken } from "./oauth.js";

describe("oauth", () => {
  it("saves and loads token", () => {
    const dir = mkdtempSync(join(tmpdir(), "oauth-"));
    const path = join(dir, "oauth.json");
    saveOAuthToken({ provider: "anthropic", accessToken: "token-123", expiresAt: Date.now() + 1000 }, path);
    const raw = readFileSync(path, "utf8");
    expect(raw).toContain("token-123");

    const loaded = loadOAuthToken(path);
    expect(loaded?.accessToken).toBe("token-123");
  });

  it("returns null when expired", () => {
    const dir = mkdtempSync(join(tmpdir(), "oauth-"));
    const path = join(dir, "oauth.json");
    saveOAuthToken({ provider: "anthropic", accessToken: "expired", expiresAt: Date.now() - 1 }, path);
    expect(loadOAuthToken(path)).toBeNull();
  });

  it("resolves env first", () => {
    process.env.ANTHROPIC_API_KEY = "env-key";
    expect(resolveAnthropicAuth()).toBe("env-key");
    delete process.env.ANTHROPIC_API_KEY;
  });
});
