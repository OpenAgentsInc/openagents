import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { ApertureConfigCompilerLive } from "../src/compiler/apertureCompiler.js";
import { ApertureConfigCompilerService } from "../src/compiler/service.js";

import { makePaywall } from "./fixtures.js";

const compileEither = (paywalls: ReturnType<typeof makePaywall>[]) =>
  Effect.gen(function* () {
    const compiler = yield* ApertureConfigCompilerService;
    return yield* Effect.either(compiler.compile(paywalls));
  }).pipe(Effect.provide(ApertureConfigCompilerLive));

describe("lightning-ops compiler diagnostics", () => {
  it.effect("fails with duplicate_route diagnostics", () =>
    Effect.gen(function* () {
      const a = makePaywall("a", { pathPattern: "/api/same", priority: 5 });
      const b = makePaywall("b", { pathPattern: "/api/same", priority: 10 });

      const result = yield* compileEither([a, b]);
      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left._tag).toBe("ApertureCompileValidationError");
        expect(result.left.diagnostics.map((diag) => diag.code)).toContain("duplicate_route");
      }
    }),
  );

  it.effect("fails when first-match ordering would shadow a more specific route", () =>
    Effect.gen(function* () {
      const general = makePaywall("general", { pathPattern: "/api/*", priority: 1 });
      const specific = makePaywall("specific", { pathPattern: "/api/private", priority: 2 });

      const result = yield* compileEither([general, specific]);
      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left.diagnostics.map((diag) => diag.code)).toContain("first_match_shadowed");
      }
    }),
  );

  it.effect("fails with ambiguous_route diagnostics for overlapping same-priority prefix rules", () =>
    Effect.gen(function* () {
      const one = makePaywall("one", { pathPattern: "/api/*", priority: 5 });
      const two = makePaywall("two", { pathPattern: "/api/private/*", priority: 5 });

      const result = yield* compileEither([one, two]);
      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left.diagnostics.map((diag) => diag.code)).toContain("ambiguous_route");
      }
    }),
  );

  it.effect("fails fast with typed diagnostics for missing pricing and invalid upstream/auth fields", () =>
    Effect.gen(function* () {
      const zeroPrice = makePaywall("zero", { fixedAmountMsats: 0 });
      const invalidUpstream = makePaywall("bad-upstream", {
        upstreamUrl: "not-a-url",
      });
      const protocolMismatch = makePaywall("protocol-mismatch", {
        protocol: "http",
        upstreamUrl: "https://upstream.example.com/protocol-mismatch",
      });
      const protocolBase = makePaywall("missing-protocol");
      const missingProtocol = {
        ...protocolBase,
        routes: [
          {
            ...protocolBase.routes[0]!,
            protocol: undefined as unknown as "http",
          },
        ],
      } as ReturnType<typeof makePaywall>;

      const result = yield* compileEither([zeroPrice, invalidUpstream, protocolMismatch, missingProtocol]);
      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        const codes = result.left.diagnostics.map((diag) => diag.code);
        expect(codes).toContain("missing_pricing");
        expect(codes).toContain("invalid_upstream_url");
        expect(codes).toContain("missing_route_protocol");
      }
    }),
  );
});
