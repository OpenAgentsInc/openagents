// Khala config resolver tests (fakes, no network, no real secrets): the
// credential resolution order is OPENAGENTS_API_KEY -> discovered agent token ->
// OpenAI-compatible fallback, and a missing credential fails loudly. The
// credential VALUE is never returned in a log; only the source LABEL is.

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { makeOpenRouterClientMock } from "@openagentsinc/probe-runtime/openrouter";
import { makeKhalaChatClient, resolveKhalaConfig } from "./khala-config";

let secretsDir: string;
beforeEach(() => {
  secretsDir = mkdtempSync(join(tmpdir(), "khala-config-test-"));
});
afterEach(() => {
  rmSync(secretsDir, { recursive: true, force: true });
});

describe("resolveKhalaConfig", () => {
  test("prefers OPENAGENTS_API_KEY -> real openagents/khala", () => {
    const cfg = resolveKhalaConfig({ env: { OPENAGENTS_API_KEY: "oa-key" }, secretsDir });
    expect(cfg.mode).toBe("khala");
    expect(cfg.model).toBe("openagents/khala");
    expect(cfg.baseUrl).toBe("https://openagents.com/api/v1");
    expect(cfg.keySource).toBe("OPENAGENTS_API_KEY env");
    expect(cfg.apiKey).toBe("oa-key");
  });

  test("discovers an OpenAgents agent token in the secrets dir -> real khala", () => {
    writeFileSync(join(secretsDir, "some-agent.env"), 'OPENAGENTS_AGENT_TOKEN="agent-tok"\n');
    const cfg = resolveKhalaConfig({ env: {}, secretsDir });
    expect(cfg.mode).toBe("khala");
    expect(cfg.apiKey).toBe("agent-tok");
    expect(cfg.keySource).toContain("some-agent.env");
  });

  test("falls back to PROBE_OPENAI_API_KEY (loop-proof only), clearly labeled", () => {
    const cfg = resolveKhalaConfig({ env: { PROBE_OPENAI_API_KEY: "sk-fallback" }, secretsDir });
    expect(cfg.mode).toBe("fallback");
    expect(cfg.apiKey).toBe("sk-fallback");
    expect(cfg.baseUrl).toBe("https://api.openai.com/v1");
    expect(cfg.keySource).toBe("PROBE_OPENAI_API_KEY env");
  });

  test("falls back to a probe-openai.env file in the secrets dir", () => {
    writeFileSync(join(secretsDir, "probe-openai.env"), "PROBE_OPENAI_API_KEY=sk-from-file\n");
    const cfg = resolveKhalaConfig({ env: {}, secretsDir });
    expect(cfg.mode).toBe("fallback");
    expect(cfg.apiKey).toBe("sk-from-file");
    expect(cfg.keySource).toContain("probe-openai.env");
  });

  test("env model/base overrides are honored", () => {
    const cfg = resolveKhalaConfig({
      env: { OPENAGENTS_API_KEY: "k", KHALA_MODEL: "openagents/khala", KHALA_BASE_URL: "https://staging.example/api/v1" },
      secretsDir,
    });
    expect(cfg.baseUrl).toBe("https://staging.example/api/v1");
  });

  test("throws loudly when no credential is available (never pretends to run)", () => {
    expect(() => resolveKhalaConfig({ env: {}, secretsDir })).toThrow(/no Khala credential/);
  });

  test("--no-fallback (allowFallback:false) refuses the OpenAI fallback", () => {
    expect(() =>
      resolveKhalaConfig({ env: { PROBE_OPENAI_API_KEY: "sk-x" }, secretsDir, allowFallback: false }),
    ).toThrow(/no Khala credential/);
  });
});

describe("makeKhalaChatClient backend routing (no network)", () => {
  test("routes to the gpt-oss fetch lane by default (OpenRouter not armed)", () => {
    const { selection, chat } = makeKhalaChatClient({
      env: { OPENAGENTS_API_KEY: "oa-key" },
      secretsDir,
    });
    expect(selection.backend).toBe("gpt-oss");
    expect(typeof chat.complete).toBe("function");
  });

  test("routes to the OpenRouter lane when armed; uses the injected mock layer (no spend)", async () => {
    const layer = makeOpenRouterClientMock({ replies: [{ content: "or-mock-ok" }] });
    const { selection, chat } = makeKhalaChatClient({
      env: { KHALA_DRIVER_BACKEND: "openrouter" },
      secretsDir,
      openrouter: { layer },
    });
    expect(selection.backend).toBe("openrouter");
    expect(await chat.complete([{ role: "user", content: "go" }])).toBe("or-mock-ok");
  });

  test("the OpenRouter lane does not require a Khala credential to be present", () => {
    // No OPENAGENTS_API_KEY / no fallback: gpt-oss would throw, but OpenRouter
    // routing must succeed (it has its own credential path, mocked here).
    const layer = makeOpenRouterClientMock({ replies: [{ content: "ok" }] });
    expect(() =>
      makeKhalaChatClient({ env: { OPENROUTER_LIVE: "1" }, secretsDir, openrouter: { layer } }),
    ).not.toThrow();
  });
});
