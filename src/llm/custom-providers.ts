import { existsSync, readFileSync } from "fs";
import { homedir } from "os";
import { join, resolve } from "path";
import type { Api, Model, Provider } from "./model-types.js";

interface CustomModelFile {
  providers?: Record<
    string,
    {
      baseUrl?: string;
      apiKey?: string;
      api?: Api;
      headers?: Record<string, string>;
      models?: Array<Model<Api>>;
    }
  >;
}

const defaultConfigPath = () => {
  const base = process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent");
  return resolve(base, "models.json");
};

export const loadCustomProviders = (path = defaultConfigPath()): CustomModelFile => {
  if (!existsSync(path)) return { providers: {} };
  try {
    const raw = readFileSync(path, "utf8");
    const parsed = JSON.parse(raw) as CustomModelFile;
    return parsed.providers ? parsed : { providers: {} };
  } catch {
    return { providers: {} };
  }
};

export const mergeCustomModels = (
  base: Record<Provider, Record<string, Model<Api>>>,
  custom: CustomModelFile,
): Record<Provider, Record<string, Model<Api>>> => {
  if (!custom.providers) return base;
  const merged: Record<Provider, Record<string, Model<Api>>> = { ...base };

  for (const [provider, cfg] of Object.entries(custom.providers)) {
    if (!cfg.models || cfg.models.length === 0) continue;
    const bucket = (merged as any)[provider] ?? {};
    for (const model of cfg.models) {
      bucket[model.id] = {
        ...model,
        provider,
        baseUrl: model.baseUrl || cfg.baseUrl || "",
        api: model.api || (cfg.api as Api) || model.api,
        headers: model.headers || cfg.headers,
      };
      if (cfg.baseUrl && !bucket[model.id].baseUrl) {
        bucket[model.id].baseUrl = cfg.baseUrl;
      }
    }
    (merged as any)[provider] = bucket;
  }

  return merged;
};
