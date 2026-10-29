// A relay connection for the project tools: publish events and query them,
// answering NIP-42 AUTH with the project key when the relay asks.
// (relay.openagents.com: a REQ sent before AUTH completes poisons the
// connection, so a keyed session waits for the challenge first.)
import { finalizeEvent, type Event, type EventTemplate } from "nostr-effect/pure"

export type Filter = Record<string, unknown>

/** A relay answer that didn't come: refused, timed out, cut off or rate-limited.
 * `transient` means trying the same request again later can succeed. */
export class RelayError extends Error {
  constructor(message: string, readonly transient: boolean) { super(message) }
}

/** NIP-01 machine-readable prefixes a relay puts on a refusal it may lift later. */
const TRANSIENT = /^(rate-limited|error|auth-required):/
const transient = (reason: string) => TRANSIENT.test(reason)

export type Retry = { tries: number; baseMs: number; maxMs: number; log: (line: string) => void }
export const RETRY: Retry = { tries: 8, baseMs: 1000, maxMs: 30000, log: (l) => console.error(l) }

/** A page of stored events; `finish` is the relay's NIP-67 word that nothing older is left. */
export type Page = Event[] & { finish?: boolean }

export class Relay {
  private ws!: WebSocket
  private authed = false
  private waiters = new Map<string, (ok: boolean, msg: string) => void>()
  private subs = new Map<string, { events: Event[]; done: (hints: unknown) => void; fail: (e: RelayError) => void }>()
  private challenge!: Promise<void>
  private release!: () => void
  retry: Retry = RETRY

  private constructor(readonly url: string, private key: Uint8Array | null, private timeoutMs: number) {}

  /** Opens the connection, retrying an unreachable or slow relay with backoff. */
  static async open(url: string, key: Uint8Array | null, timeoutMs = 10000): Promise<Relay> {
    const r = new Relay(url, key, timeoutMs)
    await r.again("connecting", () => r.connect())
    return r
  }

