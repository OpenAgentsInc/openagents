# LND SDK

Typed Rust access to LND for server-side Lightning. This crate adds an
experimental backend; it changes no production receiver or payout setting.
See [the phased plan](../../docs/payments/2026-10-15-lnd-sdk-plan.md).

`Client` uses SHA-256 certificate pinning, per-role macaroons, and the LND
0.21 version gate. Idempotent reads retry briefly. Payment sends and invoice
creation never retry after an uncertain response. Recover a payment by its
original hash. The blocking `service::Wallet` adapter owns a dedicated runtime;
`service::Rails` uses it for Lightning and delegates address resolution and
Spark operations to the existing provider.

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
