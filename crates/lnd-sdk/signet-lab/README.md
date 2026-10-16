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

## Remote signer and recovery

The separate `lnd-signet-recovery` binary qualifies #11862 with fresh research
nodes `signer`, `watch`, and `peer`, RPC ports 10059, 10069, and 10079, and an
explicit `$HOME/lnd-signet-recovery-11862` root. The watcher mounts only its
own data, its password, and the signer's TLS certificate and scoped macaroon.
It imports all 259 public accounts, with no seed or signer database access.

`init-signer`, `init-peer`, and `init-watch` initialize the test wallets.
`inspect` checks signet, supported LND versions, original node identities,
and watch-only account flags. Once the private 20,000-sat anchor channel is
confirmed and active, `exercise` saves and verifies its encrypted backup,
pays exact amounts in both directions, and pauses the signer for one payment.
An independent eight-second guard resumes the signer. The test requires an
unresolved payment and an unpaid receiver during the pause, then tracks the
original hash to completion. It never sends a replacement payment.

Recovery infrastructure first stops both original writers and retains their
directories. Fresh replacements copy configuration and TLS files only.
`restore-signer` imports the saved test seed; `restore-watch` imports the
original public accounts. After both replacements finish starting,
`restore-backup` applies the verified original SCB. An interrupted mutation
requires inspection of its saved marker and the original node, rather than
an automatic retry. Never restart the old watcher after this step.

`scripts/lnd-signet-recovery-observe.py` watches this exact channel for up to
72 hours. It waits for peer-assisted closure, actual CSV maturity, all sweeps,
and confirmed outputs before returning coins from the watcher and peer to
the recorded scratch wallet. The signer holds the same wallet keys as the
watcher, so it never sends a second return. The controller records each
attempt before sending, waits for the original return transaction, checks
all three balances are zero, and stops only these test containers. A missing
result after a recorded send stops for inspection.