  private connect(): Promise<void> {
    this.authed = false
    this.challenge = new Promise((r) => (this.release = r))
    const ws = new WebSocket(this.url)
    this.ws = ws
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => { ws.close(); reject(new RelayError(`${this.url}: timed out connecting`, true)) }, this.timeoutMs)
      ws.onerror = () => { clearTimeout(timer); reject(new RelayError(`${this.url}: unreachable`, true)) }
      ws.onopen = () => { clearTimeout(timer); resolve() }
      ws.onclose = () => { clearTimeout(timer); if (this.ws === ws) this.dropped() }
      ws.onmessage = (m) => { try { this.receive(JSON.parse(String(m.data))) } catch {} }
    }).then(async () => {
      // a keyed session waits briefly for the relay's challenge and answers it
      if (this.key) await Promise.race([this.challenge, Bun.sleep(1200)])
      else this.release()
    })
  }

  /** The connection went away: every request in flight fails, and can be tried again. */
  private dropped() {
    const err = new RelayError(`${this.url}: connection closed`, true)
    for (const [, s] of this.subs) s.fail(err)
    this.subs.clear()
    for (const [, w] of this.waiters) w(false, "error: connection closed")
    this.waiters.clear()
  }

  private async ready() {
    if (this.ws.readyState !== WebSocket.OPEN) await this.connect()
  }

  /** Runs one request, retrying transient failures with jittered exponential backoff. */
  private async again<T>(what: string, run: () => Promise<T>): Promise<T> {
    const { tries, baseMs, maxMs, log } = this.retry
    for (let n = 1; ; n++) {
      try { return await run() } catch (e) {
        const err = e instanceof RelayError ? e : new RelayError(String((e as Error)?.message ?? e), true)
        if (!err.transient) throw err
        if (n >= tries) throw new RelayError(`${this.url}: gave up ${what} after ${tries} tries (${err.message})`, false)
        const wait = Math.round(Math.min(maxMs, baseMs * 2 ** (n - 1)) * (0.75 + Math.random() / 2))
        log(`relay busy ${what} (${err.message}); retrying in ${(wait / 1000).toFixed(1)}s (${n}/${tries - 1})`)
        await Bun.sleep(wait)
      }
    }
  }

  private receive(msg: unknown[]) {
    const [type] = msg
    if (type === "AUTH" && this.key && !this.authed) {
      const auth = finalizeEvent({ kind: 22242, created_at: Math.floor(Date.now() / 1000), content: "",
        tags: [["relay", this.url], ["challenge", String(msg[1])]] }, this.key)
      this.waiters.set(auth.id, (ok) => { this.authed = ok; this.release() })
      this.ws.send(JSON.stringify(["AUTH", auth]))
    } else if (type === "OK") {
      const w = this.waiters.get(String(msg[1]))
      if (w) { this.waiters.delete(String(msg[1])); w(Boolean(msg[2]), String(msg[3] ?? "")) }
    } else if (type === "EVENT") {
      this.subs.get(String(msg[1]))?.events.push(msg[2] as Event)
    } else if (type === "EOSE") {
      this.subs.get(String(msg[1]))?.done(msg[2])
    } else if (type === "CLOSED") {
      // a refused subscription is never an empty answer
      const reason = String(msg[2] ?? "")
      this.subs.get(String(msg[1]))?.fail(new RelayError(`query refused: ${reason || "closed"}`, transient(reason)))
    }
  }

  /** Publishes a signed event; resolves with the relay's OK message, rejects when refused.
   * A retry sends the same signed event, so the relay stores it once ("duplicate:" is OK). */
  publish(event: Event, timeoutMs = this.timeoutMs): Promise<string> {
    return this.again(`publishing kind ${event.kind}`, () => this.publishOnce(event, timeoutMs))
  }

  private async publishOnce(event: Event, timeoutMs: number): Promise<string> {
    await this.ready()
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.waiters.delete(event.id); reject(new RelayError(`kind ${event.kind}: relay timed out`, true)) }, timeoutMs)
      this.waiters.set(event.id, (ok, msg) => {
        clearTimeout(timer)
        if (ok) resolve(msg)
        else reject(new RelayError(`kind ${event.kind} refused: ${msg}`, transient(msg)))
      })
      this.ws.send(JSON.stringify(["EVENT", event]))
    })
  }

  /** Every stored event matching the filters, up to EOSE. A timeout, a CLOSED or a
   * dropped connection rejects (and is retried), never answering short. */
  query(filters: Filter[], timeoutMs = this.timeoutMs): Promise<Page> {
    return this.again("reading", () => this.queryOnce(filters, timeoutMs))
  }

  private async queryOnce(filters: Filter[], timeoutMs: number): Promise<Page> {
    await this.ready()
    const id = `q${Math.random().toString(36).slice(2, 10)}`
    return new Promise((resolve, reject) => {
      const events: Page = []
      const end = () => { clearTimeout(timer); this.subs.delete(id) }
      const timer = setTimeout(() => {
        end()
        if (this.ws.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(["CLOSE", id]))
        reject(new RelayError("relay timed out before EOSE", true))
      }, timeoutMs)
      this.subs.set(id, {
        events,
        done: (hints) => {
          end()
          this.ws.send(JSON.stringify(["CLOSE", id]))
          if (Array.isArray(hints) && hints.includes("finish")) events.finish = true
          resolve(events)
        },
        fail: (e) => { end(); reject(e) },
      })
      this.ws.send(JSON.stringify(["REQ", id, ...filters]))
    })
  }

  /** Every stored event for each filter, one filter at a time and paged back in time:
   * relays cap what one REQ returns (relay.openagents.com cut a three-filter REQ short),
   * and a short answer must never read as "not published". */
  async all(filters: Filter[], page = 200): Promise<Event[]> {
    const seen = new Map<string, Event>()
    for (const f of filters) {
      let until: number | undefined
      let ended = false
      for (let round = 0; round < 100; round++) {
        const got: Page = await this.query([{ ...f, limit: page, ...(until === undefined ? {} : { until }) }])
        if (got.length === 0) { ended = true; break }
        for (const e of got) seen.set(e.id, e)
        if ((got as Page).finish) { ended = true; break }
        const oldest = Math.min(...got.map((e) => e.created_at))
        if (!Number.isSafeInteger(oldest) || oldest < 0 || (until !== undefined && oldest > until))
          throw new Error("relay returned an invalid pagination timestamp")
        if (oldest === until) {
          // Inclusive until returns the boundary again. Multiple events at a
          // stalled boundary may conceal a relay-capped same-second cohort.
          if (got.length > 1) throw new Error("relay timestamp cohort cannot be paged without possible loss")
          if (oldest === 0) { ended = true; break }
          until = oldest - 1
        } else until = oldest
        // A short response is NOT exhaustion: the relay may cap below limit.
        // Scan each filter independently even if another already saw its rows.
      }
      if (!ended) throw new Error("relay pagination budget exhausted; refusing partial results")
    }
    return [...seen.values()]
  }

  close() { this.ws.close() }
}

/** Signs with strictly increasing created_at, so two edits in one second still order. */
export class Signer {
  private last = 0
  constructor(private key: Uint8Array) {}
  /** Never sign at or before this time: a replaceable event or a status from
   * an earlier run in the same second would otherwise tie, and a tie can keep the old one. */
  after(seen: number) { this.last = Math.max(this.last, seen) }
  sign(t: Omit<EventTemplate, "created_at">): Event {
    const now = Math.floor(Date.now() / 1000)
    this.last = Math.max(now, this.last + 1)
    return finalizeEvent({ ...t, created_at: this.last }, this.key)
  }
}
