import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
import { issueClientLayer } from "../src/issue-client.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { projectClientLayer } from "../src/project-client.js";
import { requestBodyInputTestLayer } from "../src/request-body-input.js";
import { secretInputTestLayer } from "../src/secret-input.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

interface Written {
  readonly document: OutputDocument;
  readonly mode: OutputMode;
}

const harness = (
  handler: (input: ApiRequest) => Effect.Effect<ApiResponse, never>,
  standardInput: Readonly<Record<string, string>> = {},
) => {
  const written: Array<Written> = [];
  const transport = apiTransportTestLayer(handler);
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues({ token: "test-token" }),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    gitRunnerTestLayer(() => Effect.void),
    secretInputTestLayer("stdin-token"),
    requestBodyInputTestLayer(standardInput),
    issueClientLayer.pipe(Layer.provide(transport)),
    projectClientLayer.pipe(Layer.provide(transport)),
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        written.push({ document, mode });
      }),
    ),
  );
  const run = (argv: ReadonlyArray<string>) =>
    Effect.runPromise(
      runCliWith(["--profile", "local", ...argv]).pipe(Effect.provide(layer)) as Effect.Effect<
        void,
        unknown
      >,
    );
  return { run, written };
};

const issueBody = (number: number) => ({
  number,
  title: `Issue ${number}`,
  state: "open",
  body: "The body the tracker holds.",
  labels: [],
  assignees: [],
  milestone: null,
  user: { login: "octavia" },
  openagents: { blocked: false, progress: "to_do", blocked_by: [], blocks: [], work: [] },
});

describe("issue and project commands", () => {
  it("infers the repository from the origin remote and pages to the requested limit", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        const page = Number(new URL(input.path, "http://localhost:4000/").searchParams.get("page"));
        const start = (page - 1) * 25;
        const count = Math.max(0, Math.min(25, 40 - start));
        return {
          status: 200,
          body: {
            pagination: { page, per_page: 25, total: 40, total_pages: 2 },
            issues: Array.from({ length: count }, (_, index) => issueBody(start + index + 1)),
          },
        };
      }),
    );

    await run(["--json", "issue", "list", "--limit", "40"]);

    expect(requests[0]?.path.startsWith("/api/v1/repos/octavia/project/issues?")).toBe(true);
    expect(requests).toHaveLength(2);
    const value = written[0]?.document.value as {
      readonly pagination: Record<string, unknown>;
      readonly issues: ReadonlyArray<unknown>;
    };
    expect(written[0]?.mode).toBe("json");
    expect(value.issues).toHaveLength(40);
    expect(value.pagination["total"]).toBe(40);
  });

  it("takes -R over the inferred remote", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 200,
          body: { pagination: { page: 1, per_page: 25, total: 0, total_pages: 1 }, issues: [] },
        };
      }),
    );

    await run(["issue", "list", "-R", "OpenAgentsInc/openagents.com"]);

    expect(requests[0]?.path.startsWith("/api/v1/repos/OpenAgentsInc/openagents.com/issues?")).toBe(
      true,
    );
  });

  it("posts the comment before closing and never sends the issue body", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return input.method === "POST"
          ? { status: 201, body: { id: 1, body: "why" } }
          : { status: 200, body: { ...issueBody(155), state: "closed" } };
      }),
    );

    await run(["issue", "close", "#155", "--comment", "why"]);

    expect(requests.map((request) => `${request.method} ${request.path}`)).toEqual([
      "POST /api/v1/repos/octavia/project/issues/155/comments",
      "PATCH /api/v1/repos/octavia/project/issues/155",
    ]);
    expect(requests[0]?.body).toEqual({ body: "why" });
    expect(requests[1]?.body).toEqual({ state: "closed" });
  });

  it("reads a new issue body from standard input", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness(
      (input) =>
        Effect.sync(() => {
          requests.push(input);
          return { status: 201, body: issueBody(200) };
        }),
      { "-": "Body from standard input.\n" },
    );

    await run([
      "issue",
      "create",
      "--title",
      "From the terminal",
      "--body-file",
      "-",
      "--label",
      "area:cli",
    ]);

    expect(requests[0]?.body).toEqual({
      title: "From the terminal",
      body: "Body from standard input.\n",
      labels: ["area:cli"],
    });
    expect(written[0]?.document.human[0]).toBe("Created #200 Issue 200");
  });

  it("emits the dependency envelope unchanged under --json", async () => {
    const graph = {
      blocked: true,
      blocked_by: [{ number: 80, title: "Prerequisite", state: "open" }],
      blocks: [],
    };
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: input.method === "POST" ? 201 : 200, body: graph };
      }),
    );

    await run(["--json", "issue", "deps", "129", "--add", "#80", "--remove", "81"]);

    expect(requests.map((request) => `${request.method} ${request.path}`)).toEqual([
      "POST /api/v1/repos/octavia/project/issues/129/dependencies",
      "DELETE /api/v1/repos/octavia/project/issues/129/dependencies/81",
    ]);
    expect(written[0]?.mode).toBe("json");
    expect(written[0]?.document.value).toEqual(graph);
  });

  it("refuses an issue reference that is not a number", async () => {
    const { run } = harness(() => Effect.succeed({ status: 200, body: {} }));

    await expect(run(["issue", "view", "not-a-number"])).rejects.toThrow(
      /must be a positive number/u,
    );
  });

  it("resolves projects through the repository-scoped route", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 200,
          body: { projects: [{ number: 1, title: "Roadmap", state: "open", archived: false }] },
        };
      }),
    );

    await run(["--json", "project", "list"]);

    expect(requests[0]?.path).toBe("/api/v1/repos/octavia/project/projectsV2");
    expect(written[0]?.document.value).toEqual({
      projects: [{ number: 1, title: "Roadmap", state: "open", archived: false }],
    });
  });

  it("adds an issue to a board through the item route", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 201, body: { items: [{ id: 7, issue: { number: 155 }, values: {} }] } };
      }),
    );

    await run(["project", "item-add", "2", "--issue", "#155"]);

    expect(requests[0]?.path).toBe("/api/v1/repos/octavia/project/projectsV2/2/items");
    expect(requests[0]?.body).toEqual({ issue_number: 155 });
  });
});
