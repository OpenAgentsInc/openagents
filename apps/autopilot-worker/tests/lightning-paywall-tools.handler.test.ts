import { describe, expect, it } from "vitest";

import {
  executeLightningPaywallTool,
  type ExecuteLightningPaywallToolOptions,
} from "../src/lightningPaywallControlPlane";

type FetchStub = NonNullable<ExecuteLightningPaywallToolOptions["fetchImpl"]>;

const jsonResponse = (status: number, body: unknown, headers?: HeadersInit): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(headers ?? {}),
    },
  });

const baseEnv = {
  LIGHTNING_CONTROL_PLANE_BASE_URL: "https://openagents.example",
  LIGHTNING_CONTROL_PLANE_AUTH_TOKEN: "test-token",
};

const defaultCreateInput = {
  name: "Demo",
  policy: {
    pricingMode: "fixed",
    fixedAmountMsats: 1200,
  },
  routes: [
    {
      hostPattern: "api.example.com",
      pathPattern: "/premium",
      upstreamUrl: "https://upstream.example.com/premium",
      protocol: "https",
      timeoutMs: 5000,
      priority: 1,
    },
  ],
};

describe("lightning paywall tool handler execution", () => {
  it("returns success envelopes for create and emits deterministic receipt hashes", async () => {
    let seenAuthorization: string | null = null;

    const fetchStub: FetchStub = async (input, init) => {
      const url = String(input);
      expect(url).toContain("/api/lightning/paywalls");
      expect(init?.method).toBe("POST");
      seenAuthorization =
        init?.headers instanceof Headers
          ? init.headers.get("authorization")
          : new Headers(init?.headers).get("authorization");

      return jsonResponse(
        200,
        {
          ok: true,
          requestId: "req_create_1",
          paywall: {
            paywallId: "pw_1",
            ownerId: "owner_1",
            name: "Demo",
            status: "active",
            policy: {
              fixedAmountMsats: 1200,
            },
            routes: [{ routeId: "r1" }],
          },
        },
        { "x-oa-request-id": "req_header_1" },
      );
    };

    const first = await executeLightningPaywallTool({
      toolName: "lightning_paywall_create",
      input: defaultCreateInput,
      env: baseEnv,
      fetchImpl: fetchStub,
    });

    const second = await executeLightningPaywallTool({
      toolName: "lightning_paywall_create",
      input: defaultCreateInput,
      env: baseEnv,
      fetchImpl: fetchStub,
    });

    expect(seenAuthorization).toBe("Bearer test-token");

    expect(first.status).toBe("ok");
    expect(first.httpStatus).toBe(200);
    expect(first.requestId).toBe("req_create_1");
    expect(first.paywall?.paywallId).toBe("pw_1");
    expect(first.paywall?.routeCount).toBe(1);
    expect(first.paywall?.fixedAmountMsats).toBe(1200);

    expect(first.receipt.params_hash.startsWith("sha256:")).toBe(true);
    expect(first.receipt.output_hash.startsWith("sha256:")).toBe(true);
    expect(first.receipt.latency_ms).toBeGreaterThanOrEqual(0);
    expect(first.receipt.side_effects).toHaveLength(1);
    expect(first.receipt.side_effects[0]?.status_code).toBe(200);
    expect(first.receipt.side_effects[0]?.changed).toBe(true);

    expect(first.receipt.params_hash).toBe(second.receipt.params_hash);
    expect(first.receipt.output_hash).toBe(second.receipt.output_hash);
  });

  it("maps denial surfaces (invalid_route, paused, not_authorized, over_cap)", async () => {
    const fetchStub: FetchStub = async (input) => {
      const url = String(input);
      if (url.includes("/pause")) {
        return jsonResponse(422, { ok: false, error: "paywall_inactive" });
      }
      if (url.includes("/resume")) {
        return jsonResponse(401, { ok: false, error: "unauthorized" });
      }
      if (url.includes("/paywalls/")) {
        return jsonResponse(409, { ok: false, error: "route_conflict" });
      }
      return jsonResponse(429, { ok: false, error: "over_cap" });
    };

    const invalidRoute = await executeLightningPaywallTool({
      toolName: "lightning_paywall_update",
      input: {
        paywallId: "pw_1",
        name: "Updated",
      },
      env: baseEnv,
      fetchImpl: fetchStub,
    });
    expect(invalidRoute.status).toBe("denied");
    expect(invalidRoute.denyCode).toBe("invalid_route");

    const paused = await executeLightningPaywallTool({
      toolName: "lightning_paywall_pause",
      input: {
        paywallId: "pw_1",
        reason: "maintenance",
      },
      env: baseEnv,
      fetchImpl: fetchStub,
    });
    expect(paused.status).toBe("denied");
    expect(paused.denyCode).toBe("paused");

    const unauthorized = await executeLightningPaywallTool({
      toolName: "lightning_paywall_resume",
      input: {
        paywallId: "pw_1",
        reason: "live",
      },
      env: baseEnv,
      fetchImpl: fetchStub,
    });
    expect(unauthorized.status).toBe("denied");
    expect(unauthorized.denyCode).toBe("not_authorized");

    const overCap = await executeLightningPaywallTool({
      toolName: "lightning_paywall_list",
      input: {
        status: "active",
        limit: 25,
      },
      env: baseEnv,
      fetchImpl: fetchStub,
    });
    expect(overCap.status).toBe("denied");
    expect(overCap.denyCode).toBe("over_cap");
  });

  it("maps upstream server failures to explicit error envelopes", async () => {
    const fetchStub: FetchStub = async () =>
      jsonResponse(500, { ok: false, error: "internal_error" });

    const output = await executeLightningPaywallTool({
      toolName: "lightning_paywall_get",
      input: {
        paywallId: "pw_500",
      },
      env: baseEnv,
      fetchImpl: fetchStub,
    });

    expect(output.status).toBe("error");
    expect(output.errorCode).toBe("upstream_http_500");
    expect(output.errorMessage).toContain("internal_error");
    expect(output.receipt.side_effects[0]?.status_code).toBe(500);
  });

  it("returns not_configured denial when control-plane base URL is missing", async () => {
    const output = await executeLightningPaywallTool({
      toolName: "lightning_paywall_get",
      input: {
        paywallId: "pw_1",
      },
      env: {},
      fetchImpl: async () => {
        throw new Error("should_not_call_fetch_without_base_url");
      },
    });

    expect(output.status).toBe("denied");
    expect(output.denyCode).toBe("not_configured");
    expect(output.denyReason).toContain("LIGHTNING_CONTROL_PLANE_BASE_URL");
    expect(output.receipt.side_effects[0]?.detail).toBe("control_plane_not_configured");
  });

  it("lists settlements deterministically with cursor handling", async () => {
    const fetchStub: FetchStub = async (input) => {
      const url = String(input);
      expect(url).toContain("/api/lightning/paywalls/pw_1/settlements");
      return jsonResponse(200, {
        ok: true,
        requestId: "req_set_1",
        nextCursor: 1_730_000_000_000,
        settlements: [
          {
            settlementId: "set_older",
            paywallId: "pw_1",
            amountMsats: 1000,
            paymentProofRef: "lightning_preimage:aaa",
            createdAtMs: 1_729_000_000_000,
          },
          {
            settlementId: "set_newer",
            paywallId: "pw_1",
            amountMsats: 1500,
            paymentProofRef: "lightning_preimage:bbb",
            createdAtMs: 1_731_000_000_000,
          },
        ],
      });
    };

    const output = await executeLightningPaywallTool({
      toolName: "lightning_paywall_settlement_list",
      input: {
        paywallId: "pw_1",
        limit: 20,
      },
      env: baseEnv,
      fetchImpl: fetchStub,
    });

    expect(output.status).toBe("ok");
    expect(output.nextCursor).toBe(1_730_000_000_000);
    expect(output.settlements).toHaveLength(2);
    expect(output.settlements[0]?.settlementId).toBe("set_newer");
    expect(output.settlements[1]?.settlementId).toBe("set_older");
    expect(output.receipt.side_effects[0]?.changed).toBe(false);
  });
});
