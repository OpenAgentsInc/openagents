import { describe, expect, test } from "vite-plus/test";

import {
  CODER_RATE_CATALOG,
  DELIBERATELY_UNPRICED_MODELS,
  priceUsage,
  pricingFromModelsPayload,
} from "./pricing.ts";

const complete = (promptTokens: number, completionTokens: number, cachedInputTokens = 0) => ({
  promptTokens,
  completionTokens,
  cachedInputTokens,
});

describe("priceUsage", () => {
  test("prices a catalogued model from its input, cached, and output rates", () => {
    // gemini-3.7-flash: $1.25 in / $0.10 cached / $10.00 out per Mtok.
    // 1M uncached input + 1M cached input + 1M output.
    const result = priceUsage("gemini-3.7-flash", complete(2_000_000, 1_000_000, 1_000_000));

    expect(result.disposition).toBe("known");
    expect(result.usd).toBeCloseTo(1.25 + 0.1 + 10.0, 10);
  });

  test("bills cached reads at the cached rate, not the input rate", () => {
    const allUncached = priceUsage("gemini-3.7-flash", complete(1_000_000, 0, 0));
    const allCached = priceUsage("gemini-3.7-flash", complete(1_000_000, 0, 1_000_000));

    expect(allUncached.usd).toBeCloseTo(1.25, 10);
    expect(allCached.usd).toBeCloseTo(0.1, 10);
  });

  test("falls back to the input rate where the catalog declares no cached rate", () => {
    // ox-alpha declares no cached rate, so a cached read costs full input.
    const row = CODER_RATE_CATALOG["ox-alpha"]!;
    expect(row.cachedInputUsdPerMtok).toBe(row.inputUsdPerMtok);

    const cached = priceUsage("ox-alpha", complete(1_000_000, 0, 1_000_000));
    expect(cached.usd).toBeCloseTo(0.5, 10);
  });

  test("marks every catalogued rate as an operator placeholder", () => {
    for (const row of Object.values(CODER_RATE_CATALOG)) {
      expect(row.rateBasis).toBe("operator_placeholder");
    }
  });

  // The constraint this whole package exists for.
  test("refuses to price gpt-5.6-luna, and never prices it at zero", () => {
    const result = priceUsage("gpt-5.6-luna", complete(500_000, 50_000));

    expect(result.usd).toBeNull();
    expect(result.usd).not.toBe(0);
    expect(result.disposition).toBe("unpriced_model");
    expect(result.reason).toContain("gpt-5.6-luna");
  });

  test("names gpt-5.6-luna as deliberately unpriced, not merely unrecognised", () => {
    expect(Object.keys(DELIBERATELY_UNPRICED_MODELS)).toContain("gpt-5.6-luna");
    expect(priceUsage("gpt-5.6-luna", complete(1, 1)).disposition).toBe("unpriced_model");
    expect(priceUsage("some-model-nobody-shipped", complete(1, 1)).disposition).toBe(
      "unknown_model",
    );
  });

  test("reports the local lane as unmetered rather than free", () => {
    for (const id of ["ollama:qwen3.8:27b-mtp-q8_0", "ollama/qwen3.8"]) {
      const result = priceUsage(id, complete(100_000, 10_000));
      expect(result.usd).toBeNull();
      expect(result.disposition).toBe("unmetered_local_lane");
    }
  });

  test("reports unknown usage rather than pricing a missing count as zero", () => {
    const noPrompt = priceUsage("gemini-3.7-flash", {
      promptTokens: null,
      completionTokens: 100,
      cachedInputTokens: 0,
    });
    const noCompletion = priceUsage("gemini-3.7-flash", {
      promptTokens: 100,
      completionTokens: null,
      cachedInputTokens: 0,
    });

    expect(noPrompt.usd).toBeNull();
    expect(noPrompt.disposition).toBe("unknown_usage");
    expect(noCompletion.usd).toBeNull();
    expect(noCompletion.disposition).toBe("unknown_usage");
  });

  test("refuses usage claiming more cached reads than prompt tokens", () => {
    const result = priceUsage("gemini-3.7-flash", complete(1_000, 100, 5_000));

    expect(result.usd).toBeNull();
    expect(result.disposition).toBe("unknown_usage");
  });

  test("reports a trial with no model id as unknown", () => {
    expect(priceUsage(null, complete(1_000, 100)).disposition).toBe("unknown_model");
    expect(priceUsage("", complete(1_000, 100)).disposition).toBe("unknown_model");
  });
});

describe("pricingFromModelsPayload", () => {
  // The forge omits `pricing` entirely for an unpriced model, and that absence
  // is the whole signal.
  const payload = {
    models: [
      {
        id: "gemini-3.7-flash",
        pricing: {
          input_per_million_tokens: 1_250_000,
          output_per_million_tokens: 10_000_000,
          cached_input_per_million_tokens: 100_000,
        },
      },
      {
        id: "ox-alpha",
        pricing: {
          input_per_million_tokens: 500_000,
          output_per_million_tokens: 2_000_000,
        },
      },
      { id: "gpt-5.6-luna" },
    ],
  };

  test("converts micro-USD per million tokens into USD per million tokens", () => {
    const catalog = pricingFromModelsPayload(payload);

    expect(catalog["gemini-3.7-flash"]).toMatchObject({
      inputUsdPerMtok: 1.25,
      cachedInputUsdPerMtok: 0.1,
      outputUsdPerMtok: 10,
    });
  });

  test("resolves a missing cached rate to the input rate", () => {
    const catalog = pricingFromModelsPayload(payload);

    expect(catalog["ox-alpha"]?.cachedInputUsdPerMtok).toBe(0.5);
  });

  test("carries the served catalog's omission through as unpriced", () => {
    const catalog = pricingFromModelsPayload(payload);

    expect(catalog["gpt-5.6-luna"]).toBeUndefined();
    expect(priceUsage("gpt-5.6-luna", complete(1_000, 100), catalog).usd).toBeNull();
  });

  test("never claims a served rate was operator-confirmed", () => {
    for (const row of Object.values(pricingFromModelsPayload(payload))) {
      expect(row.rateBasis).toBe("operator_placeholder");
    }
  });

  test("survives a payload that is not a model list", () => {
    expect(pricingFromModelsPayload(null)).toEqual({});
    expect(pricingFromModelsPayload({ models: "nope" })).toEqual({});
    expect(pricingFromModelsPayload({ models: [{ id: 1 }, {}] })).toEqual({});
  });
});
