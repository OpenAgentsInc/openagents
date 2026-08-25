import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Fiber, Layer } from "effect";
import { describe, expect, it, vi } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { fleetClientLayer } from "../src/fleet-client.js";
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

const fullSha = "f".repeat(40);
const targetId = "0d4e8a70-0000-4000-8000-000000000001";

const targetBody = (status: string, overrides: Record<string, unknown> = {}) => ({
  id: targetId,
  repo: "openagents.com",
  sha: fullSha,
  status,
  terminal: ["live", "failed", "reverted"].includes(status),
  promoted_by: "operator:1",
  environment: "production",
  source: "api",
  artifact_digest: null,
  deployment_lane: null,
  error_code: null,
  promoted_at: "2026-08-24T00:00:00Z",
  updated_at: "2026-08-24T00:00:00Z",
  status_url: `http://localhost:4000/api/v1/admin/forge/targets/${targetId}`,
  ...overrides,
});

const envelope = (status: number, code: string, message: string) => ({
  status,
  body: { message, code, status, documentation_url: "", request_id: "req-1", errors: {} },
});

const harness = (handler: (input: ApiRequest) => Effect.Effect<ApiResponse, never>) => {
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
    requestBodyInputTestLayer({}),
    fleetClientLayer.pipe(Layer.provide(transport)),
    issueClientLayer.pipe(Layer.provide(transport)),
    projectClientLayer.pipe(Layer.provide(transport)),
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        written.push({ document, mode });
      }),
    ),
  );
  const program = (argv: ReadonlyArray<string>) =>
    runCliWith(["--profile", "local", ...argv]).pipe(Effect.provide(layer)) as Effect.Effect<
      void,
      unknown
    >;
  const run = (argv: ReadonlyArray<string>) => Effect.runPromise(program(argv));
  return { run, program, written };
};

const promoteArgv = (extra: ReadonlyArray<string> = []) => [
  "deploy",
  "promote",
  "--repo",
  "openagents.com",
  "--sha",
  fullSha,
  "--environment",
  "production",
  ...extra,
];

