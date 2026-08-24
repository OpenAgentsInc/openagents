/**
 * The thread's durable transcript, written as the turn loop runs.
 *
 * `POST /api/v3/threads/{id}/events` is append-only and the server's copy is
 * the only copy: this process keeps no file of its own, so what lands here is
 * what `--resume`, the export, and every other machine reading the thread will
 * ever see. The vocabulary is the one decided in the openagents.com audit of
 * 2026-08-24 — `turn.user`, `turn.reasoning`, `tool.ran`, `turn.assistant` —
 * and deltas and interface notices are deliberately not recorded: a delta is
 * how a reply arrived rather than what it is, and a notice never reached a
 * model.
 *
 * The writer must never cost the session anything. A turn loop that blocked on
 * a slow POST would make every tool call wait on the network twice, and a turn
 * that died because the transcript endpoint was down would have traded the work
 * for the record of it. So `record` is synchronous enqueue, one pump posts the
 * queue in order in the background, a failed post is retried with backoff, and
 * a persistent failure surfaces one notice and keeps queueing rather than ever
 * throwing into the loop that called it.
 */

const THREADS_PATH = "/api/v3/threads";

/**
 * How many consecutive failed posts before the reader is told once.
 *
 * One flaky request is the network being the network. Three in a row is an
 * outage the reader should know about, because from that point the durable
 * record is running behind the session it records.
 */
const TROUBLE_AFTER = 3;

/** Backoff between retries of one event, capped where waiting longer buys nothing. */
const RETRY_DELAYS_MS: ReadonlyArray<number> = [500, 1_000, 2_000, 5_000, 10_000, 30_000];

/**
 * How long `close` waits for the queue to drain before giving up.
 *
 * Flushing runs between the last turn and the revoke that closes the thread,
 * and a server that is down at exit must not hold the terminal open forever: a
 * reader who typed `exit` has left. What cannot be posted by this deadline is
 * lost, and that is the one place loss is accepted.
 */
const CLOSE_DEADLINE_MS = 5_000;

/** What one event needs from the caller. The writer supplies the rest. */
interface QueuedEvent {
  readonly eventType: string;
  readonly payload: Record<string, unknown>;
}

/**
 * The one call shape the writer makes of its transport. Named so a test can
 * hand in a plain function without matching `fetch`'s full overload set.
 */
export type TranscriptTransport = (input: URL, init?: RequestInit) => Promise<Response>;

/**
 * The slice of the writer a reply source calls. Narrow on purpose so a test
 * can stand a recorder in for the whole machinery.
 */
export interface TranscriptSink {
  record(eventType: string, payload: Record<string, unknown>): void;
}

export interface TranscriptWriterOptions {
  readonly origin: string;
  readonly threadId: string;
  /** The account token, the same authority that opened the thread. */
  readonly token: string;
  /** Where one sentence about persistent failure goes. Usually the status line. */
  readonly onTrouble?: ((message: string) => void) | undefined;
  /** Injection seam for tests. Defaults to the global `fetch`. */
  readonly fetch?: TranscriptTransport | undefined;
  /** Injection seam for tests. Defaults to the production backoff ladder. */
  readonly retryDelaysMs?: ReadonlyArray<number> | undefined;
}

export class ThreadTranscriptWriter implements TranscriptSink {
  private readonly queue: QueuedEvent[] = [];
  private readonly delays: ReadonlyArray<number>;
  private readonly post: TranscriptTransport;
  private pumping = false;
  private consecutiveFailures = 0;
  private toldOfTrouble = false;
  /**
   * Set when the server said the thread is terminal. Nothing can ever land on
   * a closed transcript, so from here events are dropped rather than queued
   * against a refusal that cannot change.
   */
  private threadClosed = false;
  /**
   * Set when `close` has run. The session is leaving: whatever could not be
   * posted by the flush deadline is not retried into a process that no longer
   * exists, and that is the one place loss is accepted.
   */
  private stopped = false;
  /** Resolvers waiting in `close` for the queue to drain. */
  private drainWaiters: Array<() => void> = [];

  constructor(private readonly options: TranscriptWriterOptions) {
    this.delays = options.retryDelaysMs ?? RETRY_DELAYS_MS;
    this.post = options.fetch ?? globalThis.fetch.bind(globalThis);
  }

  /** How many events are queued and not yet on the server. For tests and `close`. */
  get pending(): number {
    return this.queue.length;
  }

  /**
   * Queue one event for the thread's transcript.
   *
   * Returns immediately. Events post in the order they were recorded, whatever
   * the network does in between, because the transcript is a sequence and a
   * reordered one describes a session that never happened.
   */
  record(eventType: string, payload: Record<string, unknown>): void {
    if (this.threadClosed || this.stopped) return;
    this.queue.push({ eventType, payload });
    // The pump is started here rather than awaited: the caller is the turn
    // loop, and the whole contract is that it never waits on this.
    void this.pump().catch(() => undefined);
  }

