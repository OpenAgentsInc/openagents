import { Effect, Layer, Redacted } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { ApiError, InputError, type TransportError } from "../src/errors.js";
import { IssueClient, issueClientLayer } from "../src/issue-client.js";

const token = Redacted.make("test-token");
const origin = "http://localhost:4000";
const target = { origin, token, owner: "octavia", repo: "project" };

const layerFromHandler = (
  handler: (input: ApiRequest) => Effect.Effect<ApiResponse, TransportError>,
): Layer.Layer<IssueClient> => issueClientLayer.pipe(Layer.provide(apiTransportTestLayer(handler)));

const PER_PAGE = 25;

/** A list route that holds 25 to a page and ignores any per_page it is sent. */
const pagedListHandler = (total: number, requests: Array<ApiRequest>) => (input: ApiRequest) =>
  Effect.sync(() => {
    requests.push(input);
    const url = new URL(input.path, `${origin}/`);
    const page = Number(url.searchParams.get("page") ?? "1");
    const start = (page - 1) * PER_PAGE;
    const count = Math.max(0, Math.min(PER_PAGE, total - start));
    return {
      status: 200,
      body: {
        pagination: {
          page,
          per_page: PER_PAGE,
          total,
          total_pages: Math.max(1, Math.ceil(total / PER_PAGE)),
        },
        issues: Array.from({ length: count }, (_, index) => ({
          number: start + index + 1,
          title: `Issue ${start + index + 1}`,
          state: "open",
        })),
      },
    } satisfies ApiResponse;
  });

describe("issue client", () => {
  it("pages past one page of 25 and reports the server's own total", async () => {
    const requests: Array<ApiRequest> = [];
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        return yield* issues.list({ ...target, limit: 100 });
      }).pipe(Effect.provide(layerFromHandler(pagedListHandler(60, requests)))),
    );

    expect(result.issues).toHaveLength(60);
    expect(result.pagination["total"]).toBe(60);
    expect(requests).toHaveLength(3);
    expect(
      requests.map((request) => new URL(request.path, `${origin}/`).searchParams.get("page")),
    ).toEqual(["1", "2", "3"]);
  });

  it("stops at the requested limit rather than reading every page", async () => {
    const requests: Array<ApiRequest> = [];
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        return yield* issues.list({ ...target, limit: 30 });
      }).pipe(Effect.provide(layerFromHandler(pagedListHandler(200, requests)))),
    );

    expect(result.issues).toHaveLength(30);
    expect(result.pagination["total"]).toBe(200);
    expect(requests).toHaveLength(2);
  });

  it("carries every filter the list route names", async () => {
    const requests: Array<ApiRequest> = [];
    await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        return yield* issues.list({
          ...target,
          limit: 5,
          state: "all",
          label: "area:cli",
          assignee: "octavia",
          milestone: "1",
          search: "prerequisite",
          blocked: false,
        });
      }).pipe(Effect.provide(layerFromHandler(pagedListHandler(3, requests)))),
    );

    const parameters = new URL(requests[0]?.path ?? "", `${origin}/`).searchParams;
    expect(parameters.get("state")).toBe("all");
    expect(parameters.get("labels")).toBe("area:cli");
    expect(parameters.get("assignee")).toBe("octavia");
    expect(parameters.get("milestone")).toBe("1");
    expect(parameters.get("q")).toBe("prerequisite");
    expect(parameters.get("blocked")).toBe("false");
  });

  it("refuses a limit that is not a positive integer", async () => {
    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        return yield* issues.list({ ...target, limit: 0 });
      }).pipe(Effect.provide(layerFromHandler(pagedListHandler(1, []))), Effect.flip),
    );

    expect(failure).toBeInstanceOf(InputError);
  });

  it("changes state without sending a body key that would overwrite the issue text", async () => {
    const requests: Array<ApiRequest> = [];
    await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        return yield* issues.setState({ ...target, number: 155, state: "closed" });
      }).pipe(
        Effect.provide(
          layerFromHandler((input) =>
            Effect.sync(() => {
              requests.push(input);
              return { status: 200, body: { number: 155, state: "closed" } };
            }),
          ),
        ),
      ),
    );

    expect(requests[0]?.method).toBe("PATCH");
    expect(requests[0]?.path).toBe("/api/v1/repos/octavia/project/issues/155");
    expect(requests[0]?.body).toEqual({ state: "closed" });
    expect(Object.keys(requests[0]?.body as Record<string, unknown>)).not.toContain("body");
  });

  it("reads, adds, and removes prerequisite edges", async () => {
    const requests: Array<ApiRequest> = [];
    const graph = { blocked: false, blocked_by: [], blocks: [] };
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: input.method === "POST" ? 201 : 200, body: graph };
      }),
    );

    await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        yield* issues.dependencies({ ...target, number: 129 });
        yield* issues.addDependencies({ ...target, number: 129, blockedBy: [80, 81] });
        yield* issues.removeDependency({ ...target, number: 129, blockedBy: 81 });
      }).pipe(Effect.provide(layer)),
    );

    expect(requests.map((request) => `${request.method} ${request.path}`)).toEqual([
      "GET /api/v1/repos/octavia/project/issues/129/dependencies",
      "POST /api/v1/repos/octavia/project/issues/129/dependencies",
      "DELETE /api/v1/repos/octavia/project/issues/129/dependencies/81",
    ]);
    expect(requests[1]?.body).toEqual({ blocked_by: [80, 81] });
    expect(requests[2]?.body).toBeUndefined();
  });

  it("names the field the server rejected rather than reporting a bare status", async () => {
    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const issues = yield* IssueClient;
        return yield* issues.addDependencies({ ...target, number: 129, blockedBy: [99999] });
      }).pipe(
        Effect.provide(
          layerFromHandler(() =>
            Effect.succeed({
              status: 422,
              body: {
                message: "Validation Failed",
                code: "validation_failed",
                status: 422,
                documentation_url: "http://localhost:4000/api/v1",
                request_id: "request-1",
                errors: { blocked_by: ["Issue #99999 does not exist in this repository"] },
              },
            }),
          ),
        ),
        Effect.flip,
      ),
    );

    expect(failure).toBeInstanceOf(ApiError);
    const error = failure as ApiError;
    expect(error.status).toBe(422);
    expect(error.code).toBe("validation_failed");
    expect(error.requestId).toBe("request-1");
    expect(error.message).toBe(
      "Validation Failed (blocked_by: Issue #99999 does not exist in this repository)",
    );
  });
});
