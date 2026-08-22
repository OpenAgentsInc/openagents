import * as NodeServices from "@effect/platform-node/NodeServices";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer, type IncomingMessage } from "node:http";
import { resolve } from "node:path";

import { Effect, Layer, Option, Redacted } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { runCliWith } from "../src/cli.js";
import { CredentialStore, credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { exitCodeFor, TransportError, type CliError } from "../src/errors.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { requestBodyInputTestLayer } from "../src/request-body-input.js";
import { secretInputTestLayer } from "../src/secret-input.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

const harness = (options?: {
  readonly response?: ApiResponse;
  readonly transport?: (input: ApiRequest) => Effect.Effect<ApiResponse, TransportError>;
  readonly token?: string;
  readonly bodies?: Readonly<Record<string, string>>;
  readonly credentials?: Layer.Layer<CredentialStore>;
}) => {
  const requests: Array<ApiRequest> = [];
  const output: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
  const respond =
    options?.transport ??
    ((_input: ApiRequest) =>
      Effect.succeed(
        options?.response ?? ({ status: 200, body: { ok: true } } satisfies ApiResponse),
      ));
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues(options?.token === undefined ? {} : { token: options.token }),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    options?.credentials ?? credentialStoreUnavailableLayer,
    gitRunnerTestLayer(() => Effect.void),
    secretInputTestLayer("stdin-token"),
    requestBodyInputTestLayer(options?.bodies ?? {}),
    apiTransportTestLayer((input) =>
      Effect.suspend(() => {
        requests.push(input);
        return respond(input);
      }),
    ),
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        output.push({ document, mode });
      }),
    ),
  );
  return { layer, requests, output };
};

type Harness = ReturnType<typeof harness>;

const run = (harnessed: Harness, argv: ReadonlyArray<string>) =>
  Effect.runPromise(
    runCliWith(["--profile", "local", ...argv]).pipe(Effect.provide(harnessed.layer)),
  );

const failureOf = (harnessed: Harness, argv: ReadonlyArray<string>): Promise<CliError> =>
  Effect.runPromise(
    runCliWith(["--profile", "local", ...argv]).pipe(Effect.provide(harnessed.layer), Effect.flip),
  ) as Promise<CliError>;

