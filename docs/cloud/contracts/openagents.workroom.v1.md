# openagents.workroom.v1

Status: ratified scaffold for Cloud MVP issue `CND-002`

This contract describes the private workroom sidecar shape. A workroom is the
user-facing execution boundary for a repo, agent, task, preview, artifact set,
and receipt trail.

## Identity

```text
workroom_id
org_id
program_id
template_id
node_id
contract_version
```

## Runtime

```text
runtime_kind
image_or_profile_digest
workspace_digest
resource_limits
network_policy
filesystem_policy
timeout_policy
```

## Capabilities

```text
github_repo_access
model_gateway_access
email_send_receive
artifact_store_write
private_api_proxy
peer_workroom_call
receipt_sink
memory_context_read
```

## Local Gateways

```text
/openagents/model
/openagents/artifacts
/openagents/receipts
/openagents/memory
/openagents/email
/openagents/settlement
```

## Receipt Events

```text
created
capability_attached
preview_exposed
collaborator_granted
token_minted
endpoint_token_minted
ingress_revoked
artifact_uploaded
closeout_submitted
destroyed_or_archived
```

## Invariants

- Workrooms start private.
- Workrooms do not receive wallet authority by default.
- Any capability attachment requires a `capability_attached` receipt.
- Any public or collaborator ingress exposure requires a `preview_exposed`
  receipt.
- Any collaborator grant, endpoint token mint, custom-domain change, or ingress
  revocation requires a receipt.
- Any artifact upload requires an `artifact_uploaded` receipt that cites the
  content digest.
- Closeout requires every declared artifact to have a matching digest and
  upload receipt.
- Workroom lifecycle transitions are explicit, validated, restart-safe, and
  receipt-bearing.
- Destroy is terminal and cannot run before required closeout policy is
  satisfied.
- Local gateway paths stay under `/openagents/`.

## Local Metadata

`oa-workroomd metadata get` exposes non-secret workroom context for agents:
workroom id, program id, repo, template id, budget, deadline, trust tier, and
capability names. It rejects raw secret, token, wallet, private key, and private
topology markers, and appends every read to `metadata-access.jsonl`.

## Link-Local Gateways

`oa-workroomd gateway access` checks model, artifacts, receipts, memory, email,
and settlement gateway access against explicit capability allow-lists. Every
access appends a redacted audit event to `gateway-access.jsonl`; revoked
capabilities deny on the next access check without process restart.

## Managed Preview Ingress

`oa-workroomd ingress status` returns file-backed ingress policy from
`ingress-state.json`. Workrooms default to `private`. Public and collaborator
visibility changes emit `preview_exposed` receipts. Named collaborator grants
emit `collaborator_granted` receipts. Endpoint tokens are stored only as
`sha256:` digests and emit `endpoint_token_minted` receipts. Revocations remove
matching token digests, collaborator grants, custom domains, or public preview
visibility and emit `ingress_revoked` receipts.

## Artifact Closeout

`oa-workroomd artifacts policy init` declares required output names.
`oa-workroomd artifacts upload` stores file content under
`artifacts/sha256/<digest>` and emits an `artifact_uploaded` receipt with the
content digest. `oa-workroomd closeout submit` fails closed until every required
artifact has a content object and matching upload receipt. Successful closeout
writes `closeout-manifest.json` with artifact digests, upload receipt digests,
status, and a manifest digest; it also emits a `closeout_submitted` receipt
citing the manifest digest for Forge verification.

## Lifecycle

`oa-workroomd lifecycle` persists the state machine in `lifecycle-state.json`
and appends accepted transitions to `lifecycle-receipts.jsonl`. The explicit
states are `not_created`, `created`, `running`, `paused`, `exposed`,
`closed_out`, `archived`, and `destroyed`. Accepted actions are `create`,
`start`, `pause`, `resume`, `expose`, `closeout`, `archive`, and `destroy`.
Invalid transitions fail closed without writing a receipt. `destroyed` is
terminal. If artifact policy declares required outputs, lifecycle `closeout`
and `destroy` require a submitted closeout manifest first.

## Fixture Set

The executable fixture set lives in `fixtures/workroom_v1/`:

- `private-workroom.json`
- `capability-attached-workroom.json`
- `preview-exposed-workroom.json`

The `openagents-cloud-contract` crate parses and validates all three fixtures.
