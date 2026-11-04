# 82. Mempool acceptance and limits

> **Status: partly implemented.** Native CPU, unvalidated explorer observations.
Full Core acceptance parity, script execution, finality, relative locks,
standardness, replacement, package limits, eviction and relay remain planned.

The fast-path explorer needs to show unconfirmed transactions and their fee
rates without claiming the validation chapters have landed. This profile
tracks bounded local observations against our own confirmed coin state and
observed parents. The confirmed address balance remains confirmed-only.

## Keep the contract explicit

The new [mempool projection](../../docs/protocol/bitcoin-mempool-v1.md)
and [protobuf source](../../proto/openagents/bitcoin/mempool/v1alpha1/mempool.proto)
have their own identity. They preserve the published confirmed explorer API.
Every reply labels validation as unvalidated and completeness as false.
The observation time, received count and omitted count are visible.

An observed transaction needs a canonical parse, nonempty inputs and outputs,
no null or duplicate inputs, and at most 400,000 serialized weight units.
Each input resolves to an own-node confirmed coin or an earlier admitted
parent output. Already observed spends conflict; the first admitted spend
wins. Confirmed IDs, missing inputs, immature coinbases, bad amounts,
negative fees and exhausted resources are omitted.

The fee reuses [chapter 40](ch040-weight-vsize-and-fees.md): input values
minus output values within MoneyRange. Virtual size is serialized weight
rounded up to virtual bytes, not a claim of Core's sigop-adjusted policy size.
Rates are integer satoshis per 1,000 virtual bytes, rounded down.

## Prepare a whole observation

`mp_add_vartime` resolves every input, computes the fee and builds the whole
batch before applying it. Its private ephemeral table uses `T + txid` for
duplicates, `S + outpoint` for conflicts, and `C + outpoint` for parent coins.
A refusal leaves that table unchanged. These keys are never durable chain
state. The native adapter excludes already-confirmed IDs before addition.

The [worked program](../examples/mempool-observations.coax) shows the real
P2WPKH example's 5,806-sat fee, duplicate refusal and a child's 78-sat fee.
Its [exact output](../fixtures/mempool-observations.out) is retained.

## Observe a peer and follow the chain

The [Coax peer collector](../examples/live/mempool-peer.coax) negotiates
transaction announcements, requests witness transaction bodies, verifies
frame checksums and matches returned txids to requests. It handles fragmented
frames, ping/pong and duplicate announcements. It atomically replaces a
bounded local feed; it neither submits nor relays transactions:

```shell
coax/web/tool.sh bitcoin/examples/live/mempool-peer.coax signet HOST PORT UNIX_SECONDS STATE.mempool 660
```

This records new announcements during a bounded interval; it does not ask a
peer for its whole mempool. The file holds at most eight MiB and 1,024
candidates. The native service retries missing parents for at most 32
dependency passes and counts remaining omissions. A missing or malformed
feed is unavailable data, not a successful empty mempool.

The node rebuilds observations after a feed replacement or a confirmed tip
change. Mining removes confirmed IDs. A conflicting branch can remove a
descendant, and a return reorg can make the same observed descendant eligible
again. The [explorer](../explorer/README.md) exposes the MEMPOOL page, exact
fees and rates, full copyable txids, observation counts and desktop/mobile
layout. It refreshes even when the confirmed hash is unchanged.

## Check independently

The library and feed root gates kill 238 and 57 mutants respectively, with
zero survivors and no equivalents marked. Checks cover exact identifiers,
parent coins, maturity, conflicts, multiple inputs, null and duplicate
inputs, all capacity refusals and unchanged state on failure.

The [independent oracle](../../coax/tests/bitcoin/mempool.py) records three
Core-validated dependent regtest transactions, then stops Core. Its peer
supplies fragmented and duplicate announcements, an unsolicited transaction
and a corrupt frame. Native API comparisons and one open browser follow
the observations through mining and both reorgs. See the
[verification record](../../docs/verification/bitcoin-explorer-mempool-2026-11-03.md).
