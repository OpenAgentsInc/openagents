import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { ApertureConfigCompilerLive } from "../src/compiler/apertureCompiler.js";
import { ApertureConfigCompilerService } from "../src/compiler/service.js";
import type { ControlPlanePaywall } from "../src/contracts.js";

import { makePaywall } from "./fixtures.js";

const compile = (paywalls: ReadonlyArray<ControlPlanePaywall>) =>
  Effect.gen(function* () {
    const compiler = yield* ApertureConfigCompilerService;
    return yield* compiler.compile(paywalls);
  }).pipe(Effect.provide(ApertureConfigCompilerLive));

describe("lightning-ops deterministic Aperture compiler", () => {
  it.effect("produces stable hash and ordered rules independent of input ordering", () =>
    Effect.gen(function* () {
      const alpha = makePaywall("alpha", { priority: 20, pathPattern: "/api/alpha" });
      const beta = makePaywall("beta", { priority: 5, pathPattern: "/api/beta" });

      const first = yield* compile([alpha, beta]);
      const second = yield* compile([beta, alpha]);

      expect(first.configHash).toBe(second.configHash);
      expect(first.apertureYaml).toBe(second.apertureYaml);
      expect(first.ruleCount).toBe(2);
      expect(first.valid).toBe(true);
      expect(first.rules[0]?.paywallId).toBe("beta");
      expect(first.rules[1]?.paywallId).toBe("alpha");

      expect(first.apertureYaml).toBe(
        [
          "version: 1",
          "routes:",
          "  - id: beta:route_beta",
          "    match:",
          "      host: openagents.com",
          "      path: /api/beta",
          "    upstream:",
          "      url: https://upstream.example.com/beta",
          "      protocol: https",
          "      timeout_ms: 6000",
          "    auth:",
          "      type: l402",
          "      paywall_id: beta",
          "    pricing:",
          "      mode: fixed_msats",
          "      amount_msats: 2000",
          "  - id: alpha:route_alpha",
          "    match:",
          "      host: openagents.com",
          "      path: /api/alpha",
          "    upstream:",
          "      url: https://upstream.example.com/alpha",
          "      protocol: https",
          "      timeout_ms: 6000",
          "    auth:",
          "      type: l402",
          "      paywall_id: alpha",
          "    pricing:",
          "      mode: fixed_msats",
          "      amount_msats: 2000",
          "",
        ].join("\n"),
      );
    }),
  );
});
