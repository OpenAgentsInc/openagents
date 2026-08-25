import { afterEach, describe, expect, it, vi } from "vitest";

import {
  assertResumable,
  fetchAllEvents,
  listThreads,
  parsePick,
  pickLast,
  replayEntries,
  replayWire,
  repositoryOf,
  resumableThreads,
  type ThreadEvent,
  type ThreadSummary,
} from "../src/coder-resume.js";
import { remintThread, ThreadUnavailable } from "../src/coder-thread.js";
import type { TranscriptSink } from "../src/coder-transcript.js";

const ORIGIN = "https://openagents.test";
const TOKEN = "oa_pat_account";
const THREAD_ID = "9bb19447-ecf4-4f1b-b44e-6b128664da9c";

const summary = (overrides: Partial<ThreadSummary> = {}): ThreadSummary => {
  const objective = overrides.objective ?? "openagents coder in openagents on main";
  const named = repositoryOf(objective);
  return {
    id: THREAD_ID,
    status: "open",
    objective,
    eventCount: 4,
    startedAt: "2026-08-24T12:00:00Z",
    repository: named?.repository,
    branch: named?.branch,
    ...overrides,
  };
};

/**
 * A recorded session, in the vocabulary the transcript writer posts: a first
 * turn that read a file, a steered second turn the reader interrupted. Every
 * replay assertion runs against this stream rather than one invented per test.
 */
const FIXTURE: ReadonlyArray<ThreadEvent> = [
  {
    id: 1,
    eventType: "turn.user",
    payload: { text: "standing context\n\n---\n\nwhat is in mix.exs?" },
    emittedAt: "2026-08-24T12:00:01Z",
  },
  {
    id: 2,
    eventType: "turn.reasoning",
    payload: { text: "I should read the file before answering." },
    emittedAt: "2026-08-24T12:00:02Z",
  },
  {
    id: 3,
    eventType: "tool.ran",
    payload: {
      call_id: "call-1",
      tool: "shell",
      arguments: `{"command":"cat mix.exs"}`,
      status: "succeeded",
      output: "defmodule OpenAgents.MixProject do",
    },
    emittedAt: "2026-08-24T12:00:03Z",
  },
  {
    id: 4,
    eventType: "turn.assistant",
    payload: {
      text: "It defines the OpenAgents application.",
      usage: { prompt_tokens: 120, completion_tokens: 30, total_tokens: 150, calls: 2 },
      tool_calls: 1,
    },
    emittedAt: "2026-08-24T12:00:04Z",
  },
  {
    id: 5,
    eventType: "turn.user",
    payload: { text: "also check the version", steered: true },
    emittedAt: "2026-08-24T12:00:05Z",
  },
  {
    id: 6,
    eventType: "tool.ran",
    payload: {
      call_id: "call-2",
      tool: "shell",
      arguments: `{"command":"cat VERSION"}`,
      status: "failed",
      error: "cat: VERSION: No such file or directory",
    },
    emittedAt: "2026-08-24T12:00:06Z",
  },
  {
    id: 7,
    eventType: "turn.assistant",
    payload: {
      text: "There is no VERSION file;",
      usage: { prompt_tokens: 200, completion_tokens: 8, total_tokens: 208, calls: 2 },
      tool_calls: 1,
      interrupted: true,
    },
    emittedAt: "2026-08-24T12:00:07Z",
  },
];

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("repositoryOf", () => {
  it("parses back the objective this CLI composes", () => {
    expect(repositoryOf("openagents coder in openagents.com on coder-resume")).toEqual({
      repository: "openagents.com",
      branch: "coder-resume",
    });
  });

  it("parses nothing from an objective some other caller wrote", () => {
    expect(repositoryOf("delegated children of openagents coder in repo")).toBeUndefined();
    expect(repositoryOf("nightly triage run")).toBeUndefined();
  });
});

