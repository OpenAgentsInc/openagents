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
