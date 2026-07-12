# D1-E: confirmed agent timeline through Desktop Runtime Gateway

- Issue: #8673
- Parents: #8574, #8566
- Status: closed; historical checked-in issue source
- Depends on: closed #8672

## Landed boundary

Desktop Runtime Gateway protocol v3 adds one schema-bounded query:

```text
agent.timeline(runRef)
```

Electron main obtains the shared #8672 reader from its authenticated Sync host,
opens exactly `scope.agent_run.<runRef>`, and returns content only after that
scope is live. A successful result contains:

- exact requested/confirmed run ref;
- the server-projected `agent_run.routeId` as `routeRef`;
- bounded lifecycle state/timestamps and confirmed entity version;
- actual scope phase/cursor/pending count; and
- at most 500 ordered confirmed event refs, sequences, generic type/summary/
  status, artifact refs, timestamps, and versions.

The route rule is now frozen: `routeRef` from confirmed server state is the
only run↔thread/route attachment fact. Renderer code may not transform or guess
it from `runRef`. This avoids silently merging canonical chat identity with the
older agent-run route convention.

Non-live, missing, and read failure return typed body-free unavailable results.
The schema cannot carry owner/objective/repository/runtime/backend, provider
source, raw payload JSON, external callback refs, credentials, store/session/
transport objects, arbitrary IPC, or process authority. No preload method or
IPC channel was added; the existing generic decoded gateway call is reused.

## Evidence

The Runtime Gateway e2e round-trips the query/result through both schema
boundaries, proves the server `routeRef` survives unchanged, verifies bounded
confirmed run/event versions, rejects an invalid traversal-shaped run ref, and
proves a catching-up/offline scope returns no content. Desktop host tests prove
the shared reader exists only behind live authenticated Sync and disappears on
denial.

Contract: `openagents_desktop.seam.runtime_gateway_agent_timeline.v1`.

## Explicit residual

No runtime launch, visible timeline, canonical chat creation for the route,
provider process stream, interrupt/resume, mobile UI, or live-account proof is
claimed here. Those later leaves consume this closed query; they do not widen
it or invent a second event model.
