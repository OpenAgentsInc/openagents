# oa-node Settlement Modes

Status: Cloud MVP scaffold for `CND-021`

`oa-node settlement` keeps managed Cloud settlement policy explicit and separate
from public contributor Pylon wallet behavior.

```bash
oa-node settlement status --json
oa-node settlement mode set no-wallet --json
oa-node settlement mode set internal-accounting \
  --treasury-ref treasury://batch/local-1 \
  --nexus-ref nexus://settlement/local-1 \
  --json
oa-node settlement receipt append \
  --amount-microusd 12345 \
  --treasury-ref treasury://batch/local-1 \
  --nexus-ref nexus://settlement/local-1 \
  --json
```

Managed nodes default to `no-wallet`. The private Cloud repo supports only:

- `no-wallet`
- `internal-accounting`

`contributor-wallet` is rejected here because it belongs to public contributor
Pylon. This keeps public wallet UX and private managed-node accounting from
sharing implementation authority.

Internal-accounting receipts record amount, Treasury reconciliation ref, Nexus
settlement ref, result, and receipt digest. `oa-node status --json` projects
`internal_accounting` into `policy.settlement_policy` and exposes the latest
accounting receipt digest in `evidence.payout_or_accounting_receipts`.

Settlement refs are metadata refs only. Raw wallet seeds, private keys, bearer
tokens, and private topology markers are rejected.
