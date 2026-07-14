# VP-1 treasury wallet recovery runbook

- Date: 2026-07-14
- Issue: [#8795](https://github.com/OpenAgentsInc/openagents/issues/8795)
- Status: owner procedure; not executed by this document
- Scope: recover the retired OpenAgents treasury MDK and Spark wallet value
- Last observed balance: approximately `9,274 sats` (`8,622` MDK and `652`
  Spark), before fees and subject to fresh wallet synchronization

## Safety boundary

This procedure moves real bitcoin and therefore requires the owner to choose
the destination and approve the sweep. An agent may prepare checks and
redacted receipts, but must not start a wallet or send funds without that
approval.

The preserved recovery authority is the Secret Manager material formerly
bound as `MDK_TREASURY_MNEMONIC` and `MDK_TREASURY_ACCESS_TOKEN`. Never print,
copy into a shell transcript, commit, issue, log, temporary archive, or public
receipt either value. The last observed balances are recovery estimates, not
proof of current spendability. Fees may make the final received amount lower.

Exactly one process may hold the mnemonic at a time. Before recovery, prove
that the production and staging treasury services, Cloudflare treasury
container, local daemons, and any previous recovery process are stopped. Do
not restore while the old production container can still start: two writers
against one wallet can corrupt state or duplicate a send.

## Admission

The owner records, using refs rather than secret values:

1. the approved receiving wallet and a locally verified destination;
2. the expected mainnet network and the last observed MDK/Spark balance
   buckets;
3. proof that every old treasury writer is at zero;
4. the exact recovery-tool versions and their artifact hashes; and
5. an encrypted, owner-controlled evidence location outside the repository.

Secret Manager access should be granted only to the isolated recovery identity
for the duration of the procedure. Inject secrets directly into that process;
do not write an env file or command-line argument. Disable shell tracing,
history, crash upload, and verbose SDK logging before secret injection.

## Restore and parity checks

Use one isolated recovery host and one private wallet home. Restore the MDK
wallet and the Spark SDK in that same single-writer window, but inspect each
rail separately.

For MDK, mnemonic-only recovery is **not** send-readiness evidence. Historical
MDK behavior could show a balance while lacking the channel monitor or
outbound-capacity state required to send. Require the supported restore/sync
path, original durable wallet state when available, and a positive outbound
readiness result. If that evidence is unavailable, stop and record
`mnemonic_restore_not_send_ready`; do not probe repeatedly with live sends.

For Spark, initialize from the preserved treasury mnemonic on mainnet, use a
fresh private storage directory, complete SDK synchronization, and require the
same stable wallet fingerprint on two consecutive reads. Do not create a new
mnemonic, register a new identity, or interpret an empty first read as proof
that the historical wallet was empty.

Before any send, require all of the following:

- network, wallet fingerprint, and recovery-source refs match the retirement
  record;
- two consecutive synchronized balance reads agree for each rail;
- transaction-history counts and redacted digests are stable across those
  reads;
- MDK reports usable outbound capacity, not merely a positive balance;
- Spark reports a bounded maximum sendable amount;
- no new production ledger or wallet activity has appeared since the quiet
  snapshot; and
- the proposed amount leaves the fee/reserve margin required by the wallet.

A parity mismatch, unknown wallet home, absent history, changing balance, or
second live writer is a hard stop. Preserve the evidence and return to owner
review.

## Owner-controlled sweep

The owner confirms the destination in the receiving wallet UI, then approves
one rail at a time. Prefer the wallet's supported sweep/send-all operation; if
it requires an amount, compute it from the freshly synchronized maximum
sendable value, not from the `9,274 sats` observation.

1. Sweep the smaller Spark rail first. Wait for a terminal payment result and
   verify the destination received it before touching MDK.
2. Re-synchronize Spark and record a redacted terminal receipt plus the
   remaining-balance bucket.
3. Sweep MDK once, only after its outbound-readiness gate passes. Do not retry
   an unknown result. Reconcile by payment history and destination receipt
   first; retry only an identical idempotent operation after proving the first
   attempt did not settle.
4. Re-synchronize MDK and verify that the expected remainder is fee/reserve
   dust or zero. Any material remainder stays in recovery review.

Receipts may contain rail, amount, fee, timestamps, transaction-status class,
hashed operation/payment refs, destination-receipt ref, tool version, and
before/after balance buckets. They must not contain a mnemonic, access token,
raw destination, invoice, preimage, payment hash, provider payload, wallet
path, SDK database, or daemon log.

## Teardown

After both rails reconcile:

1. stop the recovery process and verify no wallet process remains;
2. securely remove its private wallet home, SDK storage, environment, and
   temporary receipts after the encrypted evidence copy is verified;
3. revoke the recovery identity's Secret Manager access;
4. disable the preserved secret versions only after the owner confirms the
   destination receipts and the encrypted recovery record is readable;
5. retain only redacted receipt refs in the VP-1 closeout; and
6. prove the retired Cloud Run/Cloudflare services and routes cannot restart or
   regain secret access.

Do not destroy the last recovery material merely because a send was attempted.
Secret destruction is the final owner action after terminal settlement,
archive verification, and an explicit no-recovery-needed sign-off.