describe("openagents api", () => {
  it("sends the token, the resolved path, and the response body to stdout", async () => {
    const harnessed = harness({
      token: "oa_pat_fixture",
      response: { status: 200, body: [{ number: 41 }] },
    });
    await run(harnessed, ["--json", "api", "repos/octavia/project/issues"]);

    expect(harnessed.requests).toHaveLength(1);
    expect(harnessed.requests[0]?.origin).toBe("http://localhost:4000");
    expect(harnessed.requests[0]?.method).toBe("GET");
    expect(harnessed.requests[0]?.path).toBe("/api/v3/repos/octavia/project/issues");
    expect(Redacted.value(harnessed.requests[0]?.token ?? Redacted.make(""))).toBe(
      "oa_pat_fixture",
    );
    expect(harnessed.requests[0]?.body).toBeUndefined();
    expect(harnessed.output[0]?.mode).toBe("json");
    expect(harnessed.output[0]?.document.value).toEqual([{ number: 41 }]);
  });

  it("writes the body as JSON in human mode too, so the command composes with jq", async () => {
    const harnessed = harness({
      token: "oa_pat_fixture",
      response: { status: 200, body: { a: 1 } },
    });
    await run(harnessed, ["api", "user"]);

    expect(harnessed.output[0]?.mode).toBe("human");
    expect(harnessed.output[0]?.document.human).toEqual([JSON.stringify({ a: 1 }, null, 2)]);
  });

  it.each(["GET", "POST", "PATCH", "PUT", "DELETE"] as const)(
    "sends %s with -X",
    async (method) => {
      const harnessed = harness({ token: "oa_pat_fixture" });
      await run(harnessed, ["api", "-X", method, "repos/octavia/project/issues/41"]);
      expect(harnessed.requests[0]?.method).toBe(method);
    },
  );

  it("accepts an absolute API path and a relative path as the same route", async () => {
    const harnessed = harness({ token: "oa_pat_fixture" });
    await run(harnessed, ["api", "repos/octavia/project/issues"]);
    await run(harnessed, ["api", "/api/v3/repos/octavia/project/issues"]);
    expect(harnessed.requests.map((request) => request.path)).toEqual([
      "/api/v3/repos/octavia/project/issues",
      "/api/v3/repos/octavia/project/issues",
    ]);
  });

  it("refuses a path that leaves the configured origin", async () => {
    const harnessed = harness({ token: "oa_pat_fixture" });
    const failure = await failureOf(harnessed, ["api", "https://openagents.com/api/v3/user"]);
    expect(failure._tag).toBe("OpenAgentsCli.InputError");
    expect(failure.message).toContain("leaves the configured API origin");
    expect(harnessed.requests).toHaveLength(0);
  });

  it("builds a JSON object from repeated --field flags and defaults to POST", async () => {
    const harnessed = harness({ token: "oa_pat_fixture", response: { status: 201, body: {} } });
    await run(harnessed, [
      "api",
      "-f",
      "title=Fix the bug",
      "-f",
      "body=It fails on Tuesdays",
      "repos/octavia/project/issues",
    ]);

    expect(harnessed.requests[0]?.method).toBe("POST");
    expect(harnessed.requests[0]?.body).toEqual({
      title: "Fix the bug",
      body: "It fails on Tuesdays",
    });
  });

  it("reads a whole JSON body from a file and from standard input", async () => {
    const harnessed = harness({
      token: "oa_pat_fixture",
      bodies: { "body.json": '{"labels":["bug"]}', "-": '{"milestone":3}' },
    });
    await run(harnessed, ["api", "--input", "body.json", "repos/octavia/project/issues"]);
    await run(harnessed, ["api", "-X", "PATCH", "--input", "-", "repos/octavia/project/issues/41"]);

    expect(harnessed.requests[0]?.body).toEqual({ labels: ["bug"] });
    expect(harnessed.requests[1]?.method).toBe("PATCH");
    expect(harnessed.requests[1]?.body).toEqual({ milestone: 3 });
  });

  it("refuses --field together with --input", async () => {
    const harnessed = harness({ token: "oa_pat_fixture", bodies: { "-": "{}" } });
    const failure = await failureOf(harnessed, [
      "api",
      "-f",
      "title=x",
      "--input",
      "-",
      "repos/octavia/project/issues",
    ]);
    expect(failure._tag).toBe("OpenAgentsCli.InputError");
    expect(failure.message).toContain("Use either --field or --input");
    expect(harnessed.requests).toHaveLength(0);
  });

  it("merges repeated --header flags into the request", async () => {
    const harnessed = harness({ token: "oa_pat_fixture" });
    await run(harnessed, [
      "api",
      "-H",
      "X-Trace-Id: abc",
      "-H",
      "Accept: application/vnd.openagents+json",
      "repos/octavia/project/issues",
    ]);
    expect(harnessed.requests[0]?.headers).toEqual({
      "x-trace-id": "abc",
      accept: "application/vnd.openagents+json",
    });
  });

  it("refuses a --header that would replace the session authorization", async () => {
    const harnessed = harness({ token: "oa_pat_fixture" });
    const failure = await failureOf(harnessed, [
      "api",
      "-H",
      "Authorization: Bearer stolen",
      "repos/octavia/project/issues",
    ]);
    expect(failure._tag).toBe("OpenAgentsCli.InputError");
    expect(failure.message).toContain("authorization header from your OpenAgents session");
    expect(harnessed.requests).toHaveLength(0);
  });

  it("asks an unauthenticated caller to sign in, with the guidance every command gives", async () => {
    const harnessed = harness({
      credentials: Layer.succeed(
        CredentialStore,
        CredentialStore.of({
          get: () => Effect.succeed(Option.none()),
          set: () => Effect.void,
          remove: () => Effect.void,
        }),
      ),
    });
    const failure = await failureOf(harnessed, ["api", "repos/octavia/project/issues"]);
    expect(failure._tag).toBe("OpenAgentsCli.AuthenticationRequired");
    expect(failure.message).toContain("OPENAGENTS_TOKEN");
    expect(exitCodeFor(failure)).toBe(3);
    expect(harnessed.requests).toHaveLength(0);
  });

  it("fails a 404 with the status, the server message, and the request id", async () => {
    const harnessed = harness({
      token: "oa_pat_fixture",
      response: {
        status: 404,
        body: { message: "Not Found", code: "not_found" },
        requestId: "request-404",
      },
    });
    const failure = await failureOf(harnessed, ["api", "repos/octavia/project/issues/9999"]);
    expect(failure._tag).toBe("OpenAgentsCli.ApiError");
    expect(failure.message).toContain("HTTP 404 for GET /api/v3/repos/octavia/project/issues/9999");
    expect(failure.message).toContain("Not Found");
    if (failure._tag === "OpenAgentsCli.ApiError") {
      expect(failure.status).toBe(404);
      expect(failure.code).toBe("not_found");
      expect(failure.requestId).toBe("request-404");
    }
    expect(exitCodeFor(failure)).not.toBe(0);
    expect(harnessed.output).toHaveLength(0);
  });

  it("separates a transport failure from an HTTP error status", async () => {
    const harnessed = harness({
      token: "oa_pat_fixture",
      transport: () =>
        Effect.fail(
          new TransportError({
            operation: "sending the request",
            message: "The OpenAgents API request failed during sending the request.",
            cause: new Error("ECONNREFUSED"),
          }),
        ),
    });
    const failure = await failureOf(harnessed, ["api", "repos/octavia/project/issues"]);
    expect(failure._tag).toBe("OpenAgentsCli.TransportError");
    expect(exitCodeFor(failure)).toBe(6);
    expect(harnessed.output).toHaveLength(0);
  });
});

