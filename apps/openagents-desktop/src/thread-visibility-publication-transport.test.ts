import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadVisibilityPublicationPath,
  DesktopThreadVisibilityPublicationRequestSchemaLiteral,
  publishDesktopThreadPublicVisibility,
} from "./thread-visibility-publication-transport.ts";

const THREAD = "thread.visibility.publication.1";

const receipt = (audience: unknown = { kind: "internet_readable" }) => ({
  schema: "openagents.thread_disclosure_receipt.v1",
  receiptRef: "receipt.visibility.publication.1",
  intentRef: "intent.visibility.publication.1",
  idempotencyKey: "idempotency.visibility.publication.1",
  threadRef: THREAD,
  observedAt: "2026-07-17T19:50:00Z",
  kind: "thread.visibility.set",
  result: {
    status: "visibility_applied",
    visibilityVersion: 4,
    target: { audience, administratorAccess: { kind: "none" } },
  },
});

const request = (overrides: Record<string, unknown> = {}) => ({
  schema: DesktopThreadVisibilityPublicationRequestSchemaLiteral,
  receipt: receipt(),
  authorization: {
    status: "authorized",
    basis: "owner",
    receiptRef: "receipt.visibility.publication.1",
    threadRef: THREAD,
    visibilityVersion: 4,
  },
  source: { kind: "agent-run", id: THREAD },
  ...overrides,
});

type ObservedCall = Readonly<{ url: string; init: RequestInit | undefined }>;

const harness = (
  input: Readonly<{
    token?: string | null;
    response?: Response;
    throwFetch?: boolean;
    baseUrl?: string;
  }> = {},
) => {
  const calls: Array<ObservedCall> = [];
  let credentialReads = 0;
  return {
    dependencies: {
      baseUrl: input.baseUrl ?? "https://openagents.test/base",
      accessToken: () => {
        credentialReads += 1;
        return input.token === undefined ? "host-secret" : input.token;
      },
      fetch: async (url: string | URL | Request, init?: RequestInit) => {
        calls.push({ url: String(url), init });
        if (input.throwFetch) throw new Error("network detail");
        return (
          input.response ??
          Response.json(
            {
              id: "share.publication.1",
              url: "https://openagents.test/share/share.publication.1",
              audienceLabel: "Public",
              status: "active",
            },
            { status: 201 },
          )
        );
      },
    },
    calls,
    credentialReads: () => credentialReads,
  };
};

const run = (dependencies: ReturnType<typeof harness>["dependencies"], input: unknown) =>
  Effect.runPromise(publishDesktopThreadPublicVisibility(dependencies, input));

