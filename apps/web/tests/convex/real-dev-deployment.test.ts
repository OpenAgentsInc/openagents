import { ConvexHttpClient } from "convex/browser";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { api } from "../../convex/_generated/api";

const enabled = String(process.env.OA_REAL_CONVEX_TESTS ?? "") === "1";
const convexUrl = process.env.VITE_CONVEX_URL ?? process.env.CONVEX_URL ?? null;
const secret = process.env.OA_TEST_RESET_SECRET ?? null;

describe("Convex real dev deployment (optional)", () => {
  if (!enabled) {
    it.skip("set OA_REAL_CONVEX_TESTS=1 to enable real Convex deployment tests", () => {});
    return;
  }

  if (!convexUrl) {
    throw new Error("OA_REAL_CONVEX_TESTS=1 requires VITE_CONVEX_URL (or CONVEX_URL)");
  }
  if (!secret) {
    throw new Error("OA_REAL_CONVEX_TESTS=1 requires OA_TEST_RESET_SECRET (matches Convex env var)");
  }

  const client = new ConvexHttpClient(convexUrl, { logger: false });

  const reset = async () => {
    const res = await client.mutation(api.testing.resetAll, { secret } as any);
    if (!(res as any)?.ok) throw new Error(`resetAll failed: ${JSON.stringify(res)}`);
  };

  beforeAll(async () => {
    await reset();
  });

  afterAll(async () => {
    await reset();
  });

  it("can create and query an anon thread after wipe", async () => {
    const threadId = `test-${Date.now()}`;
    const anonKey = `anon-${Math.random().toString(36).slice(2)}`;

    const ensured = await client.mutation(api.autopilot.threads.ensureAnonThread, { threadId, anonKey } as any);
    expect((ensured as any)?.ok).toBe(true);

    const snap = await client.query(api.autopilot.messages.getThreadSnapshot, {
      threadId,
      anonKey,
      maxMessages: 200,
      maxParts: 5000,
    } as any);

    expect((snap as any)?.ok).toBe(true);
    expect(Array.isArray((snap as any)?.messages)).toBe(true);
    // Welcome seed message.
    expect((snap as any).messages.length).toBeGreaterThan(0);
  });
});

