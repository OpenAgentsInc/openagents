import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { forumClientLayer } from "../src/forum-client.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { requestBodyInputTestLayer } from "../src/request-body-input.js";
import { secretInputTestLayer } from "../src/secret-input.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

const harness = (response: ApiResponse) => {
  const requests: Array<ApiRequest> = [];
  const output: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
  const transport = apiTransportTestLayer((input) =>
    Effect.sync(() => {
      requests.push(input);
      return response;
    }),
  );
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues({ token: "oa_pat_fixture" }),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    gitRunnerTestLayer(() => Effect.void),
    secretInputTestLayer("stdin-token"),
    requestBodyInputTestLayer({}),
    transport,
    forumClientLayer.pipe(Layer.provide(transport)),
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        output.push({ document, mode });
      }),
    ),
  );
  return { layer, requests, output };
};

const topics = {
  query: "fable",
  topics: [
    {
      id: "415e16a7-183c-40d7-90c6-1c0e81a4f873",
      title: "Independent audit",
      author: { display_name: "Fable Coder", is_agent: true },
      board: { slug: "product-promises", title: "Product Promises" },
    },
  ],
};

describe("openagents forum search", () => {
  it("sends q, board, and page to the topics route", async () => {
    const harnessed = harness({ status: 200, body: topics });

    await Effect.runPromise(
      runCliWith([
        "--profile",
        "local",
        "forum",
        "search",
        "fable",
        "--board",
        "general",
        "--page",
        "2",
      ]).pipe(Effect.provide(harnessed.layer)),
    );

    expect(harnessed.requests).toHaveLength(1);
    expect(harnessed.requests[0]?.method).toBe("GET");
    expect(harnessed.requests[0]?.path).toBe("/api/v1/forum/topics?q=fable&forum=general&page=2");
  });

  it("renders each match with its author and board", async () => {
    const harnessed = harness({ status: 200, body: topics });

    await Effect.runPromise(
      runCliWith(["--profile", "local", "forum", "search", "fable"]).pipe(
        Effect.provide(harnessed.layer),
      ),
    );

    expect(harnessed.requests[0]?.path).toBe("/api/v1/forum/topics?q=fable");
    expect(harnessed.output[0]?.document.human).toEqual([
      "415e16a7 — Independent audit — Fable Coder [product-promises]",
    ]);
  });

  it("says so when nothing matches", async () => {
    const harnessed = harness({ status: 200, body: { query: "nothing", topics: [] } });

    await Effect.runPromise(
      runCliWith(["--profile", "local", "forum", "search", "nothing"]).pipe(
        Effect.provide(harnessed.layer),
      ),
    );

    expect(harnessed.output[0]?.document.human).toEqual(["No topics match."]);
  });
});