  /**
   * Wait for the queue to drain, up to a deadline.
   *
   * Called before the thread is revoked, because revoking closes the
   * transcript and anything still queued would then be refused
   * `thread_terminal`. Failure to drain by the deadline resolves rather than
   * rejects: a session on its way out has nobody left to throw to.
   */
  async close(deadlineMs = CLOSE_DEADLINE_MS): Promise<void> {
    try {
      if (this.queue.length === 0 && !this.pumping) return;

      await new Promise<void>((resolve) => {
        const timer = setTimeout(() => {
          this.drainWaiters = this.drainWaiters.filter((waiter) => waiter !== settle);
          resolve();
        }, deadlineMs);
        const settle = () => {
          clearTimeout(timer);
          resolve();
        };
        this.drainWaiters.push(settle);
      });
    } finally {
      // Whatever the flush achieved, the writer stops here: a retry loop that
      // outlived the session it was recording would hold the process open for
      // a transcript nobody is in.
      this.stopped = true;
    }
  }

  /**
   * Post the queue, in order, one event at a time.
   *
   * One pump runs at a time. A transient failure — the network refusing, a
   * 5xx — retries the same event up the backoff ladder and never skips it,
   * because posting the next event first would reorder the transcript. A
   * refusal that cannot change is treated by kind: `thread_terminal` closes
   * the writer, and any other 4xx drops that one event and moves on, since a
   * payload the server has already called invalid will be invalid tomorrow.
   */
  private async pump(): Promise<void> {
    if (this.pumping) return;
    this.pumping = true;

    try {
      while (this.queue.length > 0 && !this.threadClosed && !this.stopped) {
        const event = this.queue[0];
        if (event === undefined) break;

        // Events post strictly in order; each depends on the one before it
        // being on the server, so there is nothing here to run concurrently.
        // eslint-disable-next-line no-await-in-loop
        const outcome = await this.send(event);

        if (outcome === "posted") {
          this.queue.shift();
          this.consecutiveFailures = 0;
          continue;
        }

        if (outcome === "unsupported") {
          this.threadClosed = true;
          this.queue.length = 0;
          this.trouble(
            "This server does not serve a thread transcript yet, so this session is not " +
              "recorded on it. The work is unaffected.",
          );
          break;
        }

        if (outcome === "thread_closed") {
          this.threadClosed = true;
          this.queue.length = 0;
          this.trouble(
            "This thread is closed, so the rest of the session will not reach its transcript.",
          );
          break;
        }

        if (outcome === "refused") {
          // The server named the event invalid. Retrying cannot change that,
          // and holding the queue on it would silently stop the record.
          this.queue.shift();
          this.trouble(
            "The server refused a transcript event, which was dropped. The session continues.",
          );
          continue;
        }

        // Transient. Same event, next rung of the ladder.
        this.consecutiveFailures += 1;
        if (this.consecutiveFailures >= TROUBLE_AFTER) {
          this.trouble(
            "The thread transcript is not reaching the server. " +
              "Events are queued and will keep retrying in the background.",
          );
        }

        const rung = Math.min(this.consecutiveFailures - 1, this.delays.length - 1);
        // eslint-disable-next-line no-await-in-loop
        await sleep(this.delays[rung] ?? 0);
      }
    } finally {
      this.pumping = false;
      if (this.queue.length === 0 || this.threadClosed) {
        for (const waiter of this.drainWaiters.splice(0)) waiter();
      }
    }
  }

  /** One POST, translated to what the pump can act on. Never throws. */
  private async send(
    event: QueuedEvent,
  ): Promise<"posted" | "retry" | "refused" | "thread_closed" | "unsupported"> {
    let response: Response;
    try {
      response = await this.post(
        new URL(`${THREADS_PATH}/${this.options.threadId}/events`, this.options.origin),
        {
          method: "POST",
          headers: {
            authorization: `Bearer ${this.options.token}`,
            "content-type": "application/json",
            accept: "application/json",
          },
          body: JSON.stringify({ event_type: event.eventType, payload: event.payload }),
        },
      );
    } catch {
      return "retry";
    }

    if (response.status >= 200 && response.status < 300) return "posted";
    // A server that is down answers 5xx; the event is still good.
    if (response.status >= 500) return "retry";

    // A server without the route is a server older than this client, not a
    // server calling the event invalid. Retrying cannot help and neither can
    // editing the payload, so the record stops for the session and says why
    // once — rather than reporting a refusal on every turn for a route that was
    // never reached.
    if (response.status === 404 || response.status === 405) return "unsupported";

    const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    if (body["code"] === "thread_terminal") return "thread_closed";
    return "refused";
  }

  /** One sentence, once. A status line repeating itself is a status line ignored. */
  private trouble(message: string): void {
    if (this.toldOfTrouble) return;
    this.toldOfTrouble = true;
    this.options.onTrouble?.(message);
  }
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => {
    // Always a real timer, even at zero: a retry loop that resumed on the
    // microtask queue would starve the event loop the session runs on. The
    // timer is unreferenced so a backoff mid-wait cannot hold the process
    // open after the session that was being recorded has ended.
    const timer = setTimeout(resolve, Math.max(0, ms));
    if (typeof timer === "object" && "unref" in timer) timer.unref();
  });
