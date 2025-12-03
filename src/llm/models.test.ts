import { describe, expect, it } from "bun:test";
import { calculateCost, getModel, getModels, getProviders } from "./models.js";
import { MODELS } from "./models.generated.js";
import type { Provider } from "./model-types.js";

const isGeneratedProvider = (provider: Provider): provider is keyof typeof MODELS =>
  Object.prototype.hasOwnProperty.call(MODELS, provider);

describe("models registry", () => {
  it("exposes providers and models", () => {
    const providers = getProviders().filter(isGeneratedProvider);
    expect(providers.length).toBeGreaterThan(0);

    const firstProvider = providers[0];
    const models = getModels(firstProvider);
    expect(models.length).toBeGreaterThan(0);
  });

  it("returns typed model by provider/id", () => {
    const provider = getProviders()
      .filter(isGeneratedProvider)
      .find((p) => getModels(p).length > 0);
    expect(provider).toBeTruthy();
    if (!provider) {
      return;
    }

    const [firstModel] = getModels(provider);
    const resolved = getModel(provider, firstModel.id as keyof (typeof MODELS)[typeof provider]);
    expect(resolved).toBeDefined();
    expect(resolved.id).toBe(firstModel.id);
    expect(resolved.provider).toBe(provider);
  });

  it("calculates usage cost using million-token pricing", () => {
    const provider = getProviders()
      .filter(isGeneratedProvider)
      .find((p) => getModels(p).length > 0)!;
    const model = getModels(provider)[0]!;

    const usage = calculateCost(model, {
      input: 1_000_000,
      output: 2_000_000,
      cacheRead: 0,
      cacheWrite: 0,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    });

    expect(usage.input).toBeCloseTo(model.cost.input);
    expect(usage.output).toBeCloseTo(model.cost.output * 2);
    expect(usage.total).toBeCloseTo(usage.input + usage.output);
  });
});
