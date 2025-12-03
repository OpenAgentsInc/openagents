import { describe, expect, it } from "bun:test";
import { mkdtempSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { loadCustomProviders, mergeCustomModels } from "./custom-providers.js";
import { MODELS } from "./models.generated.js";

describe("custom providers", () => {
  it("loads empty when file missing", () => {
    const cfg = loadCustomProviders("/nonexistent/path/models.json");
    expect(cfg.providers).toEqual({});
  });

  it("loads and merges custom provider models", () => {
    const dir = mkdtempSync(join(tmpdir(), "models-"));
    const path = join(dir, "models.json");
    writeFileSync(
      path,
      JSON.stringify({
        providers: {
          ollama: {
            baseUrl: "http://localhost:11434/v1",
            api: "openai-completions",
            models: [
              {
                id: "llama-3-8b",
                name: "Llama 3 8B",
                api: "openai-completions",
                provider: "ollama",
                baseUrl: "",
                reasoning: false,
                input: ["text"],
                cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
                contextWindow: 128000,
                maxTokens: 32000,
              },
            ],
          },
        },
      }),
      "utf8",
    );

    const cfg = loadCustomProviders(path);
    const merged = mergeCustomModels(MODELS as any, cfg);
    expect(merged.ollama["llama-3-8b"].baseUrl).toBe("http://localhost:11434/v1");
    expect(merged.ollama["llama-3-8b"].provider).toBe("ollama");
  });
});
