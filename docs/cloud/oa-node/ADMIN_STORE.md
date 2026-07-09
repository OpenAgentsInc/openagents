# oa-node Admin Store

Status: implemented for Cloud MVP issue `CND-007`

The managed node admin store is a local file-backed MVP store under the
`oa-node` state directory:

```text
node-state.json
admin-store.json
health-events.jsonl
```

`node-state.json` owns identity. `admin-store.json` owns durable operator and
runtime control state. `health-events.jsonl` is append-only health history.

## Durable State

`admin-store.json` persists:

- desired mode;
- observed status;
- inventory summary placeholder;
- update channel/current version/pending update placeholder;
- quarantine state;
- receipt cursors, including the health event count.

The current MVP exposes desired-mode mutation and health-event append/list
commands:

```bash
oa-node admin desired-mode get --json
oa-node admin desired-mode set online --json
oa-node admin health append --severity warn --code disk_low --detail "disk warning" --json
oa-node admin health list --json
```

## Restart Behavior

Every command loads from disk. Re-running `oa-node status --json` after a
separate `desired-mode set` process projects the persisted desired mode into
`openagents.cloud_node.v1`.

Health events append one JSON object per line to `health-events.jsonl`; they are
not rewritten during normal append. Status projects event ids into
`evidence.health_events`.

## Safe Degradation

If initialized node state exists but `admin-store.json` is missing, status
returns `observed_status=degraded` with `degradation_reason=admin_store_missing`.

If the admin store or health-event log is corrupt, status still returns a valid
`openagents.cloud_node.v1` snapshot with a degradation reason instead of
panicking or advertising healthy capacity.
