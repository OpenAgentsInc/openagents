import { afterEach, describe, expect, it, vi } from "vitest";

import { OxAlphaReplySource, OxAlphaUnavailable } from "../src/coder-ox.js";

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

const collect = async (source: OxAlphaReplySource, prompt = "hello") => {
  const chunks: string[] = [];
  for await (const chunk of source.reply(prompt, new AbortController().signal)) chunks.push(chunk);
  return chunks.join("");
};

const source = () =>
  new OxAlphaReplySource({ origin: "https://openagents.test", token: "test-token" });

afterEach(() => {
  vi.unstubAllGlobals();
});

/** Answer `GET /chat/events` from a script and `POST /chat/turns` with a run id. */
const stubFetch = (pages: ReadonlyArray<ReadonlyArray<unknown>>, submit = json(202, {})) => {
  let page = 0;
  const calls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((url: URL, init?: RequestInit) => {
      const path = url.pathname;
      calls.push(`${init?.method ?? "GET"} ${path}`);
      if (path.endsWith("/chat/turns")) return Promise.resolve(submit.clone());
      const events = pages[Math.min(page, pages.length - 1)] ?? [];
      page += 1;
      return Promise.resolve(json(200, { events }));
    }),
  );
  return calls;
};

describe("OxAlphaReplySource", () => {
  it("yields text deltas for the submitted run and stops at completed", async () => {
    stubFetch(
      [
        [],
        [
          { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "Hello" } },
          { run_id: "run-1", sequence: 2, type: "text_delta", payload: { value: " there" } },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("Hello there");
  });

  it("does not replay events that existed before the turn was submitted", async () => {
    stubFetch(
      [
        [{ run_id: "run-0", sequence: 9, type: "text_delta", payload: { value: "OLD" } }],
        [
          { run_id: "run-0", sequence: 9, type: "text_delta", payload: { value: "OLD" } },
          { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "new" } },
          { run_id: "run-1", sequence: 2, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("new");
  });

  it("does not repeat a delta already delivered on an earlier poll", async () => {
    stubFetch(
      [
        [],
        [{ run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "one" } }],
        [
          { run_id: "run-1", sequence: 1, type: "text_delta", payload: { value: "one" } },
          { run_id: "run-1", sequence: 2, type: "text_delta", payload: { value: " two" } },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("one two");
  });

  it("ignores reasoning deltas, which the transcript does not show", async () => {
    stubFetch(
      [
        [],
        [
          { run_id: "run-1", sequence: 1, type: "reasoning_delta", payload: { value: "thinking" } },
          { run_id: "run-1", sequence: 2, type: "text_delta", payload: { value: "said" } },
          { run_id: "run-1", sequence: 3, type: "response_completed", payload: {} },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );

    expect(await collect(source())).toBe("said");
  });

  it("reports a missing scope rather than an empty reply", async () => {
    stubFetch([[]], json(401, { error: "invalid_api_token" }));
    await expect(collect(source())).rejects.toThrow(OxAlphaUnavailable);
    await expect(collect(source())).rejects.toThrow(/chat:account/);
  });

  it("names the one-turn-at-a-time rule when the server refuses a concurrent turn", async () => {
    stubFetch([[]], json(409, { error: "turn_in_progress" }));
    await expect(collect(source())).rejects.toThrow(/one turn runs at a time/i);
  });

  it("surfaces a failed turn with the server's reason", async () => {
    stubFetch(
      [
        [],
        [
          {
            run_id: "run-1",
            sequence: 1,
            type: "response_failed",
            payload: { reason: "provider refused", code: "invalid_response" },
          },
        ],
      ],
      json(202, { turn: { id: "run-1" } }),
    );
    await expect(collect(source())).rejects.toThrow(/provider refused/);
  });

  it("sends the prompt and the reasoning effort the caller chose", async () => {
    const seen: Array<Record<string, unknown>> = [];
    let submitted = false;
    vi.stubGlobal(
      "fetch",
      vi.fn((url: URL, init?: RequestInit) => {
        if (url.pathname.endsWith("/chat/turns")) {
          seen.push(JSON.parse(String(init?.body)) as Record<string, unknown>);
          submitted = true;
          return Promise.resolve(json(202, { turn: { id: "run-1" } }));
        }
        // The run does not exist until it is submitted, which is what the
        // pre-submit snapshot relies on.
        return Promise.resolve(
          json(200, {
            events: submitted
              ? [{ run_id: "run-1", sequence: 1, type: "response_completed", payload: {} }]
              : [],
          }),
        );
      }),
    );

    const configured = new OxAlphaReplySource({
      origin: "https://openagents.test",
      token: "test-token",
      reasoning: "high",
    });
    await collect(configured, "do the thing");

    expect(seen[0]).toEqual({ message: "do the thing", reasoning: "high" });
  });

  it("reports the model it runs on", () => {
    expect(source().model).toBe("stealth/ox-alpha");
  });
});
