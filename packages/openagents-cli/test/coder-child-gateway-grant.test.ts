import { request as httpRequest } from "node:http";

import { Redacted } from "effect";
import { describe, expect, it, vi, afterEach } from "vitest";

import { startChildGateway, type ChildGrant } from "../src/coder-child-gateway.js";

const grant = (token: string): ChildGrant => ({
  proxyUrl: "https://openagents.test/api/inference/proxy",
  token: Redacted.make(token),
  model: "ox-alpha",
});

/**
 * Call the gateway over raw HTTP.
 *
 * Not `fetch`: these tests replace `fetch` to stand in for the proxy upstream,
 * and a client that also used it would be answered by the stand-in without the
 * gateway ever running.
 */
const call = async (baseUrl: string): Promise<number> => {
  const url = new URL(`${baseUrl}/chat/completions`);
  return await new Promise<number>((resolve, reject) => {
    const request = httpRequest(
      { hostname: url.hostname, port: url.port, path: url.pathname, method: "POST" },
      (response) => {
        response.resume();
        response.on("end", () => resolve(response.statusCode ?? 0));
      },
    );
    request.on("error", reject);
    request.end(JSON.stringify({ messages: [{ role: "user", content: "hi" }] }));
  });
};

afterEach(() => {
  vi.restoreAllMocks();
});

describe("a child gateway whose grant has expired", () => {
  it("mints another and sends the same request on it", async () => {
    const sentWith: string[] = [];
    vi.spyOn(globalThis, "fetch").mockImplementation(
      // eslint-disable-next-line @typescript-eslint/require-await -- a stand-in upstream
      async (_input, init) => {
        const auth = String((init?.headers as Record<string, string>)?.["authorization"] ?? "");
        sentWith.push(auth);
        // A grant minted an hour ago, then one minted moments ago.
        return auth.includes("stale")
          ? new Response(JSON.stringify({ error: { code: "grant_expired" } }), { status: 403 })
          : new Response(JSON.stringify({ ok: true }), { status: 200 });
      },
    );

    const gateway = await startChildGateway(grant("stale"), async () =>
      Promise.resolve(grant("fresh")),
    );

    const status = await call(gateway.baseUrl);

    // The child sees the answer, not the refusal: four children failing on a
    // credential minted at breakfast is the failure this exists to stop.
    expect(status).toBe(200);
    expect(sentWith.some((auth) => auth.includes("stale"))).toBe(true);
    expect(sentWith.some((auth) => auth.includes("fresh"))).toBe(true);

    await gateway.close();
  });

  it("reports the refusal when no fresh grant can be had", async () => {
    vi.spyOn(globalThis, "fetch").mockImplementation(async () =>
      Promise.resolve(
        new Response(JSON.stringify({ error: { code: "grant_expired" } }), { status: 403 }),
      ),
    );

    const gateway = await startChildGateway(grant("stale"), async () => Promise.resolve(undefined));

    // Reported rather than retried forever.
    expect(await call(gateway.baseUrl)).toBe(403);

    await gateway.close();
  });

  it("does not retry a caller that cannot mint one", async () => {
    let calls = 0;
    vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
      calls += 1;
      return await Promise.resolve(
        new Response(JSON.stringify({ error: { code: "grant_expired" } }), { status: 403 }),
      );
    });

    const gateway = await startChildGateway(grant("stale"));
    await call(gateway.baseUrl);

    expect(calls).toBe(1);
    await gateway.close();
  });
});