describe("deploy commands", () => {
  it("promotes the exact reviewed inputs and reports accepted, not live", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: targetBody("queued") };
      }),
    );

    await run([
      "--json",
      ...promoteArgv([
        "--idempotency-key",
        "release-key-0001",
        "--expected-current-target",
        "0d4e8a70-0000-4000-8000-000000000000",
      ]),
    ]);

    expect(requests).toHaveLength(1);
    expect(requests[0]?.method).toBe("POST");
    expect(requests[0]?.path).toBe("/api/v1/admin/forge/targets");
    expect(requests[0]?.body).toEqual({
      repo: "openagents.com",
      sha: fullSha,
      environment: "production",
      idempotency_key: "release-key-0001",
      expected_current_target_id: "0d4e8a70-0000-4000-8000-000000000000",
    });
    const value = written[0]?.document.value as Record<string, unknown>;
    expect(written[0]?.mode).toBe("json");
    expect(value["accepted"]).toBe(true);
    expect(value["live"]).toBe(false);
    expect(value["outcome"]).toBe("accepted");
    expect(value["replayed"]).toBe(false);
    expect((value["target"] as Record<string, unknown>)["sha"]).toBe(fullSha);
  });

  it("never prints the idempotency key in output or the human summary", async () => {
    const { run, written } = harness(() =>
      Effect.succeed({ status: 202, body: targetBody("queued") }),
    );

    await run(["--json", ...promoteArgv(["--idempotency-key", "secret-idempotency-key"])]);

    expect(JSON.stringify(written)).not.toContain("secret-idempotency-key");
  });

  it("generates one idempotency key and reuses it when it must not be printed", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: targetBody("queued") };
      }),
    );

    await run(promoteArgv());

    const key = (requests[0]?.body as Record<string, unknown>)["idempotency_key"];
    expect(typeof key).toBe("string");
    expect((key as string).length).toBeGreaterThanOrEqual(8);
    expect(JSON.stringify(written)).not.toContain(key);
  });

  it("refuses an abbreviated SHA before sending any request", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: targetBody("queued") };
      }),
    );

    await expect(
      run([
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        "f".repeat(12),
        "--environment",
        "production",
      ]),
    ).rejects.toThrow(/full 40-character commit SHA/u);
    expect(requests).toHaveLength(0);
  });

  it("refuses a branch name in --sha before sending any request", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: targetBody("queued") };
      }),
    );

    await expect(
      run([
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        "main",
        "--environment",
        "production",
      ]),
    ).rejects.toThrow(/full 40-character commit SHA/u);
    expect(requests).toHaveLength(0);
  });

  it("refuses a promotion with no explicit environment before sending any request", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: targetBody("queued") };
      }),
    );

    await expect(
      run(["deploy", "promote", "--repo", "openagents.com", "--sha", fullSha]),
    ).rejects.toThrow(/--environment production explicitly/u);
    expect(requests).toHaveLength(0);
  });

  it("refuses a promotion with no repository before sending any request", async () => {
    const requests: Array<ApiRequest> = [];
    const { run } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: targetBody("queued") };
      }),
    );

    await expect(
      run(["deploy", "promote", "--sha", fullSha, "--environment", "production"]),
    ).rejects.toThrow(/--repo/u);
    expect(requests).toHaveLength(0);
  });

  it("waits to live and reports the terminal outcome", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return input.method === "POST"
          ? { status: 202, body: targetBody("queued") }
          : { status: 200, body: targetBody("live") };
      }),
    );

    await run(["--json", ...promoteArgv(["--wait", "--idempotency-key", "release-key-0002"])]);

    expect(requests.map((request) => `${request.method} ${request.path}`)).toEqual([
      "POST /api/v1/admin/forge/targets",
      `GET /api/v1/admin/forge/targets/${targetId}`,
    ]);
    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["outcome"]).toBe("live");
    expect(value["live"]).toBe(true);
    expect(value["terminal"]).toBe(true);
  });

  it("reports a failed target as a deployment failure with its failure code", async () => {
    const { run, written } = harness((input) =>
      Effect.succeed(
        input.method === "POST"
          ? { status: 202, body: targetBody("queued") }
          : { status: 200, body: targetBody("failed", { error_code: "artifact_mismatch" }) },
      ),
    );

    await expect(
      run(["--json", ...promoteArgv(["--wait", "--idempotency-key", "release-key-0003"])]),
    ).rejects.toThrow(/reached failed \(artifact_mismatch\)/u);
    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["outcome"]).toBe("failed");
    expect(value["live"]).toBe(false);
    expect(value["failure_code"]).toBe("artifact_mismatch");
  });

  it("reports a reverted target as a deployment failure", async () => {
    const { run, written } = harness((input) =>
      Effect.succeed(
        input.method === "POST"
          ? { status: 202, body: targetBody("queued") }
          : { status: 200, body: targetBody("reverted") },
      ),
    );

    await expect(
      run(["--json", ...promoteArgv(["--wait", "--idempotency-key", "release-key-0004"])]),
    ).rejects.toThrow(/reached reverted/u);
    expect((written[0]?.document.value as Record<string, unknown>)["outcome"]).toBe("reverted");
  });

  it("reports needs_rolling_replace as its own condition, not a failure", async () => {
    const { run, written } = harness((input) =>
      Effect.succeed(
        input.method === "POST"
          ? { status: 202, body: targetBody("queued") }
          : { status: 200, body: targetBody("needs_rolling_replace") },
      ),
    );

    await expect(
      run(["--json", ...promoteArgv(["--wait", "--idempotency-key", "release-key-0005"])]),
    ).rejects.toThrow(/rolling replacement/u);
    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["outcome"]).toBe("needs_rolling_replace");
    expect(value["failure_code"]).toBeNull();
  });

  it("names the operator scope when the account is not an operator", async () => {
    const { run } = harness(() =>
      Effect.succeed(
        envelope(403, "not_operator", "The credential's account is not a current operator"),
      ),
    );

    await expect(run(promoteArgv(["--idempotency-key", "release-key-0006"]))).rejects.toThrow(
      /deployments:promote.*forge:write cannot promote.*auth login --scope deployments:promote/su,
    );
  });

  it("guides a forge:write-only token to the privileged login", async () => {
    const { run } = harness(() =>
      Effect.succeed(
        envelope(401, "unauthenticated", "Requires an API token carrying deployments:promote"),
      ),
    );

    await expect(run(["deploy", "view", targetId])).rejects.toThrow(
      /auth login --scope deployments:promote/u,
    );
  });

  it("shows one target through the status resource", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 200, body: targetBody("deploying") };
      }),
    );

    await run(["--json", "deploy", "view", targetId]);

    expect(requests[0]?.path).toBe(`/api/v1/admin/forge/targets/${targetId}`);
    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["outcome"]).toBe("pending");
    expect(value["terminal"]).toBe(false);
  });

  it("lists bounded recent history with the requested repository and limit", async () => {
    const requests: Array<ApiRequest> = [];
    const { run, written } = harness((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 200,
          body: { repo: "openagents.com", targets: [targetBody("live")] },
        };
      }),
    );

    await run(["--json", "deploy", "list", "--repo", "openagents.com", "--limit", "5"]);

    expect(requests[0]?.path).toBe("/api/v1/admin/forge/targets?repo=openagents.com&limit=5");
    expect(written[0]?.document.value).toEqual({
      repo: "openagents.com",
      targets: [targetBody("live")],
    });
  });

  it("stops polling on interrupt without sending a cancellation", async () => {
    const requests: Array<ApiRequest> = [];
    let polled: (() => void) | undefined;
    const firstPoll = new Promise<void>((resolvePoll) => {
      polled = resolvePoll;
    });
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    try {
      const { program } = harness((input) =>
        Effect.sync(() => {
          requests.push(input);
          if (input.method === "GET") polled?.();
          return input.method === "POST"
            ? { status: 202, body: targetBody("queued") }
            : { status: 200, body: targetBody("building") };
        }),
      );

      const fiber = Effect.runFork(
        program(promoteArgv(["--wait", "--idempotency-key", "release-key-0007"])),
      );
      await Promise.race([
        firstPoll,
        new Promise<never>((_resolve, reject) =>
          setTimeout(() => reject(new Error("the CLI never polled the status resource")), 5_000),
        ),
      ]);
      await Effect.runPromise(Fiber.interrupt(fiber));

      const afterInterrupt = requests.length;
      // Only the promotion POST and status GETs ever went out; interruption
      // sent nothing to the server.
      expect(requests.every((request, index) => index === 0 || request.method === "GET")).toBe(
        true,
      );
      expect(requests[0]?.method).toBe("POST");
      await new Promise((resolveWait) => setTimeout(resolveWait, 50));
      expect(requests.length).toBe(afterInterrupt);
      const hints = errorSpy.mock.calls.map((call) => call.join(" ")).join("\n");
      expect(hints).toContain(`deploy view ${targetId}`);
    } finally {
      errorSpy.mockRestore();
    }
  });
});