describe("Desktop public-visibility publication transport", () => {
  test("publishes one ref-only public share through the existing route", async () => {
    const testHarness = harness();
    await expect(run(testHarness.dependencies, request())).resolves.toEqual({
      status: "published",
      shareRef: "share.publication.1",
      url: "https://openagents.test/share/share.publication.1",
      receiptRef: "receipt.visibility.publication.1",
      threadRef: THREAD,
      visibilityVersion: 4,
    });
    expect(testHarness.credentialReads()).toBe(1);
    expect(testHarness.calls).toHaveLength(1);
    const call = testHarness.calls[0]!;
    expect(call.url).toBe(`https://openagents.test${DesktopThreadVisibilityPublicationPath}`);
    expect(call.init?.method).toBe("POST");
    expect(new Headers(call.init?.headers).get("authorization")).toBe("Bearer host-secret");
    expect(JSON.parse(String(call.init?.body))).toEqual({
      source: { kind: "agent-run", id: THREAD },
      audience: { _tag: "Public" },
    });
    expect(String(call.init?.body)).not.toMatch(
      /transcript|message|export|path|credential|provider/i,
    );
  });

  test("supports an exact team-thread source without adding team or content fields", async () => {
    const testHarness = harness();
    await expect(
      run(testHarness.dependencies, request({ source: { kind: "team-thread", id: THREAD } })),
    ).resolves.toMatchObject({ status: "published", threadRef: THREAD });
    expect(JSON.parse(String(testHarness.calls[0]?.init?.body))).toEqual({
      source: { kind: "team-thread", id: THREAD },
      audience: { _tag: "Public" },
    });
  });

  test("rejects malformed and mismatched authority before reading credentials", async () => {
    const cases = [
      { ...request(), raw: "content" },
      request({ authorization: { ...request().authorization, basis: "internet_readable" } }),
      request({ authorization: { ...request().authorization, receiptRef: "receipt.other" } }),
      request({ authorization: { ...request().authorization, visibilityVersion: 5 } }),
      request({ source: { kind: "agent-run", id: "thread.other" } }),
      request({ source: { kind: "unsupported", id: THREAD } }),
      request({ receipt: { ...receipt(), raw: "content" } }),
    ];
    for (const value of cases) {
      const testHarness = harness();
      await expect(run(testHarness.dependencies, value)).resolves.toEqual({
        status: "rejected",
        reason: "invalid_request",
      });
      expect(testHarness.credentialReads()).toBe(0);
      expect(testHarness.calls).toHaveLength(0);
    }
  });

  test("rejects non-public visibility without dispatch", async () => {
    const testHarness = harness();
    await expect(
      run(
        testHarness.dependencies,
        request({
          receipt: receipt({ kind: "owner_only" }),
        }),
      ),
    ).resolves.toEqual({ status: "rejected", reason: "unsupported_visibility" });
    expect(testHarness.credentialReads()).toBe(0);
    expect(testHarness.calls).toHaveLength(0);
  });

  test("requires host-custodied credentials and a valid service origin", async () => {
    const missing = harness({ token: null });
    await expect(run(missing.dependencies, request())).resolves.toEqual({
      status: "rejected",
      reason: "authentication_required",
    });
    expect(missing.calls).toHaveLength(0);

    const invalidOrigin = harness({ baseUrl: "file:///private/path" });
    await expect(run(invalidOrigin.dependencies, request())).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(invalidOrigin.calls).toHaveLength(0);
  });

  test("maps definitive HTTP rejections without reading response details", async () => {
    for (const [status, reason] of [
      [401, "authentication_required"],
      [403, "publication_forbidden"],
      [400, "publication_rejected"],
      [422, "publication_rejected"],
    ] as const) {
      const testHarness = harness({ response: new Response("private detail", { status }) });
      await expect(run(testHarness.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason,
      });
      expect(testHarness.calls).toHaveLength(1);
    }
  });

  test("reports ambiguous delivery without retrying or claiming failure", async () => {
    const cases = [
      harness({ throwFetch: true }),
      harness({ response: new Response("", { status: 500 }) }),
      harness({ response: new Response("", { status: 429 }) }),
      harness({ response: new Response("not-json", { status: 201 }) }),
      harness({
        response: Response.json(
          {
            id: "share.publication.1",
            url: "https://attacker.test/share/share.publication.1",
            audienceLabel: "Public",
            status: "active",
          },
          { status: 201 },
        ),
      }),
    ];
    for (const testHarness of cases) {
      await expect(run(testHarness.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason: "publication_outcome_unknown",
      });
      expect(testHarness.calls).toHaveLength(1);
    }
  });

  test("rejects expanded or inactive success responses as ambiguous outcomes", async () => {
    for (const payload of [
      {
        id: "share.publication.1",
        url: "https://openagents.test/share/share.publication.1",
        audienceLabel: "Public",
        status: "active",
        raw: "content",
      },
      {
        id: "share.publication.1",
        url: "https://openagents.test/share/share.publication.1",
        audienceLabel: "Public",
        status: "revoked",
      },
    ]) {
      const testHarness = harness({ response: Response.json(payload, { status: 201 }) });
      await expect(run(testHarness.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason: "publication_outcome_unknown",
      });
    }
  });
});
