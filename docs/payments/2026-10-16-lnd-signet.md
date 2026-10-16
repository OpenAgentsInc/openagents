# LND signet payment qualification (2026-10-16)

The #11861 research lab completed real signet payments through the existing
Rust LND SDK, x402 HTTP resource, companion HTTP Payment encoding, MCP toll,
and shared durable replay store. The implementation is on main in
`be843ff1c2` and `1080ba31b5`. Standard network admission remains the pinned
upstream mainnet/testnet profile; signet requires explicit
`NetworkPolicy::SignetResearch` at both ends and the bound
`extra.researchProfile: "openagents:signet:1"` marker.

| Purchase | Invoice and received amount (msat) | Fee (msat) | Payment hash |
| --- | ---: | ---: | --- |
| Exact HTTP | 1,001,001 | 0 | `d6938bcfe220b8d5ae2947ce4f8b3902f05b1163a1580b78bb33808e280979dd` |
| Companion Payment | 2,000,000 | 0 | `c14d4984112673ceb792e1d35d8c006be5b7b1839e05cfb7eb7d38c312f4724a` |
| MCP tool | 1,000,000 | 0 | `0607d855e259f4c797be40da27badc2436fdbd8455016e20cc0eede3f69edbab` |

Both fresh LND 0.21.1-beta nodes ran on the isolated GCP research builder
with loopback-only RPC. Their confirmed, private 20,000-sat anchor channel
was `4b811c5c24fc24ab208ffbd0bfccf0d65b17035f56733daa659c988f055a6aef:1`.
Private route hints, signed description hashes, exact msat amounts, payee,
preimage hash, receiver settlement, and fees within the 5,000-msat cap
passed. These were direct single-hop payments with zero routing fee; this
run does not measure a third-party routing service or a nonzero-fee refusal.

The program rejected the wrong TLS pin, invoice creation by the payout
credential, payment by the invoice credential, changed HTTP bodies, wrong
preimages, missing research markers, changed MCP arguments, and standard
profile admission of signet. The focused suites also cover currency,
network, payee, amount, and policy substitutions.

Each paid HTTP/MCP response was deliberately discarded on its actual socket.
The service saved exactly three executions, and replay through both HTTP
encodings and MCP returned `duplicate_settlement`. Restarting preserved the
same three payment hashes and execution count. Removing one local saved
proof exercised recovery through `TrackPayment` and reproduced the identical
proof without another send. That last check simulates loss of the local
receipt, rather than a Lightning routing outage. LND independently listed
exactly three successful payments and three settled invoices.

A consumer starting from settle index 1 received the next two actual
`SubscribeInvoices` updates. Its three hashes matched both payment lookup
and receiver settlement. Focused Nostr, x402, and LND SDK checks passed;
formatting passed. The [standalone Rust harness](../../crates/lnd-sdk/signet-lab/)
contains the exact retry and test-directory restrictions.

Initial funding was capped at 50,000 signet sats from the existing scratch
wallet. The channel closed cooperatively in transaction
`28cf3492d4733d998fa20fa9b397b1fdfefbf4453c54c4b4575031a46fbccc4a`,
confirmed at height 326599. After LND's closure confirmation count, both
wallets were drained to the existing scratch wallet. Return transactions
`c8dccc571fcfc14988842e3dd733a7b12d70466a76635ebd16b78e1bcdfdeb19` and
`005ff469596c7b748bd0e0d4498971644ee8b096c6bff0c39ed0af9f1cc8901f`
confirmed at height **326602**, returning **48,518 sats**. The 1,482-sat
difference is the measured peer-funding, channel-open, cooperative-close,
and return fees; initial funding cost another 192 sats. Both wallet
balances were zero, and both research containers stopped.

The [public evidence archive](evidence/lnd-signet-2026-10-16.tar.gz) contains
20 selected files plus a SHA-256 manifest: payment/settlement measurements,
restart and hash-recovery results, the actual stream cursor and replay,
source hashes, chain fees, and final cleanup. Archive SHA-256:
`d72c05bcaa63fd23cafecc3c5fb886473a51e507741800903fcbce66e76b89be`.
Seven recorded source hashes match the merged implementation. Invoice,
preimage, credential, and seed fields are excluded. This evidence records
the research run; it does not provide public payment preimages.

## Remote signing and fresh backup recovery (#11862)

The follow-up uses a separate seed-holding signer and a watcher with all
259 accounts marked watch-only. Their confirmed private 20,000-sat anchor
channel carried 1,001,001 msat outbound and 500,001 msat inbound. A further
1,000,000-msat payment remained unresolved and unpaid during a bounded
signer process pause, then completed under its original hash after resume
in 4.375 seconds. All three direct payments had zero routing fee. This
measures a process pause, not loss of a VM.

Both original writers then stopped. Fresh databases restored the signer's
test seed and the watcher's original public accounts, preserving their
node identity. The encrypted SCB verified. LND explicitly refused its first
application during startup; after inspecting that refusal, the next
application waited for `SERVER_ACTIVE`, chain sync, and no existing channel.
The Rust tool now checks that readiness before applying a backup.
Peer-assisted recovery broadcast
`48594d7f692ff092d2f87b1843584dc811b12ff6b67f55c21b67c68441832ce0`.
The bounded controller watches the actual closure, CSV maturity, sweeps,
and returns. No channel database was copied or restarted from an older
state. Implementation `2c2f1004b1` is on main; final recovery and cleanup
remain tracked in [#11862](https://github.com/OpenAgentsInc/openagents/issues/11862).

## Synthetic Taproot Assets (#11863)

The separate two-node signet experiment minted 1,000,000 units of
`openagents-signet-test-11863`, asset ID
`f7f99f1e88cf8a430f3566caaa2afe5b7065d7c6452c0bba4497fecfc2a68f3a`.
These units are synthetic, with no promised redemption or monetary value.
The pinned image contains litd 0.17, LND 0.21, and tapd 0.8; RPC is
loopback-only, and each node has a fresh research wallet.

| Operation | Confirmed height | Transaction | Fee (sat) |
| --- | ---: | --- | ---: |
| Mint 1,000,000 units | 326612 | `793fa5c113a0e6f083aad0695890b5e22d81d673fbdab8756bdc37a720215639` | 308 |
| Transfer 10,000 units to the second node | 326613 | `c25831aecb719fb332e46b192d54e521a88ed5d56d8e830747ab25310ea8db28` | 509 |

The receiver exported its transfer proof and tapd verified it. A separate
read-only capture verified the same private proof again and checked both
transactions, block heights, and fees through Core. Proof SHA-256 is
`bb2960542abc3571254bf4dcd18d5a00c9e77861c9a9a4f7545244cba242148d`;
the proof bytes, wallet seeds, and credentials are not published.
The [public on-chain archive](evidence/lnd-assets-onchain-signet-2026-10-16.tar.gz)
contains 11 files plus a checked manifest, including the public asset,
transfer, proof-verification result, node identities, and Core reads.
Archive SHA-256:
`5b364b332c02c6ebd0b61f701ac0bc5c0fd0627210cc2be9f4770316d304de50`.

The private asset channel and RFQ payments in both directions still wait for
previously funded scratch-wallet outputs to reach CSV maturity. No new faucet
or mainnet funds are used. The original observer and a separate cleanup
controller retain the 150,000-sat funding cap and original 72-hour deadline.
After the RFQ checks, cleanup closes the original channel cooperatively,
verifies final asset proofs, returns ordinary BTC without spending asset
carriers, and stops both research nodes. Those steps remain actual-run
requirements; the mint and on-chain transfer do not complete the issue.
