# LND SDK

Typed Rust access to LND for server-side Lightning. This crate adds an
experimental backend; it changes no production receiver or payout setting.
See [the phased plan](../../docs/payments/2026-10-15-lnd-sdk-plan.md).

`Client` uses SHA-256 certificate pinning, per-role macaroons, and the LND
0.21 version gate. Idempotent reads retry briefly. Payment sends and invoice
creation never retry after an uncertain response. Recover a payment by its
original hash. The blocking `service::Wallet` adapter owns a dedicated runtime;
`service::Rails` uses it for Lightning, including durable Spark refill
invoices. Address resolution and Spark transfers use the existing provider.

`settlements` resumes from the consumer's saved settlement index and catches
up on old invoices on first use. A `SettlementSink` must commit each effect
and its index in one transaction. This is required for exactly-once effects.
For private peers without a public graph entry, operations callers can read
`private_route_hints` and supply them to `create_invoice_with_hints`.

## Build and check

```sh
cargo build -p lnd-sdk
cargo test -p lnd-sdk
scripts/lnd-sdk-regtest.sh
```

Normal builds need no `protoc`. Tests regenerate the RPC types and compare
them with the checked-in files; they need `protoc`. To update those files,
run `scripts/lnd-sdk-proto.sh`. Source hashes and upstream licenses live in
`proto/`. The pinned releases are LND `v0.21.1-beta`, Taproot Assets `v0.8.0`,
Loop `v0.34.0-beta`, and Lightning Terminal `v0.17.0-alpha`.

The Unix-only `supervisor` feature starts isolated Docker fixtures with fresh
regtest wallets, scoped volumes, and dynamic local ports. An unfunded offline
testnet signer checks the existing x402 validator without admitting regtest
invoices. The fixture removes its containers and volumes when it exits. It
never opens a production wallet or reads a treasury secret.

## Buy inbound capacity

`lnd-liquidity SETTINGS ACTION` supports `info`, `plan`, `order`, `status`,
`pay`, `guard`, `open`, and `costs`. Start with
[`liquidity.example.json`](../../deploy/lnd/liquidity.example.json).
The example has no spending budget. `info` and `plan` only read the provider.
The owner must choose the capital, LSP fee limit, routing fee limit, peer,
fee rate, and approval before an order, payment, or channel open.

Run `guard` in a separate process throughout an LSPS1 purchase: it accepts
only the selected provider's anchor channels at the requested capacity, with
at least three confirmations. Connect that provider before `pay`. Run `open`
for the separately approved stable peer; it requests anchors and confirmed
inputs, and checks the remaining reserve before opening. The opening
allowance is a conservative balance check, not a transaction fee ceiling.
Check the actual fee and reserve afterward. Pause funding if the allowance
is insufficient. No command retries an uncertain mutation.

Keep one protected journal per node and approved budget. After an interrupted
`pay`, repeat `pay` only to track the saved hash. After an interrupted `order`
or `open`, inspect provider orders or LND pending channels and resolve the
original action; do not delete the file or make another journal. `costs`
saves actual purchase, routing, and opening costs and compares them with 2%
of settled inbound payments. An empty receipt history has no percentage.
Capital and the anchor reserve are not operating costs.

Provider endpoints and identities come from [ZEUS](https://docs.zeusln.app/lsp/services/lsps1/)
and [Megalith](https://docs.megalithic.me/lightning-services/lsp1-get-inbound-liquidity-for-mobile-clients/).
Requests follow [LSPS1](https://github.com/lightning/blips/blob/master/blip-0051.md).
Always fetch current limits; the provider examples can lag its live service.


## Wallet setup

`transport::wallet_unlocker` connects the generated WalletUnlocker client
with the same TLS certificate pin as the initialized client. It sends no
macaroon and performs no RPC. Wallet initialization callers must set a
deadline, save private recovery material before initialization, and inspect
an uncertain result before retrying. After initialization, use `Client::connect`, then compare the network and
node ID with the intended wallet before funding it.
