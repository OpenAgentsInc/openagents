# oa-node Signed Updates And Rollback

Status: Cloud MVP scaffold for `CND-019`

`oa-node update` models signed release-channel policy and receipt-bearing update
application. The MVP is file-backed and records update outcomes without
downloading or replacing binaries.

```bash
oa-node update status --json
oa-node update policy set --channel stable --pin 0.2.0 --json
oa-node update policy set --channel stable --defer --json
oa-node update apply \
  --target-version 0.2.0 \
  --signer local-keychain://openagents/cloud/release \
  --signature-digest sha256:abc123 \
  --json
oa-node update apply \
  --target-version 0.3.0 \
  --signer local-keychain://openagents/cloud/release \
  --signature-digest sha256:def456 \
  --result failed \
  --json
oa-node update rollback \
  --target-version 0.2.0 \
  --signer local-keychain://openagents/cloud/release \
  --signature-digest sha256:rollback \
  --json
```

The update files are:

```text
admin-store.json
update-receipts.jsonl
health-events.jsonl
```

Update receipts record action, previous version, target version, signer,
signature digest, result, receipt digest, and emission time. Successful applies
promote `current_version`. Failed applies roll back to the previous version when
one exists; if there is no previous version, the node is quarantined. Rollback
commands explicitly set the target rollback version and emit a rollback receipt.

Fleet policy can pin a node to an exact version or defer updates. Deferred or
pin-mismatched updates do not change `current_version`; they set
`pending_update` and emit receipts with `deferred` or `deferred_pinned`.

Signer refs, versions, channels, and receipt fields are rejected when they
contain raw secret, token, wallet, private-key, or private-topology markers.
