import { describe, expect, test } from "vitest";

import {
  DESKTOP_CODEX_USAGE_INGEST_PATH,
  DESKTOP_CODEX_USAGE_RESPONSE_SCHEMA,
  DESKTOP_CODEX_USAGE_SCHEMA,
  handleDesktopCodexUsageRequest,
} from "./desktop-codex-usage-routes";
import { makeD1TokenUsageLedger, type TokenUsageEventRow } from "./token-usage-ledger";
import { makeSqliteD1, TOKEN_LEDGER_D1_SCHEMA } from "./test/sqlite-d1";

const context = {} as ExecutionContext;
const nowIso = "2026-07-16T14:00:00.000Z";
const validBody = {
  schemaVersion: DESKTOP_CODEX_USAGE_SCHEMA,
  turnRef: "desktop.turn.0001",
  model: "gpt-5.2-codex",
  observedAt: "2026-07-16T13:59:00.000Z",
  usage: {
    inputTokens: 100,
    cachedInputTokens: 20,
    outputTokens: 30,
    reasoningTokens: 7,
    totalTokens: 137,
  },
};

const request = (body: unknown = validBody, authorization = "Bearer desktop-session"): Request =>
  new Request(`https://openagents.com${DESKTOP_CODEX_USAGE_INGEST_PATH}`, {
    body: JSON.stringify(body),
    headers: {
      authorization,
      "content-type": "application/json",
      "idempotency-key": validBody.turnRef,
    },
    method: "POST",
  });

const setup = () => {
  const sqlite = makeSqliteD1();
  sqlite.exec(TOKEN_LEDGER_D1_SCHEMA);
  const observed: Array<{
    idempotencyKey: string;
    tokensServed: number;
  }> = [];
  const ledger = makeD1TokenUsageLedger(
    sqlite.db,
    {
      isoTimestampAfterIso: (timestamp, milliseconds) =>
        new Date(Date.parse(timestamp) + milliseconds).toISOString(),
      nowIso: () => nowIso,
      utcStartOfDayIsoTimestamp: (timestamp) => `${timestamp.slice(0, 10)}T00:00:00.000Z`,
    },
    {
      onIngestedEvent: (event) => {
        observed.push({
          idempotencyKey: event.idempotencyKey,
          tokensServed: event.tokensServed,
        });
        return Promise.resolve();
      },
    },
  );

  const dependencies = (ownerUserId = "github:owner-1") => ({
    ledger: () => ledger,
    requireUserBearerSession: async (incoming: Request) =>
      incoming.headers.get("authorization") === "Bearer desktop-session"
        ? { user: { userId: ownerUserId } }
        : undefined,
    userIdFromSession: (session: Readonly<{ user: { userId: string } }>) => session.user.userId,
  });

  const rows = async (): Promise<Array<TokenUsageEventRow>> => {
    const result = await sqlite.db
      .prepare("SELECT * FROM token_usage_events ORDER BY id")
      .all<TokenUsageEventRow>();
    return result.results;
  };

  return { dependencies, observed, rows, sqlite };
};

describe("Desktop Codex exact usage ingest", () => {
  test("requires a verified user bearer before reading usage", async () => {
    const state = setup();
    const response = await handleDesktopCodexUsageRequest(
      state.dependencies(),
      request(validBody, ""),
      {},
      context,
    );

    expect(response.status).toBe(401);
    expect(await response.json()).toEqual({ error: "unauthorized" });
    expect(await state.rows()).toEqual([]);
    state.sqlite.close();
  });

  test("rejects malformed, zero, excess, and private payload material", async () => {
    const state = setup();
    const invalid = [
      { ...validBody, prompt: "private prompt" },
      { ...validBody, accountRef: "agent:caller-chosen" },
      { ...validBody, workspacePath: "/Users/private/repo" },
      { ...validBody, turnRef: "/Users/private/turn" },
      { ...validBody, model: "" },
      {
        ...validBody,
        usage: {
          inputTokens: 0,
          cachedInputTokens: 0,
          outputTokens: 0,
          reasoningTokens: 0,
        },
      },
      {
        ...validBody,
        usage: { ...validBody.usage, outputTokens: -1 },
      },
      {
        ...validBody,
        usage: { ...validBody.usage, totalTokens: 136 },
      },
    ];

    for (const body of invalid) {
      const response = await handleDesktopCodexUsageRequest(
        state.dependencies(),
        request(body),
        {},
        context,
      );
      expect(response.status).toBe(400);
      expect(await response.json()).toEqual({
        error: "invalid_desktop_codex_usage",
      });
    }
    expect(await state.rows()).toEqual([]);
    state.sqlite.close();
  });

  test("derives the owner and inserts the exact safe token split", async () => {
    const state = setup();
    const response = await handleDesktopCodexUsageRequest(
      state.dependencies(),
      request(),
      {},
      context,
    );

    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({
      schemaVersion: DESKTOP_CODEX_USAGE_RESPONSE_SCHEMA,
      insertedTokenUsage: true,
      tokensServedDelta: 137,
    });
    const [row] = await state.rows();
    expect(row).toMatchObject({
      account_ref: null,
      actor_user_id: "github:owner-1",
      cache_read_tokens: 20,
      demand_channel: "direct_local",
      demand_client: "openagents_desktop",
      demand_kind: "own_capacity",
      demand_source: "desktop_local_codex",
      input_tokens: 100,
      leaderboard_eligible: 0,
      model: "gpt-5.2-codex",
      output_tokens: 37,
      reasoning_tokens: 7,
      source_route: "unknown",
      total_tokens: 137,
      usage_truth: "exact",
    });
    expect(row?.safe_metadata_json).toBe('{"usageBasis":"desktop_codex_sdk_turn_completed"}');
    expect(JSON.stringify(row)).not.toContain("private");
    state.sqlite.close();
  });

  test("replays one owner turn once and advances the public observer once", async () => {
    const state = setup();
    const first = await handleDesktopCodexUsageRequest(
      state.dependencies(),
      request(),
      {},
      context,
    );
    const replay = await handleDesktopCodexUsageRequest(
      state.dependencies(),
      request(),
      {},
      context,
    );

    expect(await first.json()).toMatchObject({
      insertedTokenUsage: true,
      tokensServedDelta: 137,
    });
    expect(await replay.json()).toMatchObject({
      insertedTokenUsage: false,
      tokensServedDelta: 0,
    });
    expect(await state.rows()).toHaveLength(1);
    expect(state.observed).toEqual([
      {
        idempotencyKey: expect.stringMatching(/^desktop:codex:turn:/),
        tokensServed: 137,
      },
    ]);
    state.sqlite.close();
  });

  test("does not collide when two owners report the same turn ref", async () => {
    const state = setup();
    const ownerOne = await handleDesktopCodexUsageRequest(
      state.dependencies("github:owner-1"),
      request(),
      {},
      context,
    );
    const ownerTwo = await handleDesktopCodexUsageRequest(
      state.dependencies("github:owner-2"),
      request(),
      {},
      context,
    );

    expect(await ownerOne.json()).toMatchObject({ insertedTokenUsage: true });
    expect(await ownerTwo.json()).toMatchObject({ insertedTokenUsage: true });
    const rows = await state.rows();
    expect(rows).toHaveLength(2);
    expect(new Set(rows.map((row) => row.idempotency_key)).size).toBe(2);
    expect(rows.map((row) => row.actor_user_id).sort()).toEqual([
      "github:owner-1",
      "github:owner-2",
    ]);
    expect(state.observed).toHaveLength(2);
    state.sqlite.close();
  });
});
