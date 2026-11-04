# Bitcoin mempool observations v1

`GET /api/mempool/v1` is a read-only, separately versioned projection of
locally observed unconfirmed transactions. Its source is
`proto/openagents/bitcoin/mempool/v1alpha1/mempool.proto`. It preserves the
confirmed explorer HTTP v1 contract and uses that contract's snapshot object.

Every successful object includes `snapshot`, `validation: "unvalidated"`,
`complete: false`, integer `observed_at` Unix seconds, `received`, `omitted`
and `transactions`. Each transaction contains conventional display-order
`txid` and `wtxid`, integer `fee_sat`, `weight`, `vsize` and `rate_sat_kvb`.
The rate is the fee times 1,000 divided by virtual size, rounded down.
Virtual size is serialized weight divided by four, rounded up; it does not
claim a sigop-adjusted relay-policy size.

Fees use the node's own confirmed coins and earlier observed parents.
Duplicate inputs, coinbase transactions, missing or immature inputs,
conflicts, negative fees, invalid amounts and exhausted resources omit a
candidate. The first admitted spend wins. This profile does not run
scripts, standardness, relative locks, replacement, eviction, package policy
or relay. It never labels a transaction consensus-valid or relay-accepted.
Confirmed transaction IDs are omitted. Changes to the confirmed snapshot
rebuild observations, so mined or newly conflicting transactions disappear.
Reorgs can make previously observed transactions eligible again.

A missing, malformed, wrong-network, changed-during-read or oversized feed
returns HTTP 503; it does not become an empty successful mempool. Method,
request framing, bounded response, no-store and connection behavior follow
the explorer HTTP envelope. There is no submission endpoint.

The local input is an atomically replaced `STATE.mempool` file: ASCII
`OMP1`, little-endian uint32 chain ID (the existing Coax chain ID), uint64
observation time, then zero or more uint32-length/raw-transaction pairs.
It has an eight MiB limit and at most 1,024 candidates. Candidates are
bounded at 400,000 serialized weight units. The input is private node
coordination, not a network protocol or a persisted consensus database.
