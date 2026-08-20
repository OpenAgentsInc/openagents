import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import { normalizeApiOrigin, Profile, resolveEndpoint } from "../src/endpoint.js";

// Behavior contract: openagents_cli.api_profiles.v1

const noOverrides = {
  profile: Option.none<Profile>(),
  apiUrl: Option.none<string>(),
};
const noEnvironment = {
  profile: Option.none<string>(),
  apiUrl: Option.none<string>(),
};

describe("endpoint configuration", () => {
  it("defaults to production", async () => {
    const endpoint = await Effect.runPromise(resolveEndpoint(noOverrides, noEnvironment));
    expect(endpoint).toEqual({ origin: "https://openagents.com", profile: "production" });
  });

  it("resolves the local profile", async () => {
    const endpoint = await Effect.runPromise(
      resolveEndpoint(
        { ...noOverrides, profile: Option.some(Profile.make("local")) },
        noEnvironment,
      ),
    );
    expect(endpoint).toEqual({ origin: "http://localhost:4000", profile: "local" });
  });

  it("gives command-line profile settings precedence over environment URLs", async () => {
    const endpoint = await Effect.runPromise(
      resolveEndpoint(
        { ...noOverrides, profile: Option.some(Profile.make("staging")) },
        { ...noEnvironment, apiUrl: Option.some("http://localhost:4000") },
      ),
    );
    expect(endpoint.origin).toBe("https://staging.openagents.com");
  });

  it("uses environment settings before persisted configuration", async () => {
    const endpoint = await Effect.runPromise(
      resolveEndpoint(
        noOverrides,
        { ...noEnvironment, profile: Option.some("local") },
        { ...noEnvironment, apiUrl: Option.some("https://configured.example") },
      ),
    );
    expect(endpoint).toEqual({ origin: "http://localhost:4000", profile: "local" });
  });

  it("uses persisted configuration before the production default", async () => {
    const endpoint = await Effect.runPromise(
      resolveEndpoint(noOverrides, noEnvironment, {
        ...noEnvironment,
        profile: Option.some("staging"),
      }),
    );
    expect(endpoint).toEqual({
      origin: "https://staging.openagents.com",
      profile: "staging",
    });
  });

  it("normalizes loopback and HTTPS origins", async () => {
    await expect(Effect.runPromise(normalizeApiOrigin("http://127.0.0.1:4000/"))).resolves.toBe(
      "http://127.0.0.1:4000",
    );
    await expect(Effect.runPromise(normalizeApiOrigin("https://openagents.com/"))).resolves.toBe(
      "https://openagents.com",
    );
  });

  it("refuses non-loopback HTTP and URL paths", async () => {
    const insecure = await Effect.runPromiseExit(normalizeApiOrigin("http://openagents.com"));
    const path = await Effect.runPromiseExit(normalizeApiOrigin("https://openagents.com/api"));
    expect(insecure._tag).toBe("Failure");
    expect(path._tag).toBe("Failure");
  });
});
