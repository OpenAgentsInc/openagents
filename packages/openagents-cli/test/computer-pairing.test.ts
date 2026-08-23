import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer, Option, Redacted } from "effect";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import { ComputerClient, type ComputerClientInterface } from "../src/computer-client.js";
import { ComputerConfiguration } from "../src/computer-config.js";
import { computerPaths } from "../src/computer-config.js";
import { credentialStoreTestFileLayer, CredentialStore } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { outputTestLayer, type OutputDocument } from "../src/output.js";
import { pendingDeviceAuthorizationStoreLayer } from "../src/device-authorization-store.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

const computerClientTestLayer = (client: ComputerClientInterface): Layer.Layer<ComputerClient> =>
  Layer.succeed(ComputerClient, ComputerClient.of(client));

describe("Computer pairing commands", () => {
  it("stores only the machine credential and removes it on logout", async () => {
    const directory = await mkdtemp(join(tmpdir(), "openagents-cli-pairing-"));
    try {
      const credentialPath = join(directory, "credentials.json");
      const configPath = join(directory, "config.json");
      const outputs: Array<OutputDocument> = [];
      const client = computerClientTestLayer({
        start: () =>
          Effect.succeed({
            pairing_id: "pairing-id",
            code: "ABCD-EFGH",
            poll_secret: "poll-secret",
            verify_url: "https://openagents.com/computers",
            expires_at: "2099-01-01T00:00:00.000Z",
            interval_seconds: 3,
          }),
        wait: () =>
          Effect.succeed({
            status: "approved" as const,
            machine_id: "machine-id",
            name: "devin-box",
            token: "smct_machine-secret",
          }),
        status: () => Effect.succeed(Option.none()),
      });
      const layer = Layer.mergeAll(
        NodeServices.layer,
        environmentLayerFromValues({ configPath }),
        persistedConfigurationTestLayer({}),
        terminalSessionTestLayer(false),
        credentialStoreTestFileLayer(credentialPath),
        pendingDeviceAuthorizationStoreLayer.pipe(
          Layer.provide(environmentLayerFromValues({ configPath })),
        ),
        client,
        outputTestLayer((document) =>
          Effect.sync(() => {
            outputs.push(document);
          }),
        ),
        Layer.succeed(
          ComputerConfiguration,
          ComputerConfiguration.of({
            tier: "probe",
            roots: ["/workspace/project"],
            preApproved: [],
            paths: computerPaths(configPath),
          }),
        ),
      );

      await Effect.runPromise(
        runCliWith(["--profile", "local", "computer", "pair"]).pipe(Effect.provide(layer)),
      );
      const config = await readFile(join(directory, "computer.json"), "utf8");
      const pending = await readFile(join(directory, "device-authorizations.json"), "utf8");
      expect(config).not.toContain("smct_machine-secret");
      expect(config).not.toContain("poll-secret");
      expect(pending).not.toContain("smct_machine-secret");
      expect(pending).not.toContain("poll-secret");
      expect(outputs.flatMap((output) => JSON.stringify(output.value))).not.toContain(
        "smct_machine-secret",
      );
      expect(outputs.flatMap((output) => JSON.stringify(output.value))).not.toContain(
        "poll-secret",
      );

      const stored = await Effect.runPromise(
        Effect.gen(function* () {
          const credentials = yield* CredentialStore;
          return yield* credentials.get("http://localhost:4000", "computer");
        }).pipe(Effect.provide(layer)),
      );
      expect(Option.map(stored, Redacted.value)).toEqual(Option.some("smct_machine-secret"));

      await Effect.runPromise(
        runCliWith(["--profile", "local", "computer", "logout"]).pipe(Effect.provide(layer)),
      );
      const removed = await Effect.runPromise(
        Effect.gen(function* () {
          const credentials = yield* CredentialStore;
          return yield* credentials.get("http://localhost:4000", "computer");
        }).pipe(Effect.provide(layer)),
      );
      expect(Option.isNone(removed)).toBe(true);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("reports remote revocation without deleting the stored credential", async () => {
    const directory = await mkdtemp(join(tmpdir(), "openagents-cli-status-"));
    try {
      const credentialPath = join(directory, "credentials.json");
      const configPath = join(directory, "config.json");
      const outputs: Array<OutputDocument> = [];
      const layer = Layer.mergeAll(
        NodeServices.layer,
        environmentLayerFromValues({ configPath }),
        persistedConfigurationTestLayer({}),
        credentialStoreTestFileLayer(credentialPath),
        pendingDeviceAuthorizationStoreLayer.pipe(
          Layer.provide(environmentLayerFromValues({ configPath })),
        ),
        computerClientTestLayer({
          start: () => Effect.die("unused"),
          wait: () => Effect.die("unused"),
          status: () => Effect.succeed(Option.none()),
        }),
        outputTestLayer((document) =>
          Effect.sync(() => {
            outputs.push(document);
          }),
        ),
        Layer.succeed(
          ComputerConfiguration,
          ComputerConfiguration.of({
            tier: "probe",
            roots: [],
            preApproved: [],
            paths: computerPaths(configPath),
          }),
        ),
      );

      await Effect.runPromise(
        Effect.gen(function* () {
          const credentials = yield* CredentialStore;
          yield* credentials.set(
            "http://localhost:4000",
            Redacted.make("smct_revoked"),
            "computer",
          );
          yield* runCliWith(["--profile", "local", "--json", "computer", "status"]);
        }).pipe(Effect.provide(layer)),
      );

      expect(outputs.at(-1)?.value).toMatchObject({
        state: "unpaired",
        paired: false,
        remote_state: "unpaired",
      });
      expect(outputs.at(-1)?.human.join("\n")).toContain("run computer logout");

      const stored = await Effect.runPromise(
        Effect.gen(function* () {
          const credentials = yield* CredentialStore;
          return yield* credentials.get("http://localhost:4000", "computer");
        }).pipe(Effect.provide(layer)),
      );
      expect(Option.map(stored, Redacted.value)).toEqual(Option.some("smct_revoked"));
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});
