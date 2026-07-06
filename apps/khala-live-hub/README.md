# khala-live-hub

The owned Google Cloud Run replacement for the `openagents.com` Worker's
`KhalaSyncHubDO` (CFG-5, [#8520](https://github.com/OpenAgentsInc/openagents/issues/8520);
epic [#8515](https://github.com/OpenAgentsInc/openagents/issues/8515)):
the Khala Sync per-scope live hub — recent changelog window, live-tail
WebSocket fan-out, offset-resumable catch-up, MustRefetch-below-window —
as a Bun WS/HTTP service. Postgres stays authoritative; this service is a
CACHE AND FAN-OUT LAYER ONLY (docs/khala-sync/SPEC.md §5 semantics,
ported 1:1 — see `src/scope-hub.ts`).

## Surfaces (same relative paths the DO served)

| Route | Purpose |
| --- | --- |
| `GET /healthz` | liveness (no auth) |
| `POST /append?scope=…` | capture batch append (idempotent by version, dense with the window edge, 409 `khala_sync_hub_version_gap` on gaps) |
| `GET /log?scope=…&cursor=…&limit=…` | LogPage from the window; 410 behind-window / 409 ahead-of-window typed `SyncError`s |
| `POST /access-changed` `{scope}` | broadcast `MustRefetch(access_changed)` + close every scope socket (KS-7.1) |
| `GET /connect?scope=…&cursor=…` | live-tail WebSocket upgrade (catch-up from cursor, then DeltaFrame fan-out) |

Auth: one shared bearer (`KHALA_LIVE_HUB_TOKEN`) on everything except
`/healthz`, via `Authorization: Bearer …` or `?token=` (WebSocket clients
cannot always set upgrade headers — the same channel as commit
`b45071b9b6` on the Worker's public connect route, which is preserved:
the route still authenticates END USERS itself via
`withBearerFromQueryToken`, then its proxy swaps in this service bearer).

End-user auth/scope authorization (KS-7.1) does NOT live here — the
`/api/sync/*` route layer gates BEFORE proxying, exactly as it did in
front of the DO.

## What replaced DO storage persistence

A Cloud Run restart loses the in-memory windows. On first touch of a
scope the service rebuilds the newest window from Postgres
(`src/rebuild.ts`, direct Cloud SQL connection, single-flight per scope);
if Postgres is unreachable the hub starts empty and capture's next append
hydrates it mid-stream — the DO's own reset semantics.

## Scaling / sharding extension point

One instance (min=max=1) owns every scope at current scale — appends and
sockets for a scope MUST land on the same instance. The documented seam
for scaling is `LiveHubService.hubFor(scope)` (`src/service.ts`): shard
scopes across N instances by `hash(scope) % N` at the proxy, or a
scope→shard lookup. Do not raise max-instances without building that.

## Deploy

```sh
bun run deploy:cloudrun            # staging (khala-live-hub-staging)
bash scripts/deploy-cloudrun.sh prod
```

See `scripts/deploy-cloudrun.sh` for secrets (`khala-live-hub-token`,
`khala-live-hub-database-url-<env>`) and the deliberate Cloud Run flags
(session affinity, timeout 3600, no CPU throttling, single instance).

## Test

```sh
bun test           # ported KhalaSyncHubDO unit suite + real Bun.serve WS E2E
bun run typecheck
```
