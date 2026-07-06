// LiveHubService — scope → ScopeHub map with single-flight rebuild
// (CFG-5, #8520).
//
// One service instance owns every scope at current scale. THE SHARDING
// EXTENSION POINT is `hubFor(scope)`: to shard, deploy N instances and
// route each scope to `hash(scope) % N` at the proxy (or a scope→shard
// lookup) — nothing below this seam assumes a single process, because all
// state is already per-scope.

import type { SyncScope } from "@openagentsinc/khala-sync"
import { ScopeHub, type ScopeHubBounds } from "./scope-hub.js"
import type { ChangelogEntry } from "@openagentsinc/khala-sync"
import { encodeChangelogEntry } from "@openagentsinc/khala-sync"

export type LiveHubServiceConfig = Readonly<{
  bounds?: ScopeHubBounds | undefined
  /**
   * Best-effort newest-window loader (src/rebuild.ts over Postgres).
   * `undefined` disables rebuild: hubs start empty and hydrate from
   * capture appends (the DO's fresh-hub semantics).
   */
  loadWindow?:
    | ((scope: SyncScope) => Promise<Array<ChangelogEntry>>)
    | undefined
  log?: ((line: string) => void) | undefined
}>

export class LiveHubService {
  private readonly hubs = new Map<SyncScope, ScopeHub>()
  private readonly initFlight = new Map<SyncScope, Promise<void>>()
  private readonly config: LiveHubServiceConfig
  private readonly log: (line: string) => void

  constructor(config: LiveHubServiceConfig = {}) {
    this.config = config
    this.log = config.log ?? (() => {})
  }

  scopeCount(): number {
    return this.hubs.size
  }

  socketCount(): number {
    let total = 0
    for (const hub of this.hubs.values()) total += hub.socketCount()
    return total
  }

  /**
   * The per-scope hub, created (and best-effort rebuilt from Postgres,
   * single-flight) on first touch. Every caller awaits the SAME rebuild
   * promise, so an append can never interleave with its scope's rebuild.
   */
  async hubFor(scope: SyncScope): Promise<ScopeHub> {
    const existing = this.hubs.get(scope)
    if (existing !== undefined) {
      const inflight = this.initFlight.get(scope)
      if (inflight !== undefined) await inflight
      return existing
    }

    const hub = new ScopeHub(scope, this.config.bounds ?? {})
    this.hubs.set(scope, hub)

    const loadWindow = this.config.loadWindow
    if (loadWindow !== undefined) {
      const flight = (async () => {
        try {
          const entries = await loadWindow(scope)
          if (entries.length > 0) {
            // A rebuild is just an append into an empty window (the
            // mid-stream rehydrate path); encode back to the wire shape
            // the append contract expects.
            const response = hub.append({
              entries: entries.map((entry) => encodeChangelogEntry(entry)),
              scope,
            })
            if (response.status !== 200) {
              this.log(
                `live-hub ${scope}: window rebuild append rejected ` +
                  `(http ${response.status}); starting empty`,
              )
            } else {
              this.log(
                `live-hub ${scope}: window rebuilt from Postgres ` +
                  `(${entries.length} entries)`,
              )
            }
          }
        } catch (error) {
          // Best-effort: an unreachable Postgres leaves the hub empty
          // (fresh-hub semantics; capture appends hydrate mid-stream).
          this.log(
            `live-hub ${scope}: window rebuild failed ` +
              `(${error instanceof Error ? error.message.slice(0, 200) : String(error)}); starting empty`,
          )
        } finally {
          this.initFlight.delete(scope)
        }
      })()
      this.initFlight.set(scope, flight)
      await flight
    }

    return hub
  }

  /** Keepalive tick across every scope (Cloud Run idle guard). */
  pingAll(): void {
    for (const hub of this.hubs.values()) hub.pingAll()
  }

  /** Close every socket and release every window (shutdown). */
  dispose(): void {
    for (const hub of this.hubs.values()) hub.dispose()
    this.hubs.clear()
  }
}
