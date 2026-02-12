import { createServer } from "node:http";

import { executeLightningPaywallTool } from "../src/lightningPaywallControlPlane";

type SmokeCase = {
  readonly name: string;
  readonly toolName:
    | "lightning_paywall_create"
    | "lightning_paywall_update"
    | "lightning_paywall_pause"
    | "lightning_paywall_resume"
    | "lightning_paywall_get"
    | "lightning_paywall_list"
    | "lightning_paywall_settlement_list";
  readonly input: Record<string, unknown>;
  readonly expect: {
    readonly status: "ok" | "denied" | "error";
    readonly denyCode?: string;
    readonly errorCode?: string;
  };
};

const isJsonMode = process.argv.includes("--json");

function assertCondition(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

const paywallBody = {
  paywallId: "pw_1",
  ownerId: "owner_1",
  name: "Demo paywall",
  status: "active",
  policy: {
    fixedAmountMsats: 1200,
  },
  routes: [{ routeId: "r_1" }],
};

const readJsonBody = async (req: import("node:http").IncomingMessage): Promise<unknown> => {
  const chunks: Array<Buffer> = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  const text = Buffer.concat(chunks).toString("utf8");
  if (!text.trim()) return null;
  return JSON.parse(text);
};

const startControlPlaneStub = async () => {
  const server = createServer(async (req, res) => {
    const method = req.method ?? "GET";
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    const auth = req.headers.authorization;

    res.setHeader("content-type", "application/json; charset=utf-8");
    res.setHeader("cache-control", "no-store");

    if (auth !== "Bearer smoke-token") {
      res.statusCode = 401;
      res.end(JSON.stringify({ ok: false, error: "unauthorized" }));
      return;
    }

    if (method === "POST" && url.pathname === "/api/lightning/paywalls") {
      const body = (await readJsonBody(req)) as Record<string, unknown> | null;
      const name = typeof body?.name === "string" ? body.name : "Demo paywall";
      res.statusCode = 200;
      res.end(
        JSON.stringify({
          ok: true,
          requestId: "req_create",
          paywall: {
            ...paywallBody,
            name,
          },
        }),
      );
      return;
    }

    if (method === "GET" && url.pathname === "/api/lightning/paywalls") {
      res.statusCode = 200;
      res.end(
        JSON.stringify({
          ok: true,
          requestId: "req_list",
          paywalls: [
            paywallBody,
            {
              ...paywallBody,
              paywallId: "pw_2",
              name: "Second paywall",
            },
          ],
        }),
      );
      return;
    }

    if (method === "GET" && url.pathname === "/api/lightning/settlements") {
      res.statusCode = 200;
      res.end(
        JSON.stringify({
          ok: true,
          requestId: "req_set_owner",
          settlements: [
            {
              settlementId: "set_older",
              paywallId: "pw_1",
              amountMsats: 1000,
              paymentProofRef: "lightning_preimage:aaaa",
              createdAtMs: 10,
            },
            {
              settlementId: "set_newer",
              paywallId: "pw_2",
              amountMsats: 2000,
              paymentProofRef: "lightning_preimage:bbbb",
              createdAtMs: 20,
            },
          ],
          nextCursor: 5,
        }),
      );
      return;
    }

    if (method === "GET" && url.pathname === "/api/lightning/paywalls/pw_1") {
      res.statusCode = 200;
      res.end(JSON.stringify({ ok: true, requestId: "req_get", paywall: paywallBody }));
      return;
    }

    if (method === "PATCH" && url.pathname === "/api/lightning/paywalls/pw_1") {
      res.statusCode = 200;
      res.end(JSON.stringify({ ok: true, requestId: "req_update", paywall: paywallBody }));
      return;
    }

    if (method === "PATCH" && url.pathname === "/api/lightning/paywalls/pw_denied") {
      res.statusCode = 409;
      res.end(JSON.stringify({ ok: false, error: "route_conflict" }));
      return;
    }

    if (method === "POST" && url.pathname === "/api/lightning/paywalls/pw_1/pause") {
      res.statusCode = 200;
      res.end(JSON.stringify({ ok: true, requestId: "req_pause", changed: true, paywall: { ...paywallBody, status: "paused" } }));
      return;
    }

    if (method === "POST" && url.pathname === "/api/lightning/paywalls/pw_1/resume") {
      res.statusCode = 200;
      res.end(JSON.stringify({ ok: true, requestId: "req_resume", changed: true, paywall: { ...paywallBody, status: "active" } }));
      return;
    }

    if (method === "GET" && url.pathname === "/api/lightning/paywalls/pw_1/settlements") {
      res.statusCode = 200;
      res.end(
        JSON.stringify({
          ok: true,
          requestId: "req_set_paywall",
          settlements: [
            {
              settlementId: "set_1",
              paywallId: "pw_1",
              amountMsats: 1100,
              paymentProofRef: "lightning_preimage:cccc",
              createdAtMs: 11,
            },
          ],
          nextCursor: null,
        }),
      );
      return;
    }

    if (method === "GET" && url.pathname === "/api/lightning/paywalls/pw_error") {
      res.statusCode = 500;
      res.end(JSON.stringify({ ok: false, error: "internal_error" }));
      return;
    }

    res.statusCode = 404;
    res.end(JSON.stringify({ ok: false, error: "not_found" }));
  });

  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve());
  });

  const addr = server.address();
  if (!addr || typeof addr === "string") {
    throw new Error("failed_to_bind_control_plane_stub");
  }

  return {
    close: async () =>
      new Promise<void>((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      }),
    baseUrl: `http://127.0.0.1:${addr.port}`,
  };
};

