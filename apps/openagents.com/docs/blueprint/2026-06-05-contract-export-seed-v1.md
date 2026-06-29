# Blueprint Contract Export Seed V1

Issue #236 adds the first Blueprint contract export seed for agents and
Rust-side consumers.

The seed lives at
`workers/api/src/blueprint/exports/contract-export.ts`. It does not yet expose a
live HTTP route or generate full JSON Schema files. Instead, it records the
stable export map that future generators and routes must satisfy:

- JSON Schema refs and future schema URLs for Objective, Program, Module,
  Program Run, Probe Program Run evidence intake, Action Submission, Source
  Authority, Context Pack, Release Gate, Optimizer Run, Simulation Branch,
  Program Registry, Probe Action Submission proposal intake, Probe Blueprint
  contribution intake, and Smoke Probe records.
- OpenAPI operation refs for the Program Registry route, contract export route,
  Probe Program Run evidence intake route, Probe Action Submission proposal
  route, and Probe Blueprint contribution route.
- Event catalog entries for Program Run recorded, Action Submission proposed,
  Release Gate decided, and Smoke Probe failed.
- Receipt catalog entries for Program Run, Action Submission, Release Gate, and
  probe failure receipts.

The seed covers these consumers:

- AI agents;
- `oa-node`;
- `oa-workroomd`;
- Probe;
- Psionic;
- Pylon;
- Nexus;
- Treasury.

The export is intentionally ref-only. It may contain stable IDs, schema refs,
OpenAPI component refs, event refs, topic refs, receipt refs, retention policy
refs, privacy policy labels, and consumer names. It must not contain provider
payloads, raw emails, private customer data, access tokens, refresh tokens,
wallet secrets, payment preimages, private keys, mnemonics, or raw run logs.

Future work can generate JSON Schema/OpenAPI artifacts from this seed after the
schema generation policy is ready. Rust consumers should treat this seed as the
compatibility map, not as proof that all downstream adapters are already
implemented.
