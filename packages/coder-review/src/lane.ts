/**
 * Where a review's text comes from.
 *
 * Two lanes, and the difference between them is the whole point of the split.
 * The live lane sends the assembled prompt to a model and returns what the
 * model wrote. The replay lane returns a reviewer response that was recorded
 * earlier, verbatim, without reading the prompt at all.
 *
 * THE REPLAY LANE NEVER GENERATES. It is not an "offline reviewer" and it does
 * not have an opinion; it is a recording. That distinction is the reason this
 * command exists: `docs/coder/autoimprove.md` §6 names "confident review
 * without understanding" as a failure mode, and a stand-in that invents a
 * plausible score when the network is down is that failure mode wearing the
 * command's own uniform. So the replay lane's ref says `replay:` and every
 * artifact it produces carries that ref, where a reader will see it.
 *
 * The `--offline` flag on the CLI selects the replay lane. There is no third
 * behavior — no canned score, no degraded live call, no default review. A lane
 * that cannot answer fails.
 */

/** A source of one reviewer response. */
export interface ReviewerLane {
  /**
   * How this reviewer is named in the review file and in the candidate
   * lineage. It carries the lane kind first (`replay:`, `responses:`) so a
   * replayed review cannot be mistaken for a fresh one at a glance.
   */
  readonly ref: string;
  readonly ask: (prompt: string, signal?: AbortSignal) => Promise<string>;
}

export class ReviewerUnavailable extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ReviewerUnavailable";
  }
}

/**
 * Replay a recorded reviewer response.
 *
 * `read` is injected rather than reading the path directly so a test can prove
 * the lane never consults the prompt: the reader takes no argument, and there
 * is no path from `ask`'s input to its output.
 */
export const replayLane = (label: string, read: () => string): ReviewerLane => ({
  ref: `replay:${label}`,
  ask: (): Promise<string> => {
    const recorded = read();
    if (recorded.trim() === "") {
      throw new ReviewerUnavailable(
        `the recorded reviewer response at ${label} is empty. A replay lane returns what was recorded; it does not fill in a review.`,
      );
    }
    return Promise.resolve(recorded);
  },
});

export interface ResponsesLaneOptions {
  /** The API origin, such as `http://localhost:4000`. */
  readonly origin: string;
  /** The account bearer. The surface also answers without one. */
  readonly token?: string | undefined;
  /** The model to review with. Absent, the surface picks its default. */
  readonly model?: string | undefined;
  /** Injected for tests. Defaults to the global `fetch`. */
  readonly fetch?: typeof globalThis.fetch | undefined;
}

/**
 * Ask a model over `POST /api/v1/responses`, in one turn with no tools.
 *
 * A review is a single question about artifacts already in the prompt, so the
 * reviewer gets no tool runtime and no second round. Handing it tools would
 * let it read the repository it is reviewing, which is exactly the contamination
 * autoimprove §3 puts the review in a separate conversation to avoid.
 *
 * A different model from the one that ran the cycle is preferred where the
 * finding is load-bearing (§6, "reviewer sharing the worker's blind spots"),
 * and `--reviewer-model` is how that is expressed. This lane does not enforce
 * it: refusing to review with the same model would stop a cheap cycle for a
 * reason the reviewer can state better than a flag can.
 */
export const responsesLane = (options: ResponsesLaneOptions): ReviewerLane => {
  const call = options.fetch ?? globalThis.fetch;
  return {
    ref: `responses:${options.model ?? "default"}@${options.origin}`,
    ask: async (prompt, signal) => {
      let response: Response;
      try {
        response = await call(new URL("/api/v1/responses", options.origin), {
          method: "POST",
          headers: {
            ...(options.token === undefined ? {} : { authorization: `Bearer ${options.token}` }),
            "content-type": "application/json",
            accept: "text/event-stream, application/json",
          },
          body: JSON.stringify({
            input: [{ role: "user", content: prompt }],
            stream: true,
            ...(options.model === undefined ? {} : { model: options.model }),
          }),
          ...(signal === undefined ? {} : { signal }),
        });
      } catch (cause) {
        throw new ReviewerUnavailable(
          `the responses API at ${options.origin} could not be reached: ${String(cause)}`,
        );
      }

      if (!response.ok) {
        throw new ReviewerUnavailable(
          `the responses API at ${options.origin} answered HTTP ${String(response.status)}.`,
        );
      }
      if (response.body === null) {
        throw new ReviewerUnavailable(
          `the responses API at ${options.origin} answered with no body.`,
        );
      }

      let text = "";
      let failure: string | undefined;
      for await (const data of frames(response.body, signal)) {
        const event = parse(data);
        if (event === undefined) continue;
        if (event["type"] === "response.output_text.delta") {
          const delta = event["delta"];
          if (typeof delta === "string") text += delta;
          continue;
        }
        if (event["type"] === "response.failed") failure = failureOf(event);
      }

      if (failure !== undefined) {
        throw new ReviewerUnavailable(`the reviewer's request failed: ${failure}`);
      }
      if (text.trim() === "") {
        throw new ReviewerUnavailable(
          `the reviewer returned no text. An empty answer is not a review, and it is not a zero.`,
        );
      }
      return text;
    },
  };
};

/** Each SSE frame's `data:` payload, in order. */
async function* frames(
  body: ReadableStream<Uint8Array>,
  signal: AbortSignal | undefined,
): AsyncIterable<string> {
  const decoder = new TextDecoder();
  const reader = body.getReader();
  let buffer = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done || signal?.aborted === true) break;
      buffer += decoder.decode(value, { stream: true });
      for (;;) {
        const boundary = buffer.indexOf("\n\n");
        if (boundary < 0) break;
        const frame = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        for (const line of frame.split("\n")) {
          if (line.startsWith("data: ")) yield line.slice(6);
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

const parse = (data: string): Record<string, unknown> | undefined => {
  try {
    const value: unknown = JSON.parse(data);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : undefined;
  } catch {
    return undefined;
  }
};

const failureOf = (event: Record<string, unknown>): string => {
  const response = event["response"];
  if (response !== null && typeof response === "object") {
    const error = (response as Record<string, unknown>)["error"];
    if (error !== null && typeof error === "object") {
      const message = (error as Record<string, unknown>)["message"];
      if (typeof message === "string") return message;
    }
  }
  return "no reason was given";
};