interface RecordedRequest {
  readonly method: string;
  readonly url: string;
  readonly headers: IncomingMessage["headers"];
  readonly body: string;
}

const loopbackServer = async (
  respond: (request: RecordedRequest) => { readonly status: number; readonly body: unknown },
) => {
  const received: Array<RecordedRequest> = [];
  const server = createServer((request, response) => {
    let body = "";
    request.on("data", (chunk) => (body += String(chunk)));
    request.on("end", () => {
      const recorded: RecordedRequest = {
        method: request.method ?? "",
        url: request.url ?? "",
        headers: request.headers,
        body,
      };
      received.push(recorded);
      const answer = respond(recorded);
      response.writeHead(answer.status, {
        "content-type": "application/json",
        "x-request-id": "request-loopback",
      });
      response.end(JSON.stringify(answer.body));
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("missing test server port");
  return {
    received,
    origin: `http://127.0.0.1:${address.port}`,
    close: async () => {
      const closed = once(server, "close");
      server.close();
      await closed;
    },
  };
};

const runCliProcess = async (
  argv: ReadonlyArray<string>,
  standardInput?: string,
): Promise<{ readonly code: number | null; readonly stdout: string; readonly stderr: string }> => {
  const child = spawn(
    process.execPath,
    ["--import", "tsx", resolve(import.meta.dirname, "../src/main.ts"), ...argv],
    {
      cwd: resolve(import.meta.dirname, "../../.."),
      env: {
        ...process.env,
        OPENAGENTS_TOKEN: "oa_pat_loopback-fixture",
        NO_COLOR: "1",
      },
      stdio: [standardInput === undefined ? "ignore" : "pipe", "pipe", "pipe"],
    },
  );
  let stdout = "";
  let stderr = "";
  child.stdout?.on("data", (chunk) => (stdout += String(chunk)));
  child.stderr?.on("data", (chunk) => (stderr += String(chunk)));
  if (standardInput !== undefined) child.stdin?.end(standardInput);
  const [code] = (await once(child, "exit")) as [number | null, string | null];
  return { code, stdout, stderr };
};

describe("openagents api against a loopback server", () => {
  it("reads a JSON body from real standard input and writes only the response to stdout", async () => {
    const server = await loopbackServer(() => ({ status: 201, body: { number: 41 } }));
    try {
      const result = await runCliProcess(
        [
          "--api-url",
          server.origin,
          "--json",
          "api",
          "--input",
          "-",
          "repos/octavia/project/issues",
        ],
        '{"title":"From stdin","labels":["bug"]}',
      );

      expect(result.code).toBe(0);
      expect(JSON.parse(result.stdout)).toEqual({ number: 41 });
      expect(server.received[0]?.method).toBe("POST");
      expect(server.received[0]?.url).toBe("/api/v3/repos/octavia/project/issues");
      expect(server.received[0]?.headers.authorization).toBe("Bearer oa_pat_loopback-fixture");
      expect(JSON.parse(server.received[0]?.body ?? "")).toEqual({
        title: "From stdin",
        labels: ["bug"],
      });
    } finally {
      await server.close();
    }
  }, 30_000);

  it("exits non-zero on a 404 and keeps the error body off stdout", async () => {
    const server = await loopbackServer(() => ({
      status: 404,
      body: { message: "Not Found" },
    }));
    try {
      const result = await runCliProcess([
        "--api-url",
        server.origin,
        "api",
        "repos/octavia/project/issues/9999",
      ]);

      expect(result.code).toBe(4);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("Not Found");
      expect(result.stderr).toContain("Request id: request-loopback");
      expect(result.stderr).toContain("HTTP 404 for GET /api/v3/repos/octavia/project/issues/9999");
    } finally {
      await server.close();
    }
  }, 30_000);
});
