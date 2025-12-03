import { describe, expect, it } from "bun:test";
import { calculateCost, getModel, getModels, getProviders } from "./models.js";

describe("models registry", () => {
  it("exposes providers and models", () => {
    const providers = getProviders();
    expect(providers.length).toBeGreaterThan(0);

    const firstProvider = providers[0];
    const models = getModels(firstProvider);
    expect(models.length).toBeGreaterThan(0);
  });

  it("returns typed model by provider/id", () => {
    const providers = getProviders();
    const provider = providers.find((p) => getModels(p).length > 0);
    expect(provider).toBeTruthy();
    if (!provider) return;

    const [firstModel] = getModels(provider);
    const resolved = getModel(provider as any, firstModel.id as any);
    expect(resolved).toBeDefined();
    expect(resolved.id).toBe(firstModel.id);
    expect(resolved.provider).toBe(provider);
  });

  it("calculates usage cost using million-token pricing", () => {
    const providers = getProviders();
    const provider = providers.find((p) => getModels(p).length > 0)!;
    const model = getModels(provider)[0]!;

    const usage = calculateCost(model as any, {
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