const run = async () => {
  const stub = await startControlPlaneStub();
  try {
    const env = {
      LIGHTNING_CONTROL_PLANE_BASE_URL: stub.baseUrl,
      LIGHTNING_CONTROL_PLANE_AUTH_TOKEN: "smoke-token",
    };

    const cases: ReadonlyArray<SmokeCase> = [
      {
        name: "create",
        toolName: "lightning_paywall_create",
        input: {
          name: "Demo paywall",
          policy: { pricingMode: "fixed", fixedAmountMsats: 1200 },
          routes: [{ hostPattern: "api.example.com", pathPattern: "/premium", upstreamUrl: "https://upstream.example.com/premium" }],
        },
        expect: { status: "ok" },
      },
      {
        name: "update",
        toolName: "lightning_paywall_update",
        input: { paywallId: "pw_1", name: "Updated" },
        expect: { status: "ok" },
      },
      {
        name: "pause",
        toolName: "lightning_paywall_pause",
        input: { paywallId: "pw_1", reason: "maintenance" },
        expect: { status: "ok" },
      },
      {
        name: "resume",
        toolName: "lightning_paywall_resume",
        input: { paywallId: "pw_1", reason: "back_online" },
        expect: { status: "ok" },
      },
      {
        name: "get",
        toolName: "lightning_paywall_get",
        input: { paywallId: "pw_1" },
        expect: { status: "ok" },
      },
      {
        name: "list",
        toolName: "lightning_paywall_list",
        input: { status: "active", limit: 20 },
        expect: { status: "ok" },
      },
      {
        name: "settlement_list_owner",
        toolName: "lightning_paywall_settlement_list",
        input: { limit: 20 },
        expect: { status: "ok" },
      },
      {
        name: "settlement_list_paywall",
        toolName: "lightning_paywall_settlement_list",
        input: { paywallId: "pw_1", limit: 20 },
        expect: { status: "ok" },
      },
      {
        name: "denied_invalid_route",
        toolName: "lightning_paywall_update",
        input: { paywallId: "pw_denied", name: "Denied" },
        expect: { status: "denied", denyCode: "invalid_route" },
      },
      {
        name: "error_upstream",
        toolName: "lightning_paywall_get",
        input: { paywallId: "pw_error" },
        expect: { status: "error", errorCode: "upstream_http_500" },
      },
    ];

    const results: Array<{
      readonly name: string;
      readonly status: string;
      readonly denyCode: string | null;
      readonly errorCode: string | null;
      readonly paramsHash: string;
      readonly outputHash: string;
    }> = [];

    for (const item of cases) {
      const output = await executeLightningPaywallTool({
        toolName: item.toolName,
        input: item.input,
        env,
      });

      assertCondition(
        output.status === item.expect.status,
        `${item.name}: expected status=${item.expect.status}, got ${output.status}`,
      );
      if (item.expect.denyCode) {
        assertCondition(
          output.denyCode === item.expect.denyCode,
          `${item.name}: expected denyCode=${item.expect.denyCode}, got ${output.denyCode}`,
        );
      }
      if (item.expect.errorCode) {
        assertCondition(
          output.errorCode === item.expect.errorCode,
          `${item.name}: expected errorCode=${item.expect.errorCode}, got ${output.errorCode}`,
        );
      }
      assertCondition(output.receipt.params_hash.startsWith("sha256:"), `${item.name}: missing params_hash`);
      assertCondition(output.receipt.output_hash.startsWith("sha256:"), `${item.name}: missing output_hash`);
      assertCondition(output.receipt.side_effects.length >= 0, `${item.name}: invalid side_effects`);

      results.push({
        name: item.name,
        status: output.status,
        denyCode: output.denyCode,
        errorCode: output.errorCode,
        paramsHash: output.receipt.params_hash,
        outputHash: output.receipt.output_hash,
      });
    }

    if (isJsonMode) {
      console.log(JSON.stringify({ ok: true, total: results.length, results }, null, 2));
    } else {
      console.log(`[paywall-tools-smoke] ok total=${results.length}`);
      for (const row of results) {
        console.log(
          `[paywall-tools-smoke] ${row.name} status=${row.status} deny=${row.denyCode ?? "-"} error=${row.errorCode ?? "-"}`,
        );
      }
    }
  } finally {
    await stub.close();
  }
};

run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
