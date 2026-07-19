import type { SyncSql } from "@openagentsinc/khala-sync-server";
import { Effect } from "effect";
import { describe, expect, test } from "vitest";

import {
  SARAH_SPEECH_MODEL,
  SARAH_SPEECH_PATH,
  SARAH_SPEECH_SCHEMA,
  SARAH_SPEECH_VOICE,
  makeSarahSpeechRoutes,
} from "./sarah-speech-routes";

type Env = Readonly<{
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }>;
  OPENAI_API_KEY?: string;
}>;

const body = {
  schema: SARAH_SPEECH_SCHEMA,
  threadRef: "thread.sarah.0123456789abcdef01234567",
  messageRef: "event.sarah.reply.1",
  text: "Hello from Sarah.",
} as const;

const makeHarness = (input: Readonly<{
  owner?: string;
  authority?: boolean;
  apiKey?: string;
  fetch?: typeof globalThis.fetch;
}> = {}) => {
  let ended = 0;
  const routes = makeSarahSpeechRoutes<Env>({
    authenticateOwner: async () => input.owner === undefined
      ? undefined
      : { userId: input.owner },
    makeSqlClient: async () => ({
      sql: (async () => []) as unknown as SyncSql,
      end: async () => { ended += 1; },
    }),
    hasAuthority: async () => input.authority ?? true,
    fetch: input.fetch,
  });
  const run = (requestBody: unknown = body, method = "POST") =>
    Effect.runPromise(routes.handle(
      new Request(`https://openagents.com${SARAH_SPEECH_PATH}`, {
        method,
        headers: { "content-type": "application/json" },
        ...(method === "POST" ? { body: JSON.stringify(requestBody) } : {}),
      }),
      {
        KHALA_SYNC_DB: { connectionString: "postgresql://fixture/private" },
        ...(input.apiKey === undefined ? {} : { OPENAI_API_KEY: input.apiKey }),
      },
      {} as ExecutionContext,
    ));
  return { run, ended: () => ended };
};

describe("Sarah owner-private OpenAI speech", () => {
  test("requires the authenticated owner and admitted Sarah thread", async () => {
    expect((await makeHarness().run()).status).toBe(401);
    expect((await makeHarness({ owner: "owner", authority: false }).run()).status).toBe(403);
  });

  test("fails closed when the server secret is absent", async () => {
    const harness = makeHarness({ owner: "owner" });
    const response = await harness.run();
    expect(response.status).toBe(503);
    expect(await response.json()).toEqual({ error: "sarah_speech_unavailable" });
    expect(harness.ended()).toBe(1);
  });

  test("sends the bounded reply to OpenAI and returns no-store AI-disclosed MP3", async () => {
    let authorization = "";
    let requestBody: Record<string, unknown> = {};
    const harness = makeHarness({
      owner: "owner",
      apiKey: "secret-fixture",
      fetch: async (_url, init) => {
        authorization = new Headers(init?.headers).get("authorization") ?? "";
        requestBody = JSON.parse(String(init?.body)) as Record<string, unknown>;
        return new Response(new Uint8Array([73, 68, 51]), {
          status: 200,
          headers: { "content-type": "audio/mpeg" },
        });
      },
    });
    const response = await harness.run();
    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe("audio/mpeg");
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("x-openagents-ai-voice")).toBe("true");
    expect(authorization).toBe("Bearer secret-fixture");
    expect(requestBody).toMatchObject({
      model: SARAH_SPEECH_MODEL,
      voice: SARAH_SPEECH_VOICE,
      input: body.text,
      response_format: "mp3",
    });
    expect(requestBody).not.toHaveProperty("threadRef");
    expect(harness.ended()).toBe(1);
  });

  test("redacts provider failures and rejects excess request fields", async () => {
    const providerFailure = makeHarness({
      owner: "owner",
      apiKey: "secret-fixture",
      fetch: async () => new Response("provider-secret-detail", { status: 500 }),
    });
    const failed = await providerFailure.run();
    expect(failed.status).toBe(502);
    expect(JSON.stringify(await failed.json())).not.toContain("provider-secret-detail");

    const invalid = await makeHarness({ owner: "owner", apiKey: "secret-fixture" })
      .run({ ...body, unexpected: true });
    expect(invalid.status).toBe(400);
  });
});
