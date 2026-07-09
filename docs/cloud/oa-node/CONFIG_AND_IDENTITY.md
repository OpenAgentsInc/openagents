# oa-node Config And Identity

Status: implemented for Cloud MVP issue `CND-006`

`oa-node init --org <id>` creates the first local managed-node state file. The
state file stores node identity, org binding, service metadata, local paths,
and a signing-key reference. It does not store signing-key material, wallet
seeds, bearer tokens, preimages, or raw accounting credentials.

## Commands

```bash
oa-node init --org org.openagents.test
oa-node init --org org.openagents.test --json
oa-node status --json
oa-node doctor --json
```

Use `--state-dir <path>` for local tests or non-default service layouts. Without
that flag, `oa-node` uses:

```text
$OPENAGENTS_CLOUD_NODE_HOME
```

or, when that environment variable is not set:

```text
$HOME/.openagents/cloud/oa-node
```

## Idempotency

Re-running `init` with the same org and state directory reuses the existing
node identity and returns `existing: true`. Re-running `init` with a different
org, node id, or signing-key reference for the same state directory is rejected
instead of rewriting identity behind the operator's back.

## Status Projection

`oa-node status --json` emits `openagents.cloud_node.v1`. Before init it emits
the managed scaffold. After init it projects the persisted identity into the
contract:

- `identity.node_id`
- `identity.operator_identity`
- `identity.account_or_org_binding`
- `identity.signing_key_ref`

The signing-key field is a reference only. The init output redacts it to
`configured`.

After identity init, durable operator/runtime control state lives in:

```text
docs/oa-node/ADMIN_STORE.md
```
