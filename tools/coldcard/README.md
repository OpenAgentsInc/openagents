# Coldcard sweep-fingerprint tooling

Independent OpenAgents implementation of the transaction-builder fingerprint
described in the Coldcard RNG postmortem. Reimplemented from the documented
rule, not copied from the reference tool, so a disagreement between the two is
meaningful evidence rather than a shared bug.

Results and interpretation:
[`docs/coldcard/2026-08-01-bitcoin-node-forensic-capability.md`](../../docs/coldcard/2026-08-01-bitcoin-node-forensic-capability.md).

## What it does

`cc-fingerprint-scan.py` walks a block range on a local unpruned Bitcoin Core
node and records every non-coinbase transaction where

- the fee is an exact whole number of satoshis per vbyte of an **estimated**
  size (`42 + 68|91|148` per input by prevout script type), and
- `nLockTime == 0`.

It also records the 33-byte-R signature share, input/output shape, script
types, and the estimate-versus-real vsize delta, so the supporting marks can be
applied afterwards instead of being baked into the match.

`cc-fingerprint-stratify.py` reads the outputs and reports false-positive rates
per million eligible transactions by era and fee rate, the effect of each
additional mark, block clustering, and the overshoot distribution.

## Requirements

A **full, unpruned** Bitcoin Core node reachable by `bitcoin-cli`. Block
scanning uses `getblock <hash> 3`, which supplies prevout values directly, so a
transaction index is not required for the scan itself. Amounts are parsed as
`Decimal` and converted to integer satoshis; no float arithmetic touches a
monetary value.

## Use

```sh
# self-test only — validates the eight published known positives
CC_SELFTEST_ONLY=1 python3 cc-fingerprint-scan.py 960367 960367 selftest

# scan a range (self-test runs first unless CC_SELFTEST=0)
python3 cc-fingerprint-scan.py 960180 960599 incident

# stratify whatever has been collected
python3 cc-fingerprint-stratify.py
```

Outputs are append-only and resumable: `cc-hits-<tag>.jsonl`,
`cc-stats-<tag>.jsonl` (the eligibility denominator), and `cc-done-<tag>.txt`
(completed heights with block-hash checkpoints). Re-running skips finished
blocks. Set `CC_OUT` to change the output directory.

A missing prevout raises rather than being skipped, so an incomplete scan can
never masquerade as a clean zero-hit result.

## Reading a result

The exact-integer test alone matches about 2.2% of all transactions. It is not
by itself evidence of anything. Fee rate, transaction shape, clustering, and
value have to be applied together, and even then a match establishes
**program similarity only** — never a person, an intent, a theft, or a link to
any particular wallet vendor.
