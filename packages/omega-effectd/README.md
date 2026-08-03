# `@openagentsinc/omega-effectd`

Supervised Node 24 + Effect Full Auto engine for Omega.

## Authority

- Freeze: `docs/omega/2026-07-24-full-auto-contract-freeze.md` (`OMEGA-FA-00`)
- Port audit: `docs/omega/2026-07-24-full-auto-port-audit.md` (`OMEGA-FA-01`)
- Omega issue: https://github.com/OpenAgentsInc/omega/issues/20

## Laws

- Active run limit: 8
- One active lease per thread
- Mutation API: `full-auto-run-actions` (engine module)
- Data root: injected (`OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT` / `OmegaEffectdPaths`)
- Never use a Zed data root or a hard-coded Electron userData path

## Pack digest

```sh
pnpm --dir packages/omega-effectd run pack:digest
```

Omega pins the printed `sha256`. It must not import this package through a
relative monorepo path or an unpublished `workspace:*` edge.

Framed protocol schema: `openagents.omega.effectd.v1` (stdio JSON lines).

Initialization also negotiates the digest-bound All Work capability profile.
Legacy clients that omit `allWork` select `omega-effectd.v1` explicitly and
cannot call Work methods. Clients that negotiate `omega-effectd.v2` can call
`work.index.read`, `work.snapshot.read`, `planning.graph.read`, and separately
negotiated claim, signed Workroom, and `work.command.execute` methods. These
methods decode and encode generated `@openagentsinc/all-work-contract` types.
The first Work adapter projects durable Full Auto runs without objective or
done-condition text. The planning method opens the Effect-owned persistent
planning authority, idempotently reconciles the checked-in v0.2.0 bootstrap,
and returns its generated graph. Neither read creates a second writable Work
store, and the GitHub bootstrap grants no command authority.

`work.command.execute` opens one durable, digest-addressed command state per
canonical Work. The service derives the Organization and authorized owner or
human assignee from owned planning state, then delegates revision,
idempotency, generation, grant, session, effect, and Owner Disposition checks
to the shared authority. It never treats a caller-supplied Organization as the
bootstrap authority and never writes to GitHub.
`work.snapshot.read` returns that durable command projection for canonical
planning Work after restart; it falls back to the owned planning snapshot when
no command record exists. Full Auto Work keeps its existing source-owned read
path.

The same generation-fenced stream carries `host_request` / `host_response`
frames for workspace resolution, thread creation, lane readiness, exact-turn
dispatch, evidence refresh, interruption, and owner-visible system notes.
Frames are bounded to 64 KiB and stale, duplicate, or late host replies never
update service state. Unanswered calls expire after 30 seconds with
`host_timeout`. A missing host adapter refuses with `host_unavailable`.
A host-confirmed missing thread settles its run to `stalled` with
`host_thread_missing`.

The private host method `resolve_sync_session` supplies an admitted
OpenAgents HTTPS base URL and runtime-only bearer to the Sync publisher and
mobile control-intent consumer. An unavailable or unsupported method keeps
Sync unavailable and never blocks local Full Auto dispatch. The service has
no environment-token or Pylon fallback. It persists only typed mobile intent
outcomes, so a lost report response or restart cannot apply one control action
twice. It never persists the bearer.

`stop` is a local terminal control and never refreshes provider authentication,
lane readiness, or host evidence. A provider becoming unavailable cannot strand
a paused run. Cached live-turn evidence is used only to request interruption of
an already-known active turn.

## Deferred

MemoHarness and initiative stay out of the first Omega port (FA-00 freeze).
