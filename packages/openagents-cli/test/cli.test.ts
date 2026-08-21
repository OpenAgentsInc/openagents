import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer, Option, Redacted } from "effect";
import { TestConsole } from "effect/testing";
import { describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import { browserLauncherTestLayer } from "../src/browser-launcher.js";
import { CredentialStore, credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { deviceClientTestLayer } from "../src/device-client.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { RepositoryClient } from "../src/repository-client.js";
import { secretInputTestLayer } from "../src/secret-input.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

const repository = {
  id: "repository-1",
  name: "project",
  full_name: "octavia/project",
  owner: { id: 10, login: "octavia", type: "User" },
  private: true,
  visibility: "private" as const,
  description: null,
  default_branch: "main",
  lifecycle_state: "ready" as const,
  provision_error_code: null,
  clone_url: "http://localhost:4000/octavia/project.git",
  html_url: "http://localhost:4000/octavia/project",
  permissions: { admin: true, push: true, pull: true },
  created_at: "2026-08-20T00:00:00Z",
  updated_at: "2026-08-20T00:00:00Z",
};

const repositoryImport = {
  id: "import-1",
  provider: "github" as const,
  source_full_name: "octavia/project",
  source_default_branch: "main",
  source_ref_digest: "a".repeat(64),
  source_head_sha: "b".repeat(40),
  state: "completed" as const,
  lfs_warning: false,
  attempt_count: 1,
  error_code: null,
  started_at: "2026-08-20T00:00:00Z",
  completed_at: "2026-08-20T00:00:01Z",
};

describe("CLI command graph", () => {
  it("routes the local profile through repository list and writes JSON", async () => {
    const seenOrigins: Array<string> = [];
    const output: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const repositoryLayer = Layer.succeed(
      RepositoryClient,
      RepositoryClient.of({
        create: (input) => Effect.sync(() => (seenOrigins.push(input.origin), repository)),
        import: (input) =>
          Effect.sync(() => {
            seenOrigins.push(input.origin);
            return {
              repository,
              repositoryImport,
            };
          }),
        authenticatedUser: (input) =>
          Effect.sync(() => {
            seenOrigins.push(input.origin);
            return {
              id: 10,
              login: "octavia",
              token_expires_at: "2026-09-20T00:00:00Z",
              namespaces: [{ id: 10, login: "octavia", type: "user" as const }],
            };
          }),
        list: (input) =>
          Effect.sync(() => {
            seenOrigins.push(input.origin);
            return { repositories: [repository], nextCursor: null };
          }),
        view: (input) => Effect.sync(() => (seenOrigins.push(input.origin), repository)),
        cloneInfo: (input) =>
          Effect.sync(() => {
            seenOrigins.push(input.origin);
            return { repository, cloneUrl: "http://localhost:4000/octavia/project.git" };
          }),
        getImport: () => Effect.succeed(repositoryImport),
      }),
    );
    const layer = Layer.mergeAll(
      NodeServices.layer,
      environmentLayerFromValues({ token: "test-token" }),
      persistedConfigurationTestLayer({}),
      terminalSessionTestLayer(false),
      credentialStoreUnavailableLayer,
      repositoryLayer,
      gitRunnerTestLayer(() => Effect.void),
      secretInputTestLayer("stdin-token"),
      outputTestLayer((document, mode) =>
        Effect.sync(() => {
          output.push({ document, mode });
        }),
      ),
    );

    await Effect.runPromise(
      runCliWith(["--profile", "local", "--json", "repo", "list"]).pipe(Effect.provide(layer)),
    );

    expect(seenOrigins).toEqual(["http://localhost:4000"]);
    expect(output).toHaveLength(1);
    expect(output[0]?.mode).toBe("json");
    expect(output[0]?.document.value).toEqual({ repositories: [repository], next_cursor: null });
  });

  it("defaults import to the matching GitHub user or organization namespace", async () => {
    const owners: Array<string | undefined> = [];
    const repositoryLayer = Layer.succeed(
      RepositoryClient,
      RepositoryClient.of({
        create: () => Effect.succeed(repository),
        authenticatedUser: () =>
          Effect.succeed({
            id: 10,
            login: "octavia",
            token_expires_at: "2026-09-20T00:00:00Z",
            namespaces: [
              { id: 10, login: "octavia", type: "user" as const },
              { id: 20, login: "acme", type: "organization" as const },
            ],
          }),
        import: (input) =>
          Effect.sync(() => {
            owners.push(input.owner);
            return { repository, repositoryImport };
          }),
        list: () => Effect.succeed({ repositories: [], nextCursor: null }),
        view: () => Effect.succeed(repository),
        cloneInfo: () =>
          Effect.succeed({
            repository,
            cloneUrl: "http://localhost:4000/octavia/project.git",
          }),
        getImport: () => Effect.succeed(repositoryImport),
      }),
    );
    const layer = Layer.mergeAll(
      NodeServices.layer,
      environmentLayerFromValues({ token: "test-token" }),
      persistedConfigurationTestLayer({}),
      terminalSessionTestLayer(false),
      credentialStoreUnavailableLayer,
      repositoryLayer,
      gitRunnerTestLayer(() => Effect.void),
      secretInputTestLayer("stdin-token"),
      outputTestLayer(() => Effect.void),
    );

    await Effect.runPromise(
      runCliWith([
        "--profile",
        "local",
        "repo",
        "import",
        "octavia/project",
        "--wait-timeout",
        "0",
      ]).pipe(Effect.provide(layer)),
    );
    await Effect.runPromise(
      runCliWith([
        "--profile",
        "local",
        "repo",
        "import",
        "acme/project",
        "--wait-timeout",
        "0",
      ]).pipe(Effect.provide(layer)),
    );

    expect(owners).toEqual([undefined, "acme"]);
  });

  it("supports token stdin without opening a browser", async () => {
    const stored: Array<{ readonly origin: string; readonly token: string }> = [];
    const credentialLayer = Layer.succeed(
      CredentialStore,
      CredentialStore.of({
        get: () => Effect.succeed(Option.none()),
        set: (origin, token) =>
          Effect.sync(() => {
            stored.push({ origin, token: Redacted.value(token) });
          }),
        remove: () => Effect.void,
      }),
    );
    const layer = Layer.mergeAll(
      NodeServices.layer,
      environmentLayerFromValues({}),
      persistedConfigurationTestLayer({}),
      terminalSessionTestLayer(false),
      credentialLayer,
      secretInputTestLayer("oa_pat_stdin-fixture"),
      outputTestLayer(() => Effect.void),
    );

    await Effect.runPromise(
      runCliWith(["--profile", "local", "auth", "login", "--token-stdin"]).pipe(
        Effect.provide(layer),
      ),
    );
    expect(stored).toEqual([{ origin: "http://localhost:4000", token: "oa_pat_stdin-fixture" }]);
  });

  it("prints a device URL and code and waits for approval without a TTY", async () => {
    const stored: Array<{ readonly origin: string; readonly token: string }> = [];
    const opened: Array<string> = [];
    const credentialLayer = Layer.succeed(
      CredentialStore,
      CredentialStore.of({
        get: () => Effect.succeed(Option.none()),
        set: (origin, token) =>
          Effect.sync(() => {
            stored.push({ origin, token: Redacted.value(token) });
          }),
        remove: () => Effect.void,
      }),
    );
    const layer = Layer.mergeAll(
      NodeServices.layer,
      TestConsole.layer,
      environmentLayerFromValues({}),
      persistedConfigurationTestLayer({}),
      terminalSessionTestLayer(false),
      credentialLayer,
      deviceClientTestLayer({
        start: () =>
          Effect.succeed({
            device_code: "secret-device-code",
            user_code: "ABCD-EFGH",
            verification_uri: "http://localhost:4000/device",
            verification_uri_complete: "http://localhost:4000/device?user_code=ABCD-EFGH",
            expires_in: 600,
            interval: 1,
          }),
        wait: () => Effect.succeed(Redacted.make("oa_pat_device-fixture")),
      }),
      browserLauncherTestLayer((url) =>
        Effect.sync(() => {
          opened.push(url);
          return true;
        }),
      ),
      outputTestLayer(() => Effect.void),
    );

    const errors = await Effect.runPromise(
      Effect.gen(function* () {
        yield* runCliWith(["--profile", "local", "auth", "login"]);
        return yield* TestConsole.errorLines;
      }).pipe(Effect.provide(layer)),
    );

    const displayed = errors.map(String).join("\n");
    expect(displayed).toContain("http://localhost:4000/device?user_code=ABCD-EFGH");
    expect(displayed).toContain("ABCD-EFGH");
    expect(displayed).toContain("Waiting for approval");
    expect(opened).toEqual([]);
    expect(stored).toEqual([{ origin: "http://localhost:4000", token: "oa_pat_device-fixture" }]);
  });
});
