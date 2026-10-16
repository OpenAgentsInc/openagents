# LND signet payment lab

An isolated research program for #11861. It uses two fresh LND 0.21 nodes,
loopback-only RPC and HTTP, explicitly selected signet admission, private
channel route hints, and the existing x402/MCP validators and durable replay
store. It does not use a production wallet or account.

```sh
cargo build --locked --manifest-path crates/lnd-sdk/signet-lab/Cargo.toml
lnd-signet-lab "$HOME/lnd-signet-11861"
```

The directory must contain fresh research nodes `a` and `b`, with their
`tls.cert` and LND data. RPC ports are 10039 and 10049. An active private
channel must join the nodes. The program bakes separate payout and invoice
macaroons, checks pinned TLS and signet identity, and serves a local counter
on port 21861. Initial funding is capped at 50,000 signet sats for the lab;
node setup and channel cleanup are separate infrastructure operations.

The three purchases are an exact 1,001,001-msat HTTP operation, a whole-sat
2,000,000-msat companion Payment operation, and a 1,000,000-msat MCP call.
The program checks exact description hashes, receiver settlement, routing
fees below the 5,000-msat cap, role restrictions, rejected substitutions,
settlement-stream hashes, and proof replay after losing each paid response.
Both HTTP encodings and MCP share one durable proof store. These are
research counters, not a commercial Computers offer or an upstream signet
compatibility claim.

The program saves each original invoice and payment-attempt marker before
sending. On restart it tracks that hash and never sends a replacement
payment. An interrupted execution with no saved result requires inspection
of its original proof claim; restarting does not dispatch it again.
Private files use mode 0600. `public-result.json` contains only hashes,
amounts, fees, and check results; never publish the whole data directory.
