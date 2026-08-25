import { afterEach, describe, expect, it, vi } from "vitest";

import {
  openLocalThread,
  threadAnnouncement,
  threadSyncWanted,
} from "../src/coder-local-thread.js";

const ORIGIN = "https://openagents.test";
const TOKEN = "oa_pat_account";
const THREAD_ID = "9bb19447-ecf4-4f1b-b44e-6b128664da9c";

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("whether the local lane reports at all", () => {
  it("is on by default", () => {
    expect(threadSyncWanted({})).toBe(true);
  });

  it("is off when OPENAGENTS_THREAD_SYNC says off", () => {
    expect(threadSyncWanted({ OPENAGENTS_THREAD_SYNC: "off" })).toBe(false);
    expect(threadSyncWanted({ OPENAGENTS_THREAD_SYNC: "OFF" })).toBe(false);
  });

  it("stays on for any other value, so only the documented word disables it", () => {
    expect(threadSyncWanted({ OPENAGENTS_THREAD_SYNC: "on" })).toBe(true);
    expect(threadSyncWanted({ OPENAGENTS_THREAD_SYNC: "" })).toBe(true);
  });
});

describe("opening a transcript-only thread", () => {
  it("posts the local lane and the vendor model, and takes the thread without a grant", async () => {
    const fetchMock = vi.fn(async (input: URL | RequestInfo, init?: RequestInit) => {
      expect(String(input)).toBe(`${ORIGIN}/api/v1/threads`);
      expect(init?.method).toBe("POST");
      const headers = init?.headers as Record<string, string>;
      expect(headers["authorization"]).toBe(`Bearer ${TOKEN}`);
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
      expect(body).toEqual({
        objective: "openagents coder in octavia/project on main",
        repository: "octavia/project",
        model: "ollama:qwen3.8:27b-mtp-q8_0",
        lane: "local",
      });
      // The server admits the record and mints no authority: a thread, no
      // grant. The client must neither expect nor use one.
      return json(201, { thread: { id: THREAD_ID, status: "open" } });
    });
    vi.stubGlobal("fetch", fetchMock);

    const opened = await openLocalThread({
      origin: ORIGIN,
      token: TOKEN,
      objective: "openagents coder in octavia/project on main",
      repository: "octavia/project",
      model: "ollama:qwen3.8:27b-mtp-q8_0",
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(opened).toEqual({ threadId: THREAD_ID });
  });

  it("carries the reasoning level when the session named one", async () => {
    const fetchMock = vi.fn(async (_input: URL | RequestInfo, init?: RequestInit) => {
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
      expect(body["reasoning"]).toBe("high");
      return json(201, { thread: { id: THREAD_ID } });
    });
    vi.stubGlobal("fetch", fetchMock);

    await openLocalThread({
      origin: ORIGIN,
      token: TOKEN,
      objective: "work",
      model: "ollama:qwen3.8",
      reasoning: "high",
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("degrades silently when the server cannot be reached", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("fetch failed");
      }),
    );

    await expect(
      openLocalThread({ origin: ORIGIN, token: TOKEN, objective: "work", model: "ollama:q" }),
    ).resolves.toBeUndefined();
  });

  it("degrades silently when the server refuses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => json(422, { code: "lane_unknown", message: "no such lane" })),
    );

    await expect(
      openLocalThread({ origin: ORIGIN, token: TOKEN, objective: "work", model: "ollama:q" }),
    ).resolves.toBeUndefined();
  });

  it("degrades silently when the response carries no thread id", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => json(201, {})),
    );

    await expect(
      openLocalThread({ origin: ORIGIN, token: TOKEN, objective: "work", model: "ollama:q" }),
    ).resolves.toBeUndefined();
  });
});

describe("the thread announcement", () => {
  it("is exactly the line the Gym adapter parses", () => {
    const line = threadAnnouncement(THREAD_ID);
    expect(line).toBe(`[oa:thread ${THREAD_ID}]`);
    // The adapter's regex, verbatim (OpenAgentsInc/openagents#38). The format
    // is a contract: a drifted line unlinks every trial from its thread.
    expect(/\[oa:thread ([0-9a-fA-F-]{36})\]/.exec(line)?.[1]).toBe(THREAD_ID);
  });
});
