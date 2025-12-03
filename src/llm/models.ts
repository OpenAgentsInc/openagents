import type { Api, Model, Provider, Usage } from "./model-types.js";
import { MODELS } from "./models.generated.js";
import { loadCustomProviders, mergeCustomModels } from "./custom-providers.js";

const custom = mergeCustomModels(MODELS as any, loadCustomProviders());
const registry: Map<Provider, Map<string, Model<Api>>> = new Map();

for (const [provider, models] of Object.entries(custom)) {
  const providerModels = new Map<string, Model<Api>>();
  for (const [id, model] of Object.entries(models)) {
    providerModels.set(id, model as Model<Api>);
  }
  registry.set(provider as Provider, providerModels);
}

type ProviderKey = keyof typeof MODELS;

type ModelApi<
  TProvider extends ProviderKey,
  TModelId extends keyof (typeof MODELS)[TProvider],
> = (typeof MODELS)[TProvider][TModelId] extends { api: infer TApi } ? (TApi extends Api ? TApi : never) : never;

export const getProviders = (): Provider[] => Array.from(registry.keys());

export function getModel<TProvider extends ProviderKey, TModelId extends keyof (typeof MODELS)[TProvider]>(
  provider: TProvider,
  modelId: TModelId,
): Model<ModelApi<TProvider, TModelId>> {
  return registry.get(provider)?.get(modelId as string) as Model<ModelApi<TProvider, TModelId>>;
}

export function getModels<TProvider extends ProviderKey>(
  provider: TProvider,
): Array<Model<ModelApi<TProvider, keyof (typeof MODELS)[TProvider]>>> {
  const models = registry.get(provider);
  return models ? (Array.from(models.values()) as Array<Model<ModelApi<TProvider, keyof (typeof MODELS)[TProvider]>>>) : [];
}

export function calculateCost<TApi extends Api>(model: Model<TApi>, usage: Usage): Usage["cost"] {
  usage.cost.input = (model.cost.input / 1_000_000) * usage.input;
  usage.cost.output = (model.cost.output / 1_000_000) * usage.output;
  usage.cost.cacheRead = (model.cost.cacheRead / 1_000_000) * usage.cacheRead;
  usage.cost.cacheWrite = (model.cost.cacheWrite / 1_000_000) * usage.cacheWrite;
  usage.cost.total = usage.cost.input + usage.cost.output + usage.cost.cacheRead + usage.cost.cacheWrite;
  return usage.cost;
}
