# Blueprint Program Registry Projection V1

Issue #234 adds the first OpenAgents product surface-owned Program Registry projection contract.
The registry is an operator-safe inspection surface for Program Types, Program
Signatures, Module Versions, Release Gates, recent Program Runs, evidence refs,
receipt refs, failure refs, and promotion state.

The initial HTTP surface is now live as an authenticated operator/API-token
route at `GET /api/blueprint/program-registry`. The response returns the same
`BlueprintProgramRegistryProjection` seed and includes
`x-blueprint-registry-version-ref` so Probe and Pylon clients can pin the
registry they decoded. `GET /api/blueprint/contracts` returns the matching
Blueprint contract export catalog for Probe, Pylon, and other runtime
consumers.

The projection excludes raw run `typedOutput` and `metadata`. Run details expose
only refs, status, confidence, latency, authority flags, and promotion state.
This keeps provider payloads, private customer text, internal traces, and raw
agent reasoning out of the registry UI/API path.

Promotion state is derived from Module Versions and Release Gates:

- production modules project as `production`;
- promotable release gates project as `promotable`;
- blocked, rejected, failed, or self-promotion gates project as `blocked`;
- draft or pending gates project as `review_pending`;
- unpromoted module candidates project as `candidate`.

The seed covers every Autopilot continuation action and leaves direct mutation
disabled. Program Runs remain evidence-only; write authority still belongs to
Action Submission plus approval and receipt paths.
