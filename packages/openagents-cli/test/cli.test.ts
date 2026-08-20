import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { RepositoryClient } from "../src/repository-client.js";
import { secretInputTestLayer } from "../src/secret-input.js";

const repository = {
  id: 1,
  name: "project",
  full_name: "octavia/project",
  owner: { id: 10, login: "octavia" },
  private: true,
  default_branch: "main",
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
              repositoryImport: { id: "import-1", state: "completed" as const },
            };
          }),
        list: (input) => Effect.sync(() => (seenOrigins.push(input.origin), [repository])),
        view: (input) => Effect.sync(() => (seenOrigins.push(input.origin), repository)),
        cloneInfo: (input) =>
          Effect.sync(() => {
            seenOrigins.push(input.origin);
            return { repository, cloneUrl: "http://localhost:4000/git/octavia/project.git" };
          }),
        getImport: () => Effect.succeed({ id: "import-1", state: "completed" as const }),
      }),
    );
    const layer = Layer.mergeAll(
      NodeServices.layer,
      environmentLayerFromValues({ token: "test-token" }),
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
    expect(output[0]?.document.value).toEqual({ repositories: [repository] });
  });
});
