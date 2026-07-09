# oa-node Quarantine

Status: Cloud MVP scaffold for `CND-020`

`oa-node quarantine` makes quarantine an explicit node control-plane state. It
blocks new work, records the policy for existing workrooms, and emits health
events plus quarantine receipts.

```bash
oa-node quarantine status --json
oa-node quarantine enter \
  --reason policy_violation \
  --workroom-policy pause \
  --workroom workroom.local.echo \
  --json
oa-node quarantine exit --reason operator_release --json
```

The quarantine files are:

```text
admin-store.json
health-events.jsonl
quarantine-receipts.jsonl
```

Entering quarantine sets desired and observed node mode to `quarantined`.
`oa-node status --json` reports that state, and Forge assignment intake refuses
new work while the node is quarantined. The workroom policy must be one of:

- `pause`
- `migrate`
- `close`

If no specific `--workroom` is supplied, the receipt records
`all_active_workrooms` so operators can see that the policy applies to the
active set known outside the local MVP.

State changes append `quarantine_entered` or `quarantine_exited` health events
and `openagents.oa_node.quarantine_receipt.v1` receipts. Quarantine fields are
rejected when they contain raw secret, token, wallet, private-key, or
private-topology markers.
