import { describe, expect, it, vi } from "vitest";

import { ThreadTranscriptWriter, type TranscriptTransport } from "../src/coder-transcript.js";

const ORIGIN = "https://openagents.test";
const TOKEN = "oa_pat_account";
const THREAD_ID = "9bb19447-ecf4-4f1b-b44e-6b128664da9c";

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

const posted = () => json(201, { thread: { id: THREAD_ID, event_count: 1 } });

interface Recorded {
  readonly url: string;
  readonly method: string;
  readonly authorization: string;
  readonly body: Record<string, unknown>;
}

/**
 * A transport that answers from a script, one response or failure per call,
 * and records what was sent. The last entry repeats, so a drain after the
 * interesting part needs no padding.
 */
const transport = (script: ReadonlyArray<Response | Error>) => {
  const calls: Recorded[] = [];
  let at = 0;

  const fetch = vi.fn(async (target: URL | string, init?: RequestInit) => {
    const headers = (init?.headers ?? {}) as Record<string, string>;
    calls.push({
      url: typeof target === "string" ? target : target.toString(),
      method: init?.method ?? "GET",
      authorization: headers["authorization"] ?? "",
      body: JSON.parse(typeof init?.body === "string" ? init.body : "{}") as Record<
        string,
        unknown
      >,
    });
    const answer = script[Math.min(at, script.length - 1)];
    at += 1;
    if (answer instanceof Error) throw answer;
    return (answer ?? posted()).clone();
  });

  return { calls, fetch };
};

const writer = (
  wire: { fetch: TranscriptTransport },
  extra?: Partial<ConstructorParameters<typeof ThreadTranscriptWriter>[0]>,
) =>
  new ThreadTranscriptWriter({
    origin: ORIGIN,
    threadId: THREAD_ID,
    token: TOKEN,
    fetch: wire.fetch,
    retryDelaysMs: [0, 0, 0],
    ...extra,
  });

describe("ThreadTranscriptWriter", () => {
  it("posts each event to the thread's transcript on the account token", async () => {
    const wire = transport([posted()]);
    const sink = writer(wire);

    sink.record("turn.user", { text: "list the open issues" });
    await sink.close();

    expect(wire.calls).toHaveLength(1);
    const call = wire.calls[0];
    expect(call?.method).toBe("POST");
    expect(call?.url).toBe(`${ORIGIN}/api/v1/threads/${THREAD_ID}/events`);
    expect(call?.authorization).toBe(`Bearer ${TOKEN}`);
    expect(call?.body).toEqual({
      event_type: "turn.user",
      payload: { text: "list the open issues" },
    });
  });

  it("posts events in the order they were recorded", async () => {
    const wire = transport([posted()]);
    const sink = writer(wire);

    sink.record("turn.user", { text: "hello" });
    sink.record("tool.ran", { tool: "shell", status: "succeeded" });
    sink.record("turn.assistant", { text: "done" });
    await sink.close();

    expect(wire.calls.map((call) => call.body["event_type"])).toEqual([
      "turn.user",
      "tool.ran",
      "turn.assistant",
    ]);
  });

  it("retries a refused connection and keeps the order across the retry", async () => {
    const wire = transport([new Error("socket closed"), posted()]);
    const sink = writer(wire);

    sink.record("turn.user", { text: "first" });
    sink.record("turn.assistant", { text: "second" });
    await sink.close();

    // The failed event is retried, not skipped: posting the next one first
    // would reorder the transcript.
    expect(wire.calls.map((call) => call.body["event_type"])).toEqual([
      "turn.user",
      "turn.user",
      "turn.assistant",
    ]);
    expect(sink.pending).toBe(0);
  });

  it("retries a server failure the same way", async () => {
    const wire = transport([json(502, {}), posted()]);
    const sink = writer(wire);

    sink.record("turn.user", { text: "hello" });
    await sink.close();

    expect(wire.calls).toHaveLength(2);
    expect(sink.pending).toBe(0);
  });

  it("never throws into the caller, whatever the transport does", async () => {
    const sink = writer({
      fetch: (() => {
        throw new Error("broken before the promise");
      }) as unknown as TranscriptTransport,
    });

    expect(() => {
      sink.record("turn.user", { text: "hello" });
    }).not.toThrow();

    // Stop the retry loop rather than leaving it running under later tests.
    await sink.close(20);
  });

  it("says so once when posting keeps failing, and keeps queueing", async () => {
    const notices: string[] = [];
    const wire = transport([new Error("down")]);
    const sink = writer(wire, {
      // A real rung after the notice, so the retry loop idles rather than
      // spins while the close deadline runs down.
      retryDelaysMs: [0, 0, 10],
      onTrouble: (message) => {
        notices.push(message);
      },
    });

    sink.record("turn.user", { text: "one" });
    sink.record("turn.user", { text: "two" });
    await sink.close(200);

    // Three consecutive failures are an outage worth one sentence; the fourth
    // and fifth are the same outage and get no second sentence.
    expect(notices).toHaveLength(1);
    expect(notices[0]).toContain("not reaching the server");
    // Nothing was dropped: both events are still queued for retry.
    expect(sink.pending).toBe(2);
  });

  it("drops an event the server called invalid and continues with the next", async () => {
    const notices: string[] = [];
    const wire = transport([json(422, { errors: { event_type: ["cannot be blank"] } }), posted()]);
    const sink = writer(wire, {
      onTrouble: (message) => {
        notices.push(message);
      },
    });

    sink.record("turn.user", { text: "refused" });
    sink.record("turn.assistant", { text: "still recorded" });
    await sink.close();

    // A payload the server has already called invalid will be invalid
    // tomorrow; retrying it would silently stop the whole record.
    expect(wire.calls.map((call) => call.body["event_type"])).toEqual([
      "turn.user",
      "turn.assistant",
    ]);
    expect(notices).toHaveLength(1);
    expect(sink.pending).toBe(0);
  });

  it("stops for good when the thread is terminal", async () => {
    const notices: string[] = [];
    const wire = transport([json(422, { code: "thread_terminal" })]);
    const sink = writer(wire, {
      onTrouble: (message) => {
        notices.push(message);
      },
    });

    sink.record("turn.user", { text: "one" });
    sink.record("turn.user", { text: "two" });
    await sink.close();
    sink.record("turn.user", { text: "three" });
    await sink.close();

    // A closed transcript can never take another event, so nothing is queued
    // against a refusal that cannot change.
    expect(wire.calls).toHaveLength(1);
    expect(notices).toHaveLength(1);
    expect(sink.pending).toBe(0);
  });

  it("gives up the flush at the deadline rather than holding the exit", async () => {
    const sink = writer({
      // A transport that never answers, which is what a dead server looks
      // like from a process that is trying to leave.
      fetch: (() => new Promise<Response>(() => undefined)) as TranscriptTransport,
    });

    sink.record("turn.user", { text: "unsendable" });

    const started = Date.now();
    await sink.close(50);
    expect(Date.now() - started).toBeLessThan(1_000);
  });
});
