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

## Synthetic signet assets

`lnd-signet-assets ROOT inspect|exercise` is the #11863 research program.
It requires `$HOME/lnd-assets-signet-11863`, original recorded node identities,
pinned local TLS, two integrated litd 0.17 / LND 0.21 / tapd 0.8 instances,
and a confirmed asset named `openagents-signet-test-11863`. This synthetic
asset has no peg or monetary value. The setup keeps RPC, the mock test oracle,
and proof delivery on loopback; all mainnet approval flags stay false.

After on-chain mint/transfer and private asset-channel preparation,
`exercise` pays 10,000 units to node B and 5,000 back to A through the existing
Rust `Assets` implementation. Each direction checks the frozen RFQ rate,
actual asset HTLC details, exact receiver units, original-hash payment
lookup, and a reopened ledger's single credit. The action book preserves
original quotes and uncertain sends across restarts. Only the selected
public payment measurements are printed; credentials and preimages remain
private. Infrastructure must retain confirmed chain/proof evidence, close
the channel, return BTC, and stop the two test nodes before #11863 closes.

`scripts/lnd-signet-assets-setup.py` creates the two fresh containers once on
the research builder. Its writable home and SQLite temporary directory stay
under the isolated data directory while the container root filesystem stays
read-only. `scripts/lnd-signet-assets-observe.py --preflight` checks the live
nodes without funding or minting. Without that flag, the 72-hour controller
watches the initial 5,000-sat funding, saves each mint/transfer attempt,
checks the confirmed receiver's exact 10,000 units and exported proof, then
waits for the existing scratch recoveries to supply channel liquidity.

Pinned tapd 0.8 fixes an asset channel's BTC value at 100,000 sats. The total
research funding cap is therefore 150,000 recycled signet sats: 5,000
initially, then 120,000 to A and 20,000 to B only when available. A low-fee
initial deposit can use a separately recorded CPFP with a 1,500-sat budget;
the controller then waits for at least 3,000 sats of confirmed change from
that same deposit. It never creates a replacement deposit. No faucet
requests are made. The signet funding helper exists only with the SDK's
`supervisor` feature and rejects every other network. The controller waits
for confirmed reserves, the original private channel, and graph readiness
before running the RFQ checks. It stops with `cleanup_pending` after those
checks; closing the channel, preserving the final asset proofs, and returning
BTC remain explicit steps in this research issue.
