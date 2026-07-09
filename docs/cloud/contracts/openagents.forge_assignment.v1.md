# openagents.forge_assignment.v1

Status: ratified scaffold for Cloud MVP issue `CND-010`

This contract is the typed assignment boundary between Forge and private
OpenAgents Cloud nodes. It is intentionally narrower than a general labor queue:
cloud nodes receive bounded workroom or worker assignments, while open-ended
labor remains under Forge/Probe operator routing.

## Assignment

Required top-level fields:

```text
contract_version
assignment_id
org_id
program_id
workroom_id
node_id
assignment_kind
template
capability
budget
artifacts
receipts
```

`assignment_kind` is one of:

- `workroom`
- `worker`
- `open_ended_labor`

`open_ended_labor` is valid Forge input but the `oa-node` adapter refuses it so
it can route back through Forge/Probe rather than being treated as sandbox
compute.

## Required Policy Blocks

`template` carries:

```text
template_id
runtime_profile
template_digest
```

`capability` carries:

```text
capability_id
capability_scope
required
```

`budget` carries:

```text
max_runtime_ms
max_cost_microusd
max_artifact_bytes
```

Sandbox worker assignments also carry `sandbox`:

```text
profile_id
profile_digest
execution_class
network_policy
filesystem_policy
secret_policy
```

`oa-node` accepts sandbox worker assignments only when this block matches a
locally registered sandbox profile and the assignment timeout/artifact budgets
fit within that profile. Missing or mismatched sandbox policy is refused with a
normal assignment receipt.

`artifacts` carries:

```text
artifact_sink
required_artifacts
retention
```

`receipts` carries:

```text
receipt_sink
required_receipts
closeout_required
```

## Receipt

Every accepted or refused local intake emits
`openagents.forge_assignment_receipt.v1`:

```text
receipt_id
assignment_id
node_id
decision
reason
assignment_digest
receipt_digest
emitted_at_ms
```

The assignment digest is computed over the received assignment JSON. The receipt
digest is computed over receipt fields excluding itself. Receipts are append-only
local evidence and are projected into `openagents.cloud_node.v1`
`evidence.job_receipts`.

## Fixture Set

The executable fixture set lives in `fixtures/forge_assignment_v1/`:

- `workroom-assignment.json`
- `open-ended-labor-assignment.json`

The `openagents-cloud-contract` crate parses and validates both fixtures.