describe("resumableThreads", () => {
  const here = summary({ id: "a" });
  const elsewhere = summary({ id: "b", objective: "openagents coder in probe on main" });
  const children = summary({
    id: "c",
    objective: "delegated children of openagents coder in openagents",
  });

  it("filters to the current repository", () => {
    expect(resumableThreads([here, elsewhere, children], "openagents", false)).toEqual([here]);
  });

  it("keeps every thread under --all, including ones without a repository", () => {
    expect(resumableThreads([here, elsewhere, children], "openagents", true)).toEqual([
      here,
      elsewhere,
      children,
    ]);
  });

  it("keeps terminal threads listed, so picking one gets the refusal that teaches", () => {
    const cancelled = summary({ id: "d", status: "cancelled" });
    expect(resumableThreads([cancelled], "openagents", false)).toEqual([cancelled]);
  });
});

describe("pickLast", () => {
  it("takes the newest, which the server lists first", () => {
    const newest = summary({ id: "newest" });
    const older = summary({ id: "older" });
    expect(pickLast([newest, older])).toBe(newest);
  });

  it("takes nothing from nothing", () => {
    expect(pickLast([])).toBeUndefined();
  });
});

describe("parsePick", () => {
  it("selects a number inside the range", () => {
    expect(parsePick("2", 3)).toBe(1);
  });

  it("cancels on empty, out of range, and non-numbers", () => {
    expect(parsePick("", 3)).toBeUndefined();
    expect(parsePick("0", 3)).toBeUndefined();
    expect(parsePick("4", 3)).toBeUndefined();
    expect(parsePick("-1", 3)).toBeUndefined();
    expect(parsePick("q", 3)).toBeUndefined();
  });
});

describe("assertResumable", () => {
  it("passes an open thread", () => {
    expect(() => assertResumable(summary())).not.toThrow();
  });

  it("refuses a terminal thread by its status", () => {
    for (const status of ["cancelled", "succeeded", "failed"]) {
      let refusal: unknown;
      try {
        assertResumable(summary({ status }));
      } catch (cause) {
        refusal = cause;
      }
      expect(refusal).toBeInstanceOf(ThreadUnavailable);
      expect((refusal as ThreadUnavailable).code).toBe("thread_terminal");
      expect((refusal as ThreadUnavailable).message).toContain(status);
    }
  });
});

describe("listThreads", () => {
  it("reads the account's listing with the account token", async () => {
    const calls: Array<{ url: string; authorization: string }> = [];
    const transport = async (url: URL, init?: RequestInit) => {
      const headers = (init?.headers ?? {}) as Record<string, string>;
      calls.push({ url: url.toString(), authorization: headers["authorization"] ?? "" });
      return new Response(
        JSON.stringify({
          threads: [
            {
              id: THREAD_ID,
              status: "open",
              objective: "openagents coder in openagents on main",
              event_count: 7,
              started_at: "2026-08-24T12:00:00Z",
            },
          ],
        }),
        { status: 200 },
      );
    };

    const threads = await listThreads({ origin: ORIGIN, token: TOKEN, fetch: transport });

    expect(calls[0]?.url).toBe(`${ORIGIN}/api/v1/threads?limit=50`);
    expect(calls[0]?.authorization).toBe(`Bearer ${TOKEN}`);
    expect(threads).toHaveLength(1);
    expect(threads[0]?.repository).toBe("openagents");
    expect(threads[0]?.eventCount).toBe(7);
  });

  it("prefers the thread's own repository field over the parsed objective", async () => {
    const transport = async () =>
      new Response(
        JSON.stringify({
          threads: [
            {
              id: THREAD_ID,
              status: "open",
              // The sentence names one repository, the field another. The
              // field wins: it is the thread's own record, written at open.
              objective: "openagents coder in renamed-checkout on main",
              repository: "OpenAgentsInc/openagents.com",
              event_count: 2,
              started_at: "2026-08-24T12:00:00Z",
            },
            {
              id: "older-thread-before-the-field",
              status: "open",
              // A thread opened before the server recorded the field still
              // resolves through the sentence this CLI composed.
              objective: "openagents coder in openagents on main",
              repository: null,
              event_count: 1,
              started_at: "2026-08-23T12:00:00Z",
            },
          ],
        }),
        { status: 200 },
      );

    const threads = await listThreads({ origin: ORIGIN, token: TOKEN, fetch: transport });

    expect(threads[0]?.repository).toBe("OpenAgentsInc/openagents.com");
    expect(threads[1]?.repository).toBe("openagents");

    // And the picker filter acts on the structured field, so the renamed
    // sentence does not hide the thread from its repository.
    expect(resumableThreads(threads, "OpenAgentsInc/openagents.com", false)).toEqual([threads[0]]);
  });
});

