# OpenAgents product surface Probe Program Run Evidence Intake

Issue #496 adds the first OpenAgents product surface route that accepts Probe-emitted Blueprint
Program Run evidence.

The route is:

- `POST /api/blueprint/program-runs`

It accepts Probe's `ProbeBlueprintProgramRunEvidence` envelope and normalizes it
into OpenAgents product surface's `BlueprintProgramRunRecord` repository. The route is authorized by
the runner callback token path, with admin API token support for operator-owned
manual verification. It returns `BlueprintProgramRunEvidenceIntakeResponse`,
which contains only the safe Program Run detail projection, receipt refs, and
the OpenAgents product surface registry version ref.

The envelope is evidence only. OpenAgents product surface rejects records that claim deploy, email,
spend, source mutation, or direct mutation authority. OpenAgents product surface also checks the raw
request body before schema decoding so unknown fields cannot smuggle raw
prompts, callback URLs or tokens, provider payloads, private file content,
private repo refs, wallet material, customer private data, or provider secrets
past an otherwise strict schema.

Accepted records preserve Probe's actor, assignment, runner, workroom, thread,
order, backend, model, lookup, menu, registry, Program Type, Program Signature,
Module Version, prompt summary, tool callback, cost, usage, latency, evidence,
receipt, and typed-output refs. OpenAgents product surface derives the Program Run `purposeRef` from
the current Blueprint Program Registry and requires the evidence
`registryVersionRef`, Program Type, Program Signature, and Module Version to
match that registry.

Registry reads now include accepted Program Runs. `GET
/api/blueprint/program-registry` lists active stored runs in `runDetails` and
adds each run id to the matching registry entry's `runIds`, while preserving the
operator-safe projection contract. Raw typed output and metadata remain in the
repository, not in the registry response.

This does not give Probe write authority. PR creation, deploys, emails,
payments, source-backed mutations, and public claim promotion still need
approval-gated Action Submissions and later release-gate intake.
