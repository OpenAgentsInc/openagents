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

  it("puts an existing issue on a milestone from the command line", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 200,
          body: { ...issueBody(129), milestone: { number: 7, title: "Ship it" } },
        };
      }),
    );

    await run(["issue", "milestone", "129", "--set", "7", "-R", "octavia/project"]);

    expect(requests[0]?.method).toBe("PATCH");
    expect(requests[0]?.path).toBe("/api/v1/repos/octavia/project/issues/129");
    expect(requests[0]?.body).toEqual({ milestone: 7 });
    // The line names the milestone the server RETURNED, so a server that stored
    // something other than what was asked for is visible instead of echoed over.
    expect(written[0]?.document.human).toEqual(["Issue #129 is on milestone #7 Ship it"]);
  });

  it("says the issue is on no milestone when the server reports none", async () => {
    const { run, written } = harness(() =>
      Effect.succeed({ status: 200, body: { ...issueBody(129), milestone: null } }),
    );

    await run(["issue", "milestone", "129", "--clear", "-R", "octavia/project"]);

    expect(written[0]?.document.human).toEqual(["Issue #129 is on no milestone."]);
  });

  it("refuses --set and --clear together instead of guessing which was meant", async () => {
    const { run } = harness(() => Effect.succeed({ status: 200, body: {} }));

    await expect(
      run(["issue", "milestone", "129", "--set", "7", "--clear", "-R", "octavia/project"]),
    ).rejects.toThrow(/either --set or --clear, not both/u);
  });

  it("refuses a milestone change that says nothing about what to change it to", async () => {
    const { run } = harness(() => Effect.succeed({ status: 200, body: {} }));

    await expect(run(["issue", "milestone", "129", "-R", "octavia/project"])).rejects.toThrow(
      /--set <number> to put the issue on one, or --clear/u,
    );
  });

  it("opens a milestone and reports the number the server assigned", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 201, body: { number: 7, title: "Ship it", state: "open" } };
      }),
    );

    await run(["milestone", "create", "Ship it", "-R", "octavia/project"]);

    expect(requests[0]?.method).toBe("POST");
    expect(requests[0]?.path).toBe("/api/v1/repos/octavia/project/milestones");
    expect(requests[0]?.body).toEqual({ title: "Ship it" });
    expect(written[0]?.document.human).toEqual(["Opened milestone #7 Ship it"]);
  });

  it("deletes a milestone through the numbered route", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 204, body: undefined };
      }),
    );

    await run(["milestone", "delete", "7", "-R", "octavia/project"]);

    expect(requests[0]?.method).toBe("DELETE");
    expect(requests[0]?.path).toBe("/api/v1/repos/octavia/project/milestones/7");
    expect(written[0]?.document.human).toEqual(["Deleted milestone #7."]);
  });

  it("lists milestones under both names, with one renderer behind them", async () => {
    const body = { milestones: [{ number: 3, title: "Ship it", state: "open" }] };
    const first = harness(() => Effect.succeed({ status: 200, body }));
    await first.run(["milestone", "list", "-R", "octavia/project"]);

    const second = harness(() => Effect.succeed({ status: 200, body }));
    await second.run(["issue", "milestones", "-R", "octavia/project"]);

    expect(first.written[0]?.document.human).toEqual(second.written[0]?.document.human);
    expect(first.written[0]?.document.human).toEqual(["#3     open    Ship it"]);
  });

  it("says a repository has no milestones rather than printing an empty table", async () => {
    const { run, written } = harness(() =>
      Effect.succeed({ status: 200, body: { milestones: [] } }),
    );

    await run(["milestone", "list", "-R", "octavia/project"]);

    expect(written[0]?.document.human).toEqual(["No milestones found."]);
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

  it("archives and then deletes a board, and the list stops carrying it", async () => {
    // Listing on either side of the delete is what makes this a proof: the
    // board is present before and absent after, so a delete that answered 204
    // without removing anything would still fail here.
    const requests: Array<ApiRequest> = [];
    const board = { number: 5, title: "Scratch", state: "open", archived: false };
    let deleted = false;
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        if (input.method === "DELETE") {
          deleted = true;
          return { status: 204, body: {} };
        }
        if (input.method === "PATCH") return { status: 200, body: { ...board, archived: true } };
        return { status: 200, body: { projects: deleted ? [] : [board] } };
      }),
    );

    await run(["--json", "project", "list", "--archived"]);
    await run(["project", "edit", "5", "--archive"]);
    await run(["project", "delete", "5", "--yes"]);
    await run(["--json", "project", "list", "--archived"]);

    expect(requests.map((entry) => `${entry.method} ${entry.path}`)).toEqual([
      "GET /api/v1/repos/octavia/project/projectsV2?archived=true",
      "PATCH /api/v1/repos/octavia/project/projectsV2/5",
      "DELETE /api/v1/repos/octavia/project/projectsV2/5",
      "GET /api/v1/repos/octavia/project/projectsV2?archived=true",
    ]);
    expect(requests[1]?.body).toEqual({ archived: true });
    expect(written[0]?.document.value).toEqual({ projects: [board] });
    expect(written[3]?.document.value).toEqual({ projects: [] });
  });

  it("refuses to delete a board without --yes and sends no request", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 204, body: {} };
      }),
    );

    await expect(run(["project", "delete", "5"])).rejects.toThrow(/--yes/u);
    expect(requests).toEqual([]);
  });

  it("refuses an edit that names no field and sends no request", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 200, body: {} };
      }),
    );

    await expect(run(["project", "edit", "5"])).rejects.toThrow(/--title/u);
    expect(requests).toEqual([]);
  });

  it("sends a title, description, and state together in one edit", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 200, body: { number: 5, title: "Renamed" } };
      }),
    );

    await run([
      "project",
      "edit",
      "5",
      "--title",
      "Renamed",
      "--description",
      "Why it exists",
      "--state",
      "closed",
    ]);

    expect(requests[0]?.method).toBe("PATCH");
    expect(requests[0]?.body).toEqual({
      title: "Renamed",
      description: "Why it exists",
      state: "closed",
    });
  });
});