describe("fetchAllEvents", () => {
  it("pages through the cursor until a page comes back short", async () => {
    // 120 events: three pages at the server's cap of fifty.
    const all = Array.from({ length: 120 }, (_ignored, index) => ({
      id: index + 1,
      schema: "thread_event.v1",
      event_type: "turn.user",
      payload: { text: `turn ${String(index + 1)}` },
      emitted_at: "2026-08-24T12:00:00Z",
    }));

    const urls: string[] = [];
    const transport = async (url: URL) => {
      urls.push(url.toString());
      const after = Number(url.searchParams.get("after") ?? "0");
      const limit = Number(url.searchParams.get("limit") ?? "50");
      const page = all.filter((event) => event.id > after).slice(0, limit);
      return new Response(
        JSON.stringify({ thread_id: THREAD_ID, event_count: all.length, events: page }),
        { status: 200 },
      );
    };

    const events = await fetchAllEvents({
      origin: ORIGIN,
      token: TOKEN,
      threadId: THREAD_ID,
      fetch: transport,
    });

    expect(events).toHaveLength(120);
    expect(events.map((event) => event.id)).toEqual(all.map((event) => event.id));
    expect(urls).toEqual([
      `${ORIGIN}/api/v1/threads/${THREAD_ID}/events?limit=50`,
      `${ORIGIN}/api/v1/threads/${THREAD_ID}/events?limit=50&after=50`,
      `${ORIGIN}/api/v1/threads/${THREAD_ID}/events?limit=50&after=100`,
    ]);
  });

  it("stops on an empty transcript", async () => {
    const transport = async () =>
      new Response(JSON.stringify({ thread_id: THREAD_ID, event_count: 0, events: [] }), {
        status: 200,
      });
    const events = await fetchAllEvents({
      origin: ORIGIN,
      token: TOKEN,
      threadId: THREAD_ID,
      fetch: transport,
    });
    expect(events).toEqual([]);
  });
});

describe("replayEntries", () => {
  it("rebuilds the session transcript in recorded order, one settled entry per fact", () => {
    const entries = replayEntries(FIXTURE);

    expect(entries.map((entry) => entry.role)).toEqual([
      "you",
      "reasoning",
      "tool",
      "assistant",
      "you",
      "tool",
      "assistant",
    ]);
    expect(entries.every((entry) => entry.settled)).toBe(true);
    expect(entries[0]?.at).toBe(Date.parse("2026-08-24T12:00:01Z"));
  });

  it("carries the tool exchange whole, with its outcome", () => {
    const entries = replayEntries(FIXTURE);
    const succeeded = entries[2]?.tool;
    const failed = entries[5]?.tool;

    expect(succeeded).toMatchObject({
      callId: "call-1",
      name: "shell",
      arguments: `{"command":"cat mix.exs"}`,
      output: "defmodule OpenAgents.MixProject do",
      status: "succeeded",
    });
    expect(failed).toMatchObject({
      callId: "call-2",
      error: "cat: VERSION: No such file or directory",
      status: "failed",
    });
  });

  it("marks the interrupted answer and keeps the turn's cost", () => {
    const entries = replayEntries(FIXTURE);
    const finished = entries[3];
    const interrupted = entries[6];

    expect(finished?.metrics).toEqual({ promptTokens: 120, completionTokens: 30, calls: 2 });
    expect(interrupted?.text).toBe("There is no VERSION file;\n\n[interrupted]");
  });

  it("skips event types outside the vocabulary rather than refusing the replay", () => {
    const entries = replayEntries([
      { id: 1, eventType: "thread.noted", payload: { note: "?" }, emittedAt: undefined },
      ...FIXTURE,
    ]);
    expect(entries).toHaveLength(7);
  });
});

