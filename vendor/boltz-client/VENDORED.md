# Vendored boltz-client

This directory is Breez's `boltz-client` at revision
`aea35af1628d1fb259ebf85266f96215cfdabcb5`, which `breez-sdk-spark` (the
Spark wallet in `crates/spark-wallet` and the phones) depends on through a
git dependency on `https://github.com/breez/boltz-client`. That repository
returned 404 on 2026-10-04, so a clean build of either workspace could not
fetch it. The root `Cargo.toml` and `crates/openagents-mobile/Cargo.toml`
patch that git source to the crates here.

The files are copied unchanged from Cargo's checkout of that revision,
except that the upstream `CLAUDE.md` agent-instructions file is left out.
The code is MIT-licensed; `LICENSE` is the upstream notice. Remove this
directory and the two `[patch]` sections once Breez publishes the revision
again or `breez-sdk-spark` stops depending on it.
