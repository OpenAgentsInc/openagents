/**
 * The local lane's transcript-only thread.
 *
 * A session on `--model ollama:<name>` answers entirely from the local Ollama
 * server, so the server is not in the turn loop at all — but the server is
 * where everything rehydrates from: resume, export, `/threads`, the Gym's live
 * run transcripts. A local session that reported nothing was invisible to all
 * of it. So when the session holds an api-url and a token, it opens a thread
 * with `"lane": "local"` — the same `POST /api/v1/threads` body, plus the lane
 * and the vendor model string — and streams its transcript there through the
 * same `ThreadTranscriptWriter` the thread lane uses. The response carries no
 * grant and none is asked for: inference stays on the local client, only the
 * record travels.
 *
 * Reporting is the default and must never cost the session anything: no token,
 * no api-url, or an unreachable server degrades silently to local-only, which
 * is why `openLocalThread` resolves `undefined` for every failure rather than
 * ever throwing. `OPENAGENTS_THREAD_SYNC=off` and `--offline` are the two ways
 * to say no on purpose.
 */

import { THREADS_PATH } from "./constants.js";

export interface LocalThreadOptions {
  readonly origin: string;
  /** The account token. Opens the thread and posts its events; no grant is minted. */
  readonly token: string;
  /** What this body of work is for. The server requires one. */
  readonly objective: string;
  /** The repository the work concerns, as `owner/name`. */
  readonly repository?: string | undefined;
  /** Recorded on the thread as its admitted execution shape. */
  readonly reasoning?: string | undefined;
  /**
   * The vendor model string, for example `ollama:qwen3.8:27b-mtp-q8_0`.
   *
   * Recorded on the thread so a reader of `/threads/:id` knows what answered.
   * Unlike the thread lane's model this pins no grant — there is nothing to
   * spend — it is the record's name for the model that ran locally.
   */
  readonly model: string;
}

/** What the local lane gets back: a thread to write to, and nothing to spend. */
export interface LocalThread {
  readonly threadId: string;
}

/**
 * Whether the session should report its transcript to the server at all.
 *
 * On unless someone said off. The one switch is `OPENAGENTS_THREAD_SYNC=off`;
 * everything else that ends with no sync — no token, no reachable server — is
 * degradation, not configuration, and is handled where it happens.
 */
export const threadSyncWanted = (env: Record<string, string | undefined>): boolean =>
  env["OPENAGENTS_THREAD_SYNC"]?.trim().toLowerCase() !== "off";

/**
 * The machine-readable thread announcement for `--plain` output.
 *
 * Exactly this shape and no other: the Gym adapter links a trial to its
 * thread by parsing captured coder output with
 * `\[oa:thread ([0-9a-fA-F-]{36})\]` (OpenAgentsInc/openagents#38), so the
 * format is a contract, not a log line.
 */
export const threadAnnouncement = (threadId: string): string => `[oa:thread ${threadId}]`;

/**
 * Open a transcript-only thread for a local-model session.
 *
 * `POST /api/v1/threads` with `"lane": "local"`. The response is a thread
 * without a grant — the server admits the record and mints no authority, and
 * this client neither expects nor uses one.
 *
 * Resolves `undefined` for every failure — the network refusing, a server
 * without the lane, a malformed body — because the local lane must never be
 * slower or louder for a server that is not there. The session runs exactly as
 * it would offline; only the record is lost, and only for this session.
 */
export const openLocalThread = async (
  options: LocalThreadOptions,
): Promise<LocalThread | undefined> => {
  try {
    const response = await fetch(new URL(THREADS_PATH, options.origin), {
      method: "POST",
      headers: {
        authorization: `Bearer ${options.token}`,
        "content-type": "application/json",
        accept: "application/json",
      },
      body: JSON.stringify({
        objective: options.objective,
        ...(options.repository === undefined ? {} : { repository: options.repository }),
        ...(options.reasoning === undefined ? {} : { reasoning: options.reasoning }),
        model: options.model,
        lane: "local",
      }),
    });

    if (response.status < 200 || response.status >= 300) return undefined;

    const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    const thread = body["thread"];
    const id =
      typeof thread === "object" && thread !== null && !Array.isArray(thread)
        ? (thread as Record<string, unknown>)["id"]
        : undefined;

    return typeof id === "string" && id.length > 0 ? { threadId: id } : undefined;
  } catch {
    return undefined;
  }
};