describe("replayWire", () => {
  it("rebuilds the messages in the shape the live loop holds", () => {
    expect(replayWire(FIXTURE, "default")).toEqual([
      { role: "user", content: "standing context\n\n---\n\nwhat is in mix.exs?" },
      {
        role: "assistant",
        content: "",
        tool_calls: [
          {
            id: "call-1",
            type: "function",
            function: { name: "shell", arguments: `{"command":"cat mix.exs"}` },
          },
        ],
      },
      { role: "tool", tool_call_id: "call-1", content: "defmodule OpenAgents.MixProject do" },
      { role: "assistant", content: "It defines the OpenAgents application." },
      { role: "user", content: "also check the version" },
      {
        role: "assistant",
        content: "",
        tool_calls: [
          {
            id: "call-2",
            type: "function",
            function: { name: "shell", arguments: `{"command":"cat VERSION"}` },
          },
        ],
      },
      { role: "tool", tool_call_id: "call-2", content: "cat: VERSION: No such file or directory" },
      { role: "assistant", content: "There is no VERSION file;" },
    ]);
  });

  it("keeps the recorded arguments as the raw JSON string", () => {
    const wire = replayWire(FIXTURE, "default");
    const call = wire[1];
    expect(call?.role === "assistant" && call.tool_calls?.[0]?.function.arguments).toBe(
      `{"command":"cat mix.exs"}`,
    );
  });

  it("keeps reasoning off the wire, as the live loop does", () => {
    const wire = replayWire(FIXTURE, "default");
    expect(wire.some((message) => message.content.includes("before answering"))).toBe(false);
  });

  it("budgets a stored tool result by the resumed session's model family", () => {
    const wire = replayWire(
      [
        {
          id: 1,
          eventType: "tool.ran",
          payload: {
            call_id: "call-1",
            tool: "shell",
            arguments: "{}",
            status: "succeeded",
            output: "x".repeat(10_000),
          },
          emittedAt: undefined,
        },
      ],
      "gemini",
    );
    const result = wire[1];
    expect(result?.role).toBe("tool");
    expect(result?.content).toContain("characters omitted from the middle");
    expect(result?.content).toContain("for the gemini model family");
    // The same event replayed for a hosted lane keeps more of it, because the
    // budget is the family's and not the record's.
    const hosted = replayWire(
      [
        {
          id: 1,
          eventType: "tool.ran",
          payload: {
            call_id: "call-1",
            tool: "shell",
            arguments: "{}",
            status: "succeeded",
            output: "x".repeat(10_000),
          },
          emittedAt: undefined,
        },
      ],
      "default",
    );
    expect((hosted[1]?.content.length ?? 0) > (result?.content.length ?? 0)).toBe(true);
  });
});

const sse = (frames: ReadonlyArray<string>) =>
  new Response([...frames.map((frame) => `data: ${frame}`), ""].join("\n\n"), {
    status: 200,
    headers: { "content-type": "text/event-stream" },
  });

