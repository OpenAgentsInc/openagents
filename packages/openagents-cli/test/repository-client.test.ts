import { Effect, Fiber, Layer, Redacted, Ref } from "effect";
import { TestClock } from "effect/testing";
import { describe, expect, it } from "vitest";

import {
  ApiTransport,
  apiTransportTestLayer,
  type ApiRequest,
  type ApiResponse,
} from "../src/api-transport.js";
import { RepositoryClient, repositoryClientLayer } from "../src/repository-client.js";
import { TransportError } from "../src/errors.js";

const token = Redacted.make("test-token");

const repositoryFixture = (fullName = "octavia/project") => {
  const [owner = "octavia", name = "project"] = fullName.split("/");
  return {
    id: "repository-1",
    name,
    full_name: fullName,
    owner: { id: 10, login: owner, type: "User" },
    private: true,
    visibility: "private",
    description: null,
    default_branch: "main",
    lifecycle_state: "ready",
    provision_error_code: null,
    clone_url: `http://localhost:4000/${fullName}.git`,
    html_url: `http://localhost:4000/${fullName}`,
    permissions: { admin: true, push: true, pull: true },
    created_at: "2026-08-20T00:00:00Z",
    updated_at: "2026-08-20T00:00:00Z",
  };
};

const importFixture = (state: "pending" | "running" | "completed" | "failed") => ({
  id: "import-1",
  provider: "github",
  source_full_name: "octavia/project",
  source_default_branch: "main",
  source_ref_digest: "a".repeat(64),
  source_head_sha: "b".repeat(40),
  state,
  lfs_warning: false,
  attempt_count: 1,
  error_code: null,
  started_at: "2026-08-20T00:00:00Z",
  completed_at: state === "completed" ? "2026-08-20T00:00:01Z" : null,
});

const layerFromHandler = (
  handler: (input: ApiRequest) => Effect.Effect<ApiResponse, TransportError>,
): Layer.Layer<RepositoryClient> =>
  repositoryClientLayer.pipe(Layer.provide(apiTransportTestLayer(handler)));

