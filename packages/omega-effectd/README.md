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
`work.index.read` and `work.snapshot.read`. Both methods decode and encode the
generated `@openagentsinc/all-work-contract` types. The first adapter projects
durable Full Auto runs without objective or done-condition text; it does not
create a second writable Work store.

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