describe("remintThread", () => {
  // The same `minted_view` shape `POST /api/v1/threads` returns for a grant at
  // minting: token, url, model, expires_at, limits — and no `remaining`,
  // because a freshly minted grant has spent nothing.
  const REMINTED = {
    thread: { id: THREAD_ID, status: "open", generation: 2 },
    grant: {
      token: "oa_grant_reminted",
      url: `${ORIGIN}/api/inference/proxy`,
      model: "gpt-5.6-luna",
      expires_at: "2026-08-24T23:59:59Z",
      limits: { max_calls: 256, max_total_tokens: 1_000_000, max_cost_microusd: 2_000_000 },
    },
  };

  interface Call {
    readonly method: string;
    readonly url: string;
    readonly body: Record<string, unknown>;
  }

  const stub = (mint: Response) => {
    const calls: Call[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (target: URL | string, init?: RequestInit) => {
        const url = typeof target === "string" ? target : target.toString();
        const raw = typeof init?.body === "string" ? init.body : "{}";
        calls.push({
          method: init?.method ?? "GET",
          url,
          body: JSON.parse(raw) as Record<string, unknown>,
        });
        if (url.endsWith("/api/inference/proxy")) {
          return sse([
            `{"choices":[{"delta":{"content":"Continuing."},"index":0}]}`,
            `{"choices":[],"usage":{"completion_tokens":3,"prompt_tokens":50,"total_tokens":53}}`,
            "[DONE]",
          ]);
        }
        if (url.endsWith("/grants")) return mint.clone();
        return new Response(JSON.stringify({}), { status: 200 });
      }),
    );
    return calls;
  };

  it("reports a server that cannot re-grant an existing thread as exactly that", async () => {
    stub(new Response(JSON.stringify({ errors: {} }), { status: 404 }));

    await expect(
      remintThread({ origin: ORIGIN, token: TOKEN, threadId: THREAD_ID }),
    ).rejects.toMatchObject({
      name: "ThreadUnavailable",
      code: "grant_unavailable",
      message: expect.stringContaining("cannot hand back authority for an existing thread"),
    });
  });

  it("continues the same thread on the re-minted grant", async () => {
    const calls = stub(new Response(JSON.stringify(REMINTED), { status: 201 }));

    const source = await remintThread({ origin: ORIGIN, token: TOKEN, threadId: THREAD_ID });

    expect(calls[0]?.method).toBe("POST");
    expect(calls[0]?.url).toBe(`${ORIGIN}/api/v1/threads/${THREAD_ID}/grants`);
    expect(source.threadId).toBe(THREAD_ID);
    expect(source.model).toBe("gpt-5.6-luna");
    // A fresh grant has spent nothing, so the budget opens at its ceilings.
    expect(source.budget).toContain("256 calls");
  });

  it("answers the next turn against the replayed transcript without re-posting any of it", async () => {
    const calls = stub(new Response(JSON.stringify(REMINTED), { status: 201 }));
    const recorded: Array<{ eventType: string; payload: Record<string, unknown> }> = [];
    const sink: TranscriptSink = {
      record: (eventType, payload) => {
        recorded.push({ eventType, payload });
      },
    };

    const source = await remintThread({ origin: ORIGIN, token: TOKEN, threadId: THREAD_ID });
    const replayed = replayWire(FIXTURE, "default");
    source.preload(replayed);
    source.useTranscript(sink);

    // Replaying wrote nothing: the server already holds these events.
    expect(recorded).toEqual([]);

    for await (const chunk of source.reply("carry on", new AbortController().signal)) {
      void chunk;
    }

    // The model was answered against the whole replayed history plus the new
    // prompt, in order.
    const turn = calls.find((call) => call.url.endsWith("/api/inference/proxy"));
    const sent = turn?.body["messages"] as Array<Record<string, unknown>>;
    expect(sent.filter((message) => message["role"] !== "system")).toEqual([
      ...replayed,
      { role: "user", content: "carry on" },
    ]);

    // Only the new turn reached the transcript writer.
    expect(recorded.map((event) => event.eventType)).toEqual(["turn.user", "turn.assistant"]);
    expect(recorded[0]?.payload["text"]).toBe("carry on");
  });
});
