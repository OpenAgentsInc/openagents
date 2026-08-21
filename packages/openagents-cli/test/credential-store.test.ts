import { Effect, Option, Redacted } from "effect";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, describe, expect, it } from "vitest";

import {
  CredentialStore,
  credentialCommandFor,
  credentialStoreTestFileLayer,
  credentialStoreUnavailableLayer,
} from "../src/credential-store.js";

const temporaryDirectories: Array<string> = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((path) => rm(path, { recursive: true, force: true })),
  );
});

describe("credential store", () => {
  it("passes a macOS token as the required Keychain password argument", () => {
    // Contract: openagents_cli.agent_device_authorization.v1
    expect(
      credentialCommandFor("darwin", "set", "https://openagents.com", "oa_pat_keychain-fixture"),
    ).toEqual({
      command: "security",
      args: [
        "add-generic-password",
        "-U",
        "-a",
        "https://openagents.com",
        "-s",
        "openagents-cli",
        "-w",
        "oa_pat_keychain-fixture",
      ],
    });
  });

  it("scopes test tokens to normalized API origins", async () => {
    const directory = await mkdtemp(join(tmpdir(), "openagents-cli-credentials-"));
    temporaryDirectories.push(directory);
    const layer = credentialStoreTestFileLayer(join(directory, "tokens.json"));
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* CredentialStore;
        yield* store.set("https://openagents.com", Redacted.make("prod-token"));
        yield* store.set("http://localhost:4000", Redacted.make("local-token"));
        return {
          production: yield* store.get("https://openagents.com"),
          local: yield* store.get("http://localhost:4000"),
          staging: yield* store.get("https://staging.openagents.com"),
        };
      }).pipe(Effect.provide(layer)),
    );

    expect(Option.map(result.production, Redacted.value)).toEqual(Option.some("prod-token"));
    expect(Option.map(result.local, Redacted.value)).toEqual(Option.some("local-token"));
    expect(Option.isNone(result.staging)).toBe(true);
  });

  it("refuses production persistence when no OS adapter is admitted", async () => {
    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const store = yield* CredentialStore;
        yield* store.set("https://openagents.com", Redacted.make("secret"));
      }).pipe(Effect.provide(credentialStoreUnavailableLayer)),
    );
    expect(exit._tag).toBe("Failure");
    if (exit._tag === "Failure") {
      expect(String(exit.cause)).toContain("CredentialPersistenceUnavailable");
      expect(String(exit.cause)).not.toContain("secret");
    }
  });
});
