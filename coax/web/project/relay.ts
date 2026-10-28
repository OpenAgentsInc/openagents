// A relay connection for the project tools: publish events and query them,
// answering NIP-42 AUTH with the project key when the relay asks.
// (relay.openagents.com: a REQ sent before AUTH completes poisons the
// connection, so a keyed session waits for the challenge first.)
import { finalizeEvent, type Event, type EventTemplate } from "nostr-effect/pure"

export type Filter = Record<string, unknown>

export class Relay {
  private ws!: WebSocket
  private authed = false
  private waiters = new Map<string, (ok: boolean, msg: string) => void>()
  private subs = new Map<string, { events: Event[]; done: () => void }>()
  private challenge: Promise<void>

  private constructor(readonly url: string, private key: Uint8Array | null) {
    let release!: () => void
    this.challenge = new Promise((r) => (release = r))
    this.release = release
  }
  private release: () => void

  static async open(url: string, key: Uint8Array | null, timeoutMs = 10000): Promise<Relay> {
    const r = new Relay(url, key)
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`${url}: timed out connecting`)), timeoutMs)
      r.ws = new WebSocket(url)
      r.ws.onerror = () => { clearTimeout(timer); reject(new Error(`${url}: unreachable`)) }
      r.ws.onopen = () => { clearTimeout(timer); resolve() }
      r.ws.onmessage = (m) => r.receive(JSON.parse(String(m.data)))
    })
    // a keyed session waits briefly for the relay's challenge and answers it
    if (key) await Promise.race([r.challenge, Bun.sleep(1200)])
    else r.release()
    return r
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
    } else if (type === "EOSE" || type === "CLOSED") {
      this.subs.get(String(msg[1]))?.done()
    }
  }

  /** Publishes a signed event; resolves with the relay's OK message, rejects when refused. */
  publish(event: Event, timeoutMs = 10000): Promise<string> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.waiters.delete(event.id); reject(new Error(`kind ${event.kind}: relay timed out`)) }, timeoutMs)
      this.waiters.set(event.id, (ok, msg) => {
        clearTimeout(timer)
        if (ok) resolve(msg)
        else reject(new Error(`kind ${event.kind} refused: ${msg}`))
      })
      this.ws.send(JSON.stringify(["EVENT", event]))
    })
  }

  /** Every stored event matching the filters, up to EOSE. */
  query(filters: Filter[], timeoutMs = 10000): Promise<Event[]> {
    const id = `q${Math.random().toString(36).slice(2, 10)}`
    return new Promise((resolve) => {
      const events: Event[] = []
      const finish = () => { clearTimeout(timer); this.subs.delete(id); this.ws.send(JSON.stringify(["CLOSE", id])); resolve(events) }
      const timer = setTimeout(finish, timeoutMs)
      this.subs.set(id, { events, done: finish })
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
      for (let round = 0; round < 100; round++) {
        const got = await this.query([{ ...f, limit: page, ...(until === undefined ? {} : { until }) }])
        const fresh = got.filter((e) => !seen.has(e.id))
        for (const e of got) seen.set(e.id, e)
        if (fresh.length === 0 || got.length < page) break
        until = Math.min(...got.map((e) => e.created_at))
      }
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
