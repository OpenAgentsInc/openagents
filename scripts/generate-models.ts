#!/usr/bin/env bun
import { writeFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import type { Api, Model, Provider } from "../src/llm/model-types.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = join(__dirname, "..");

type InputModality = "text" | "image";

interface ModelsDevModel {
  id: string;
  name?: string;
  tool_call?: boolean;
  reasoning?: boolean;
  limit?: {
    context?: number;
    output?: number;
  };
  cost?: {
    input?: number;
    output?: number;
    cache_read?: number;
    cache_write?: number;
  };
  modalities?: {
    input?: string[];
  };
}

interface ModelsDevResponse {
  [provider: string]: {
    models?: Record<string, ModelsDevModel>;
  };
}

const MODELS_DEV_ENDPOINT = "https://models.dev/api.json";
const OPENROUTER_ENDPOINT = "https://openrouter.ai/api/v1/models";

const DEFAULT_CONTEXT = 4096;
const DEFAULT_MAX_TOKENS = 4096;

const parseModalities = (modalities?: { input?: string[] }): InputModality[] => {
  const inputs: InputModality[] = ["text"];
  if (modalities?.input?.includes("image")) {
    inputs.push("image");
  }
  return inputs;
};

const toMillion = (value?: string | number): number => {
  if (value === undefined) return 0;
  const num = typeof value === "string" ? parseFloat(value) : value;
  if (Number.isNaN(num)) return 0;
  return num * 1_000_000;
};

const normalizeModelsDev = (provider: Provider, api: Api, baseUrl: string, data?: Record<string, ModelsDevModel>) => {
  if (!data) return [] as Model<Api>[];

  const models: Model<Api>[] = [];
  for (const [modelId, model] of Object.entries(data)) {
    if (!model.tool_call) continue;

    models.push({
      id: modelId,
      name: model.name || modelId,
      api,
      provider,
      baseUrl,
      reasoning: model.reasoning === true,
      input: parseModalities(model.modalities),
      cost: {
        input: model.cost?.input ?? 0,
        output: model.cost?.output ?? 0,
        cacheRead: model.cost?.cache_read ?? 0,
        cacheWrite: model.cost?.cache_write ?? 0,
      },
      contextWindow: model.limit?.context ?? DEFAULT_CONTEXT,
      maxTokens: model.limit?.output ?? DEFAULT_MAX_TOKENS,
    });
  }
  return models;
};

const fetchModelsDev = async (): Promise<Model<Api>[]> => {
  try {
    const res = await fetch(MODELS_DEV_ENDPOINT);
    if (!res.ok) throw new Error(`models.dev request failed: ${res.status}`);
    const data = (await res.json()) as ModelsDevResponse;

    const models: Model<Api>[] = [];
    models.push(
      ...normalizeModelsDev("anthropic", "anthropic-messages", "https://api.anthropic.com", data.anthropic?.models),
    );
    models.push(...normalizeModelsDev("google", "google-generative-ai", "https://generativelanguage.googleapis.com/v1beta", data.google?.models));
    models.push(...normalizeModelsDev("openai", "openai-responses", "https://api.openai.com/v1", data.openai?.models));
    models.push(...normalizeModelsDev("groq", "openai-completions", "https://api.groq.com/openai/v1", data.groq?.models));
    models.push(...normalizeModelsDev("cerebras", "openai-completions", "https://api.cerebras.ai/v1", data.cerebras?.models));
    models.push(...normalizeModelsDev("xai", "openai-completions", "https://api.x.ai/v1", data.xai?.models));
    models.push(...normalizeModelsDev("zai", "anthropic-messages", "https://api.z.ai/api/anthropic", data.zai?.models));
    return models;
  } catch (err) {
    console.error("Failed to load models.dev data:", err);
    return [];
  }
};

const fetchOpenRouterModels = async (): Promise<Model<Api>[]> => {
  try {
    const res = await fetch(OPENROUTER_ENDPOINT);
    if (!res.ok) throw new Error(`OpenRouter request failed: ${res.status}`);
    const data = (await res.json()) as { data: Array<any> };

    const models: Model<Api>[] = [];
    for (const model of data.data) {
      if (!model.supported_parameters?.includes("tools")) continue;

      const input: InputModality[] = ["text"];
      if (model.architecture?.modality?.includes("image")) {
        input.push("image");
      }

      const normalized: Model<Api> = {
        id: model.id,
        name: model.name,
        api: "openai-completions",
        provider: "openrouter",
        baseUrl: "https://openrouter.ai/api/v1",
        reasoning: model.supported_parameters?.includes("reasoning") || false,
        input,
        cost: {
          input: toMillion(model.pricing?.prompt),
          output: toMillion(model.pricing?.completion),
          cacheRead: toMillion(model.pricing?.input_cache_read),
          cacheWrite: toMillion(model.pricing?.input_cache_write),
        },
        contextWindow: model.context_length || DEFAULT_CONTEXT,
        maxTokens: model.top_provider?.max_completion_tokens || DEFAULT_MAX_TOKENS,
      };
      models.push(normalized);
    }

    return models;
  } catch (err) {
    console.error("Failed to load OpenRouter data:", err);
    return [];
  }
};

const ensureOverrides = (models: Model<Api>[]) => {
  const pushIfMissing = (match: (m: Model<Api>) => boolean, model: Model<Api>) => {
    if (!models.some(match)) {
      models.push(model);
    }
  };

  // Correct cache pricing for claude-opus-4-5 if present
  const opus45 = models.find((m) => m.provider === "anthropic" && m.id === "claude-opus-4-5");
  if (opus45) {
    opus45.cost.cacheRead = 0.5;
    opus45.cost.cacheWrite = 6.25;
  }

  pushIfMissing(
    (m) => m.provider === "openai" && m.id === "gpt-5-chat-latest",
    {
      id: "gpt-5-chat-latest",
      name: "GPT-5 Chat Latest",
      api: "openai-responses",
      provider: "openai",
      baseUrl: "https://api.openai.com/v1",
      reasoning: false,
      input: ["text", "image"],
      cost: { input: 1.25, output: 10, cacheRead: 0.125, cacheWrite: 0 },
      contextWindow: 128000,
      maxTokens: 16384,
    },
  );

  pushIfMissing(
    (m) => m.provider === "openai" && m.id === "gpt-5.1-codex",
    {
      id: "gpt-5.1-codex",
      name: "GPT-5.1 Codex",
      api: "openai-responses",
      provider: "openai",
      baseUrl: "https://api.openai.com/v1",
      reasoning: true,
      input: ["text", "image"],
      cost: { input: 1.25, output: 5, cacheRead: 0.125, cacheWrite: 1.25 },
      contextWindow: 400000,
      maxTokens: 128000,
    },
  );

  pushIfMissing(
    (m) => m.provider === "xai" && m.id === "grok-code-fast-1",
    {
      id: "grok-code-fast-1",
      name: "Grok Code Fast 1",
      api: "openai-completions",
      provider: "xai",
      baseUrl: "https://api.x.ai/v1",
      reasoning: false,
      input: ["text"],
      cost: { input: 0.2, output: 1.5, cacheRead: 0.02, cacheWrite: 0 },
      contextWindow: 32768,
      maxTokens: 8192,
    },
  );

  pushIfMissing(
    (m) => m.provider === "openrouter" && m.id === "openrouter/auto",
    {
      id: "openrouter/auto",
      name: "OpenRouter: Auto Router",
      api: "openai-completions",
      provider: "openrouter",
      baseUrl: "https://openrouter.ai/api/v1",
      reasoning: true,
      input: ["text", "image"],
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      contextWindow: 2_000_000,
      maxTokens: 30000,
    },
  );
};

const groupByProvider = (models: Model<Api>[]) => {
  const providers: Record<string, Record<string, Model<Api>>> = {};
  for (const model of models) {
    providers[model.provider] ??= {};
    if (!providers[model.provider][model.id]) {
      providers[model.provider][model.id] = model;
    }
  }
  return providers;
};

const emitModelsFile = (providers: Record<string, Record<string, Model<Api>>>) => {
  let output = `// This file is auto-generated by scripts/generate-models.ts\n// Do not edit manually - run 'bun scripts/generate-models.ts' to update\n\nimport type { Model } from "./model-types.js";\n\nexport const MODELS = {\n`;

  for (const [providerId, models] of Object.entries(providers)) {
    output += `  ${providerId}: {\n`;
    for (const model of Object.values(models)) {
      output += `    "${model.id}": {\n`;
      output += `      id: "${model.id}",\n`;
      output += `      name: "${model.name}",\n`;
      output += `      api: "${model.api}",\n`;
      output += `      provider: "${model.provider}",\n`;
      output += `      baseUrl: "${model.baseUrl}",\n`;
      output += `      reasoning: ${model.reasoning},\n`;
      output += `      input: [${model.input.map((i) => `"${i}"`).join(", ")}],\n`;
      output += `      cost: {\n`;
      output += `        input: ${model.cost.input},\n`;
      output += `        output: ${model.cost.output},\n`;
      output += `        cacheRead: ${model.cost.cacheRead},\n`;
      output += `        cacheWrite: ${model.cost.cacheWrite},\n`;
      output += `      },\n`;
      output += `      contextWindow: ${model.contextWindow},\n`;
      output += `      maxTokens: ${model.maxTokens},\n`;
      output += `    } satisfies Model<"${model.api}">,\n`;
    }
    output += `  },\n`;
  }

  output += `} as const;\n`;

  const target = join(repoRoot, "src", "llm", "models.generated.ts");
  writeFileSync(target, output);
  console.log(`Generated ${target}`);
};

async function main() {
  const modelsDev = await fetchModelsDev();
  const openRouter = await fetchOpenRouterModels();
  const allModels = [...modelsDev, ...openRouter];
  ensureOverrides(allModels);
  const providers = groupByProvider(allModels);
  emitModelsFile(providers);

  const total = allModels.length;
  const reasoning = allModels.filter((m) => m.reasoning).length;
  console.log(`Total tool-capable models: ${total}`);
  console.log(`Reasoning-capable models: ${reasoning}`);
  for (const [provider, models] of Object.entries(providers)) {
    console.log(`  ${provider}: ${Object.keys(models).length} models`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
