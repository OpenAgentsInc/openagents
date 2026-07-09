# oa-node Settlement Modes

Status: Cloud MVP scaffold for `CND-021` — **authority rewrite 2026-07-09 (#8591)**

`oa-node settlement` keeps managed Cloud settlement **policy labels** explicit
and separate from public contributor Pylon wallet behavior and from customer
credit ledgers on the `openagents.com` Worker.

```bash
oa-node settlement status --json
oa-node settlement mode set no-wallet --json
oa-node settlement mode set internal-accounting \
  --treasury-ref accounting://batch/local-1 \
  --nexus-ref payout-bridge://settlement/local-1 \
  --json
oa-node settlement receipt append \
  --amount-microusd 12345 \
  --treasury-ref accounting://batch/local-1 \
  --nexus-ref payout-bridge://settlement/local-1 \
  --json
```

> **Ref naming:** CLI flags historically used `--treasury-ref` / `--nexus-ref`.
> They store **opaque public-safe accounting / payout-bridge refs only**. They
> do not grant the deprecated Treasury product authority or the old Nexus
> product authority. Customer credits and ledgers live on the Worker; outbound
> payout/custody remains on the **MDK/Nexus payout bridge** where that path is
> still active.

Managed nodes default to `no-wallet`. Supported modes:

- `no-wallet`
- `internal-accounting`

`contributor-wallet` is rejected here because it belongs to public contributor
Pylon. This keeps public wallet UX and managed-node accounting metadata from
sharing implementation authority.

Internal-accounting receipts record amount, accounting reconciliation ref,
payout-bridge settlement ref, result, and receipt digest. `oa-node status
--json` projects `internal_accounting` into `policy.settlement_policy` and
exposes the latest accounting receipt digest in
`evidence.payout_or_accounting_receipts`.

Settlement refs are metadata refs only. Raw wallet seeds, private keys, bearer
tokens, and private topology markers are rejected.

## Authority split (current)

| Concern | Owner |
| --- | --- |
| Customer credits / ledgers / invoices | `openagents.com` Worker |
| Compute metering + redacted usage receipts | Cloud daemons + Worker ingest |
| Outbound payout / custody | MDK/Nexus payout bridge (active boundary only) |
| Contributor wallet UX | Public Pylon |
| Managed node settlement **mode** | `oa-node` (this doc) |