describe("repository client", () => {
  it("reads the authenticated GitHub identity used for namespace routing", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 200,
          body: {
            id: 10,
            login: "octavia",
            token_expires_at: "2026-09-20T00:00:00Z",
            namespaces: [
              { id: 10, login: "octavia", type: "user" },
              { id: 20, login: "acme", type: "organization" },
            ],
          },
        };
      }),
    );
    const user = await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.authenticatedUser({ origin: "http://localhost:4000", token });
      }).pipe(Effect.provide(layer)),
    );
    expect(user.login).toBe("octavia");
    expect(requests[0]?.path).toBe("/api/v3/user");
  });

  it("creates a personal repository with the Phoenix API contract", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 201, body: repositoryFixture() };
      }),
    );
    const repository = await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.create({
          origin: "http://localhost:4000",
          token,
          name: "project",
          private: true,
        });
      }).pipe(Effect.provide(layer)),
    );

    expect(repository.full_name).toBe("octavia/project");
    expect(requests).toHaveLength(1);
    expect(requests[0]?.path).toBe("/api/v3/user/repos");
    expect(requests[0]?.body).toEqual({ name: "project", private: true });
    expect(requests[0]?.headers?.["idempotency-key"]).toBeTypeOf("string");
  });

  it("creates an organization repository through the organization route", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 201, body: repositoryFixture("acme/project") };
      }),
    );
    await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.create({
          origin: "http://localhost:4000",
          token,
          owner: "acme",
          name: "project",
          private: true,
        });
      }).pipe(Effect.provide(layer)),
    );
    expect(requests[0]?.path).toBe("/api/v3/orgs/acme/repos");
  });

  it("reports repository provisioning progress before completion", async () => {
    const progress: Array<string> = [];
    const program = Effect.gen(function* () {
      const calls = yield* Ref.make(0);
      const transport = ApiTransport.of({
        request: (input) =>
          input.method === "POST"
            ? Effect.succeed({
                status: 202,
                body: { ...repositoryFixture(), lifecycle_state: "provisioning" },
              })
            : Ref.getAndUpdate(calls, (count) => count + 1).pipe(
                Effect.map((count) => ({
                  status: 200,
                  body: {
                    ...repositoryFixture(),
                    lifecycle_state: count === 0 ? "provisioning" : "ready",
                  },
                })),
              ),
      });
      const clientLayer = repositoryClientLayer.pipe(
        Layer.provide(Layer.succeed(ApiTransport, transport)),
      );
      const fiber = yield* Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.create({
          origin: "http://localhost:4000",
          token,
          name: "project",
          private: true,
          waitTimeoutMs: 10_000,
          pollIntervalMs: 1_000,
          onProgress: ({ state, elapsedMs }) =>
            Effect.sync(() => {
              progress.push(`${state}:${elapsedMs}`);
            }),
        });
      }).pipe(Effect.provide(clientLayer), Effect.forkChild);

      yield* TestClock.adjust("2 seconds");
      return yield* Fiber.join(fiber);
    }).pipe(Effect.provide(TestClock.layer()));

    const result = await Effect.runPromise(program);
    expect(result.lifecycle_state).toBe("ready");
    expect(progress).toEqual(["provisioning:0", "ready:1000"]);
  });

  it("retries a disconnected create with the same idempotency key", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.suspend(() => {
        requests.push(input);
        if (requests.length === 1) {
          return Effect.fail(
            new TransportError({
              operation: "sending the request",
              message: "connection closed",
              cause: new Error("closed"),
            }),
          );
        }
        return Effect.succeed({ status: 201, body: repositoryFixture() });
      }),
    );

    const repository = await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.create({
          origin: "http://localhost:4000",
          token,
          name: "project",
          private: true,
        });
      }).pipe(Effect.provide(layer)),
    );

    expect(repository.full_name).toBe("octavia/project");
    expect(requests).toHaveLength(2);
    expect(requests[0]?.headers?.["idempotency-key"]).toBe(
      requests[1]?.headers?.["idempotency-key"],
    );
  });

  it("decodes list and view response envelopes", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return input.path.startsWith("/api/v3/user/repos")
          ? { status: 200, body: { repositories: [repositoryFixture()], next_cursor: "next" } }
          : { status: 200, body: repositoryFixture() };
      }),
    );
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return {
          list: yield* client.list({
            origin: "http://localhost:4000",
            token,
            namespace: "octavia",
            limit: 12,
            after: "cursor value",
          }),
          view: yield* client.view({
            origin: "http://localhost:4000",
            token,
            owner: "octavia",
            repo: "project",
          }),
        };
      }).pipe(Effect.provide(layer)),
    );
    expect(result.list.repositories).toHaveLength(1);
    expect(result.list.nextCursor).toBe("next");
    expect(requests[0]?.path).toBe(
      "/api/v3/user/repos?per_page=12&after=cursor+value&namespace=octavia",
    );
    expect(result.view.full_name).toBe("octavia/project");
  });

  it("routes GitHub imports to the selected personal or organization namespace", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 202,
          body: {
            ...repositoryFixture(
              input.path.includes("/orgs/") ? "acme/project" : "octavia/project",
            ),
            import: importFixture("pending"),
            replayed: false,
          },
        };
      }),
    );
    await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        yield* client.import({
          origin: "http://localhost:4000",
          token,
          source: "octavia/project",
          private: true,
          waitTimeoutMs: 0,
        });
        yield* client.import({
          origin: "http://localhost:4000",
          token,
          owner: "acme",
          source: "acme/project",
          private: true,
          waitTimeoutMs: 0,
        });
      }).pipe(Effect.provide(layer)),
    );
    expect(requests.map((request) => request.path)).toEqual([
      "/api/v3/user/repos/imports",
      "/api/v3/orgs/acme/repos/imports",
    ]);
  });

  it("polls a one-time GitHub import to completion with TestClock", async () => {
    const progress: Array<string> = [];
    const program = Effect.gen(function* () {
      const calls = yield* Ref.make(0);
      const transport = ApiTransport.of({
        request: (input) =>
          input.method === "POST"
            ? Effect.succeed({
                status: 202,
                body: {
                  ...repositoryFixture(),
                  lifecycle_state: "provisioning",
                  import: importFixture("pending"),
                  replayed: false,
                },
              })
            : Ref.getAndUpdate(calls, (count) => count + 1).pipe(
                Effect.map((count) => ({
                  status: 200,
                  body: {
                    repository: repositoryFixture(),
                    import: importFixture(count === 0 ? "running" : "completed"),
                  },
                })),
              ),
      });
      const clientLayer = repositoryClientLayer.pipe(
        Layer.provide(Layer.succeed(ApiTransport, transport)),
      );
      const fiber = yield* Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.import({
          origin: "http://localhost:4000",
          token,
          source: "octavia/project",
          private: true,
          waitTimeoutMs: 10_000,
          pollIntervalMs: 1_000,
          onProgress: ({ state, attemptCount }) =>
            Effect.sync(() => {
              progress.push(`${state}:${attemptCount}`);
            }),
        });
      }).pipe(Effect.provide(clientLayer), Effect.forkChild);

      yield* TestClock.adjust("2 seconds");
      return yield* Fiber.join(fiber);
    }).pipe(Effect.provide(TestClock.layer()));

    const result = await Effect.runPromise(program);
    expect(result.repository.lifecycle_state).toBe("ready");
    expect(result.repositoryImport.state).toBe("completed");
    expect(result.repositoryImport.source_head_sha).toBe("b".repeat(40));
    expect(progress).toEqual(["running:1", "completed:1"]);
  });

  it("returns the bounded server error when an import fails", async () => {
    const layer = layerFromHandler((input) =>
      Effect.succeed(
        input.method === "POST"
          ? {
              status: 202,
              body: {
                ...repositoryFixture(),
                import: importFixture("pending"),
                replayed: false,
              },
            }
          : {
              status: 200,
              body: {
                repository: repositoryFixture(),
                import: { ...importFixture("failed"), error_code: "source_fetch_failed" },
              },
            },
      ),
    );

    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.import({
          origin: "http://localhost:4000",
          token,
          source: "octavia/project",
          private: true,
          waitTimeoutMs: 1_000,
          pollIntervalMs: 10,
        });
      }).pipe(Effect.provide(layer)),
    );

    expect(exit._tag).toBe("Failure");
    if (exit._tag === "Failure") expect(String(exit.cause)).toContain("source_fetch_failed");
  });

  it("reports a timeout without cancelling the server-side import", async () => {
    const program = Effect.gen(function* () {
      const layer = layerFromHandler((input) =>
        Effect.succeed(
          input.method === "POST"
            ? {
                status: 202,
                body: {
                  ...repositoryFixture(),
                  import: importFixture("pending"),
                  replayed: false,
                },
              }
            : {
                status: 200,
                body: {
                  repository: repositoryFixture(),
                  import: importFixture("running"),
                },
              },
        ),
      );
      const fiber = yield* Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.import({
          origin: "http://localhost:4000",
          token,
          source: "octavia/project",
          private: true,
          waitTimeoutMs: 1_000,
          pollIntervalMs: 100,
        });
      }).pipe(Effect.provide(layer), Effect.forkChild);

      yield* TestClock.adjust("1 second");
      return yield* Fiber.await(fiber);
    }).pipe(Effect.provide(TestClock.layer()));

    const exit = await Effect.runPromise(program);
    expect(exit._tag).toBe("Failure");
    if (exit._tag === "Failure") expect(String(exit.cause)).toContain("continues on the server");
  });

  it("maps non-success statuses to typed API errors", async () => {
    const layer = layerFromHandler(() =>
      Effect.succeed({ status: 404, body: { message: "Repository not found" } }),
    );
    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const client = yield* RepositoryClient;
        return yield* client.view({
          origin: "http://localhost:4000",
          token,
          owner: "octavia",
          repo: "missing",
        });
      }).pipe(Effect.provide(layer)),
    );
    expect(exit._tag).toBe("Failure");
    if (exit._tag === "Failure") expect(String(exit.cause)).toContain("Repository not found");
  });
});
