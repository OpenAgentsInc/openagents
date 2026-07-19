# Managed-sandbox owner broker

Issue: [SBX-07 #9030](https://github.com/OpenAgentsInc/openagents/issues/9030)

The OpenAgents Worker now has one native managed-sandbox broker shared by Sarah
and Desktop. It composes the durable Postgres lifecycle store with the private
Google Cloud runtime adapter. Box v1 remains a compatibility projection over
the same native contracts.

## Runtime admission

The component is default-off. `MANAGED_SANDBOX_BROKER_ENABLED=true` selects the
owner broker only when all of these are also configured:

- `OA_MANAGED_SANDBOX_IMAGE_DIGEST=sha256:<64 lowercase hex>`
- `OA_CLOUD_CONTROL_URL=<private admitted control service>`
- `OA_CLOUD_CONTROL_TOKEN=<Worker secret>`
- `KHALA_SYNC_DB` for the generation-fenced lifecycle store.

Missing configuration returns typed unavailability. There is no local, fake,
weaker-isolation, or alternate-provider fallback. SBX-09 owns deployment,
independent live acceptance, flag activation, rollback, and zero-residue proof.

## Callers

Sarah advertises exactly eight functions: create, list, inspect, dispatch,
interrupt, stop, resume, and delete. Each call first resolves and persists the
revision-4 Sarah authority decision for the authenticated stable owner thread.
The target effect cannot run on refusal. Results contain ordered readable
activity, the authority receipt, the native target receipt, current resource
truth, and bounded turn/event receipts when present.

Desktop calls two authenticated main-process endpoints:

- `POST /api/managed-sandboxes/desktop/admission`
- `POST /api/managed-sandboxes/desktop/commands`.

Admission returns only the exact target, immutable image, profile, bounded
lease/budget/capabilities, deny-all network posture, custody, and retention
identity. Commands use canonical `openagents.managed_sandbox_command.v1`
values. Dispatch additionally carries prompt bytes whose SHA-256 digest must
match the canonical command before runtime execution. Create additionally
carries the exact positive Desktop/Sarah attachment generation in the
authenticated broker envelope. The broker persists it into the native resource
without widening the frozen command schema.

The broker resolves an existing durable reservation before retry. It reuses the
original canonical request timestamp, returns the same accepted receipt while
the command remains pending, and refuses changed request bytes or a substituted
attachment generation under the same command identity.

## Failure truth

An accepted create/stop/resume/delete receipt is pending lifecycle truth, not
completion. Quiet output is not idle or terminal. A failed cleanup remains
`recovery_required`. Delete is complete only when the native resource is
`deleted` and `cleanupComplete` is true. No caller receives cloud credentials,
guest addresses, topology, host paths, raw shell/database access, generic
container administration, or cross-machine Full Auto admission.
