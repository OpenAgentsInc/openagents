import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadVisibilityWorkspacePublicationPath,
  DesktopThreadVisibilityWorkspacePublicationRequestSchemaLiteral,
  publishDesktopThreadWorkspaceVisibility,
} from "./thread-visibility-workspace-publication-transport.ts";

const THREAD = "thread.visibility.workspace.publication.1";
const WORKSPACE = "scope.team.team_1";

const receipt = (audience: unknown = { kind: "workspace_members", workspaceRef: WORKSPACE }) => ({
  schema: "openagents.thread_disclosure_receipt.v1",
  receiptRef: "receipt.visibility.workspace.publication.1",
  intentRef: "intent.visibility.workspace.publication.1",
  idempotencyKey: "idempotency.visibility.workspace.publication.1",
  threadRef: THREAD,
  observedAt: "2026-07-17T20:08:00Z",
  kind: "thread.visibility.set",
  result: {
    status: "visibility_applied",
    visibilityVersion: 5,
    target: {
      audience,
      administratorAccess: { kind: "workspace_admins", workspaceRef: WORKSPACE },
    },
  },
});

const request = (overrides: Record<string, unknown> = {}) => ({
  schema: DesktopThreadVisibilityWorkspacePublicationRequestSchemaLiteral,
  receipt: receipt(),
  authorization: {
    status: "authorized",
    basis: "owner",
    receiptRef: "receipt.visibility.workspace.publication.1",
    threadRef: THREAD,
    visibilityVersion: 5,
  },
  source: { kind: "agent-run", id: THREAD },
  teamName: "OpenAgents Core",
  ...overrides,
});

type ObservedCall = Readonly<{ url: string; init: RequestInit | undefined }>;

const harness = (
  input: Readonly<{
    token?: string | null;
    response?: Response;
    responses?: ReadonlyArray<Response | Error>;
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
        const scripted = input.responses?.[calls.length - 1];
        if (scripted instanceof Error) throw scripted;
        if (scripted !== undefined) return scripted;
        if (input.throwFetch) throw new Error("network detail");
        return (
          input.response ??
          Response.json(
            {
              id: "share.workspace.publication.1",
              url: "https://openagents.test/share/share.workspace.publication.1",
              audienceLabel: "Shared with members of OpenAgents Core",
              status: "active",
            },
            { headers: { "Idempotency-Replayed": "false" }, status: 201 },
          )
        );
      },
    },
    calls,
    credentialReads: () => credentialReads,
  };
};

const run = (dependencies: ReturnType<typeof harness>["dependencies"], input: unknown) =>
  Effect.runPromise(publishDesktopThreadWorkspaceVisibility(dependencies, input));

describe("Desktop workspace-visibility publication transport", () => {
  test("publishes one ref-only TeamMembers share through the existing route", async () => {
    const value = harness();
    await expect(run(value.dependencies, request())).resolves.toEqual({
      status: "published",
      shareRef: "share.workspace.publication.1",
      url: "https://openagents.test/share/share.workspace.publication.1",
      receiptRef: "receipt.visibility.workspace.publication.1",
      threadRef: THREAD,
      visibilityVersion: 5,
      workspaceRef: WORKSPACE,
    });
    expect(value.credentialReads()).toBe(1);
    expect(value.calls).toHaveLength(1);
    const call = value.calls[0]!;
    expect(call.url).toBe(
      `https://openagents.test${DesktopThreadVisibilityWorkspacePublicationPath}`,
    );
    expect(call.init?.method).toBe("POST");
    expect(new Headers(call.init?.headers).get("authorization")).toBe("Bearer host-secret");
    const key = new Headers(call.init?.headers).get("Idempotency-Key");
    expect(key).toMatch(/^desktop-workspace-share\.[a-f0-9]{64}$/);
    expect(key?.length).toBeLessThanOrEqual(128);
    expect(JSON.parse(String(call.init?.body))).toEqual({
      source: { kind: "agent-run", id: THREAD },
      audience: { _tag: "TeamMembers", teamId: "team_1", teamName: "OpenAgents Core" },
    });
    expect(String(call.init?.body)).not.toMatch(
      /transcript|message|export|path|credential|provider|group/i,
    );
  });

  test("binds team-thread source identity to the target team", async () => {
    const value = harness();
    await expect(
      run(
        value.dependencies,
        request({ source: { kind: "team-thread", id: THREAD, teamId: "team_1" } }),
      ),
    ).resolves.toMatchObject({ status: "published", workspaceRef: WORKSPACE });
    expect(JSON.parse(String(value.calls[0]?.init?.body))).toEqual({
      source: { kind: "team-thread", id: THREAD, teamId: "team_1" },
      audience: { _tag: "TeamMembers", teamId: "team_1", teamName: "OpenAgents Core" },
    });
  });

  test("accepts the exact workspace-member authorization basis", async () => {
    const value = harness();
    await expect(
      run(
        value.dependencies,
        request({ authorization: { ...request().authorization, basis: "workspace_member" } }),
      ),
    ).resolves.toMatchObject({ status: "published" });
  });

  test("rejects malformed or mismatched authority before reading credentials", async () => {
    const cases = [
      { ...request(), raw: "content" },
      request({ authorization: { ...request().authorization, basis: "named_group" } }),
      request({ authorization: { ...request().authorization, receiptRef: "receipt.other" } }),
      request({ authorization: { ...request().authorization, visibilityVersion: 6 } }),
      request({ source: { kind: "agent-run", id: "thread.other" } }),
      request({ source: { kind: "team-thread", id: THREAD, teamId: "team_other" } }),
      request({ receipt: { ...receipt(), raw: "content" } }),
      request({ teamName: " OpenAgents Core" }),
    ];
    for (const candidate of cases) {
      const value = harness();
      await expect(run(value.dependencies, candidate)).resolves.toEqual({
        status: "rejected",
        reason: "invalid_request",
      });
      expect(value.credentialReads()).toBe(0);
      expect(value.calls).toHaveLength(0);
    }
  });

  test("rejects non-team scopes and non-workspace visibility without dispatch", async () => {
    for (const candidate of [
      request({
        receipt: receipt({ kind: "workspace_members", workspaceRef: "workspace.1" }),
      }),
      request({ receipt: receipt({ kind: "internet_readable" }) }),
    ]) {
      const value = harness();
      await expect(run(value.dependencies, candidate)).resolves.toMatchObject({
        status: "rejected",
      });
      expect(value.credentialReads()).toBe(0);
      expect(value.calls).toHaveLength(0);
    }
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
    expect(invalidOrigin.credentialReads()).toBe(0);
    expect(invalidOrigin.calls).toHaveLength(0);
  });

  test("maps definitive HTTP rejections without reading response details", async () => {
    for (const [status, reason] of [
      [401, "authentication_required"],
      [403, "publication_forbidden"],
      [400, "publication_rejected"],
      [409, "publication_rejected"],
      [422, "publication_rejected"],
    ] as const) {
      const value = harness({ response: new Response("private detail", { status }) });
      await expect(run(value.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason,
      });
      expect(value.calls).toHaveLength(1);
    }
  });

  test("retries one ambiguous delivery with the same key and accepts exact replay", async () => {
    const value = harness({
      responses: [
        new Response("", { status: 503 }),
        Response.json(
          {
            id: "share.workspace.publication.1",
            url: "https://openagents.test/share/share.workspace.publication.1",
            audienceLabel: "Shared with members of OpenAgents Core",
            status: "active",
          },
          { headers: { "Idempotency-Replayed": "true" }, status: 200 },
        ),
      ],
    });

    await expect(run(value.dependencies, request())).resolves.toMatchObject({
      status: "published",
      shareRef: "share.workspace.publication.1",
    });
    expect(value.credentialReads()).toBe(1);
    expect(value.calls).toHaveLength(2);
    const first = value.calls[0]!;
    const second = value.calls[1]!;
    expect(new Headers(second.init?.headers).get("Idempotency-Key")).toBe(
      new Headers(first.init?.headers).get("Idempotency-Key"),
    );
    expect(second.init?.body).toBe(first.init?.body);
  });

  test("retries a transport failure once and accepts a first creation", async () => {
    const value = harness({
      responses: [
        new Error("network detail"),
        Response.json(
          {
            id: "share.workspace.publication.1",
            url: "https://openagents.test/share/share.workspace.publication.1",
            audienceLabel: "Shared with members of OpenAgents Core",
            status: "active",
          },
          { headers: { "Idempotency-Replayed": "false" }, status: 201 },
        ),
      ],
    });
    await expect(run(value.dependencies, request())).resolves.toMatchObject({
      status: "published",
    });
    expect(value.calls).toHaveLength(2);
  });

  test("reconciles wrong-audience first-response evidence through an exact replay", async () => {
    const value = harness({
      responses: [
        Response.json(
          {
            id: "share.workspace.publication.1",
            url: "https://openagents.test/share/share.workspace.publication.1",
            audienceLabel: "Shared publicly",
            status: "active",
          },
          { headers: { "Idempotency-Replayed": "false" }, status: 201 },
        ),
        Response.json(
          {
            id: "share.workspace.publication.1",
            url: "https://openagents.test/share/share.workspace.publication.1",
            audienceLabel: "Shared with members of OpenAgents Core",
            status: "active",
          },
          { headers: { "Idempotency-Replayed": "true" }, status: 200 },
        ),
      ],
    });

    await expect(run(value.dependencies, request())).resolves.toMatchObject({
      status: "published",
      workspaceRef: WORKSPACE,
    });
    expect(value.calls).toHaveLength(2);
  });

  test("reports ambiguous delivery after exactly one same-key retry", async () => {
    const cases = [
      harness({ throwFetch: true }),
      harness({ response: new Response("", { status: 500 }) }),
      harness({ response: new Response("", { status: 429 }) }),
      harness({ response: new Response("not-json", { status: 201 }) }),
      harness({
        response: Response.json(
          {
            id: "share.workspace.publication.1",
            url: "https://attacker.test/share/share.workspace.publication.1",
            audienceLabel: "Shared with members of OpenAgents Core",
            status: "active",
          },
          { status: 201 },
        ),
      }),
    ];
    for (const value of cases) {
      await expect(run(value.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason: "publication_outcome_unknown",
      });
      expect(value.calls).toHaveLength(2);
      expect(new Headers(value.calls[1]?.init?.headers).get("Idempotency-Key")).toBe(
        new Headers(value.calls[0]?.init?.headers).get("Idempotency-Key"),
      );
    }
  });

  test("requires the FF-D1-27 first-create/replay status header contract", async () => {
    for (const response of [
      Response.json(
        {
          id: "share.workspace.publication.1",
          url: "https://openagents.test/share/share.workspace.publication.1",
          audienceLabel: "Shared with members of OpenAgents Core",
          status: "active",
        },
        { status: 201 },
      ),
      Response.json(
        {
          id: "share.workspace.publication.1",
          url: "https://openagents.test/share/share.workspace.publication.1",
          audienceLabel: "Shared with members of OpenAgents Core",
          status: "active",
        },
        { headers: { "Idempotency-Replayed": "false" }, status: 200 },
      ),
    ]) {
      const value = harness({ response });
      await expect(run(value.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason: "publication_outcome_unknown",
      });
      expect(value.calls).toHaveLength(2);
    }
  });

  test("rejects a mismatched or expanded audience response as ambiguous", async () => {
    for (const payload of [
      {
        id: "share.workspace.publication.1",
        url: "https://openagents.test/share/share.workspace.publication.1",
        audienceLabel: "Shared publicly",
        status: "active",
      },
      {
        id: "share.workspace.publication.1",
        url: "https://openagents.test/share/share.workspace.publication.1",
        audienceLabel: "Shared with members of OpenAgents Core",
        status: "active",
        raw: "content",
      },
    ]) {
      const value = harness({ response: Response.json(payload, { status: 201 }) });
      await expect(run(value.dependencies, request())).resolves.toEqual({
        status: "rejected",
        reason: "publication_outcome_unknown",
      });
    }
  });
});
