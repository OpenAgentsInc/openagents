# Chapter 104: the public signet path, built and rehearsed

Run on 2026-11-12 on coderos-4080 (Linux x86-64), headless. Test keys only
(SHA-256("chapter 104 <name>")), coins with no value.

## What this is

Chapter 104's 2-of-3 treasury spend has run in regtest and on a private
signet
([2026-11-12-nostr-ch104-possession-and-private-signet](../2026-11-12-nostr-ch104-possession-and-private-signet/README.md)).
The step left is a broadcast on the public signet. That needs coins at the
treasury's receive 0:

    tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2

Every public faucet hands them out behind a captcha, a bot check or a
sign-in. Nobody worked around those, and no coins were claimed. So **nothing
has been broadcast on the public signet**, and the chapter stays "partly
implemented".

What this record adds: the public broadcast is now one command
(`CHAIN=public-signet` in `coax/tests/nostr/treasury_regtest.py`), and every
part of it that can run without coins has run.

## The one command

After a person has claimed signet coins by hand (at least 5,000 sat) to the
address above and the payment has one confirmation, run this from the
repository root:

```sh
nostr/relay/build.sh && { coax/web/tool.sh nostr/tools/treasury.coax >/dev/null 2>&1; nix shell nixpkgs#bitcoind -c env CHAIN=public-signet OUT=$HOME/.openagents/ch104-public-signet python3 coax/tests/nostr/treasury_regtest.py; }
```

It builds the Coax relay and the treasury tool, puts Bitcoin Core on `PATH`,
and runs the harness. The harness finds the relay, the tool and `bitcoind`
itself; `BITCOIN_BIN`, `COAX_RELAY` and `TREASURY` still override them.

| Variable | Default | What |
| --- | --- | --- |
| `CHAIN` | | `public-signet` |
| `OUT` | | Where the record and the state file go. Keep it: a rerun resumes from it |
| `ESPLORA` | `https://mempool.space/signet/api` | The Esplora API |
| `FEE_SAT` | 500 | Each transaction's fee, 280 to 1,000 sat |
| `WAIT_SECONDS` | 2700 | How long to wait for each confirmation |
| `POLL_SECONDS` | 30 | Seconds between status reads |

| Exit status | Meaning |
| --- | --- |
| 0 | The spend and the recovery are both confirmed |
| 1 | A check failed; nothing further was sent |
| 3 | Receive 0 holds nothing to spend. Nothing was signed or sent |
| 4 | A transaction was broadcast and no block held it in time. Run the same command again: it waits for the same transaction and never builds another |

Expect about 20 minutes: two signet blocks. The record lands in
`$OUT/treasury-public-signet.json` and `.log`, with the txids, block heights
and `https://mempool.space/signet/tx/TXID` URLs.

The keys are published test keys, so anyone who reads this repository can
move coins at these addresses. Run the command soon after the faucet pays.

## What the command does

1. `GET /address/ADDR/utxo`. With no confirmed output it prints what is
   missing and exits 3.
2. It takes the largest confirmed output, reads its transaction
   (`GET /tx/TXID/hex`) and checks the txid, amount and script itself. It
   writes two requests for that outpoint: 70% to receive 1 and the rest,
   less the fee, to change 0. Both are the treasury's own scripts. The
   second request's PSBT pays receive 2 instead.
3. The Coax relay starts empty on 127.0.0.1. The three key agents and the
   coordinator run as in the other modes: all three must sign the first
   request and refuse the second.
4. It checks the signed transaction (below), writes it to
   `$OUT/treasury-public-signet.state.json`, and only then sends it with
   `POST /tx`. It polls `GET /tx/TXID/status` until a block holds it.
5. Recovery, with Bitcoin Core alone and no coordinator. Core runs offline
   (`-signet -connect=0 -dnsseed=0 -fixedseeds=0 -listen=0`: no peers, zero
   blocks). `descriptorprocesspsbt` takes the descriptor from the wallet
   record with alice's and keyco's account keys, and a PSBT that spends the
   confirmed spend's two outputs to receive 3. Each `witness_utxo` is read
   back through Esplora. Core signs with its own signer and finalizes; it
   needs no wallet and no scan. The same checks, broadcast and wait follow.

The coins never leave the treasury's scripts: receive 0, then receive 1 and
change 0, then receive 3.

## Checked before each broadcast, and not

Checked:

- The harness reads the transaction's bytes: version 2, locktime 0, the
  inputs are the expected outpoints, the outputs are the stated amounts and
  treasury scripts.
- The fee is at least 1 sat/vB and at most 1,000 sat (the lowest fee ceiling
  among the three signers). Every output is at least 330 sat.
- Each witness script hashes to the spent P2WSH program and is the 2-of-3
  the harness derives from the three xpubs.
- Each signature is strict DER, low S, `SIGHASH_ALL`, and verifies over the
  BIP-143 digest the harness computes, in key order (pure Python ECDSA).
- Offline Core decodes the transaction to the same txid, size and outputs
  (`decoderawtransaction`).
- Core's script interpreter accepts every input against the spent outputs,
  with the standard flags: `signrawtransactionwithkey` with no keys signs
  nothing and reports `complete` only when every input verifies. Core must
  also refuse the same transaction with an amount one satoshi off, so the
  check can fail.
- The spent transaction, read through Esplora, hashes to its txid and holds
  the amount and script signed for.

Not checked:

- That the spent output is still unspent. Only Esplora's answer says so; no
  node of ours holds the public signet's UTXO set.
- A node's mempool policy as a whole. `testmempoolaccept` needs the UTXO
  set, so it can't run offline. The broadcast is that check.
- That Esplora tells the truth about confirmations. The block heights are
  its answers; anyone can open the URLs.

## What ran

Bitcoin Core 31.1 from nixpkgs (store path
`w5jdmd9gqdrdjrpvczs2lq8k28ygc15b-bitcoind-31.1`; `bitcoind` SHA-256
`69849ac042a5ec374d1c85dee9996ba61dd8fe65d952de647b88efd987a7e9a8`), the Coax
relay from `nostr/relay/build.sh`, and `nostr/tools/treasury.coax` built
with `coax/web/tool.sh`.

### The public signet, today: exit 3

The one command above, as written. It made two GET requests to
mempool.space (the tip height and the address's unspent outputs) and
nothing else. Record:
[treasury-public-signet-today.json](treasury-public-signet-today.json),
[log](treasury-public-signet-today.log).

```text
mode: public signet; Esplora https://mempool.space/signet/api
chain tip: block 330559
Nothing to spend. The treasury's receive 0, tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2, holds nothing on the signet https://mempool.space/signet/api serves (tip 330559).
Missing: signet coins at that address. A person must claim them from a signet faucet, by hand, to tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2 (at least 5000 sat), wait for one confirmation, and run this command again.
Nothing was signed and nothing was broadcast.
exit status 3
```

### A rehearsal of the public path on a private signet: passed

```sh
nix shell nixpkgs#bitcoind -c env REHEARSE=1 CHAIN=public-signet OUT=DIR python3 coax/tests/nostr/treasury_regtest.py
```

`REHEARSE=1` starts the private signet node of `CHAIN=signet`
(`-signet -signetchallenge=51`), pays 60,000 sat to receive 0 from it, and
serves a stand-in for Esplora on 127.0.0.1. The stand-in answers the five
endpoints the public path uses (`/address/ADDR/utxo`, `/tx/TXID/hex`,
`/tx/TXID/status`, `/blocks/tip/height`, `POST /tx`) by asking that node,
and digs a block three seconds after each broadcast. The public path then
runs against that URL through the same HTTP code. The offline Core is a
second node with the public signet's parameters, zero blocks and no peers.

Record:
[treasury-public-signet-rehearsal.json](treasury-public-signet-rehearsal.json),
[log](treasury-public-signet-rehearsal.log). 121 seconds, exit 0.

| | Result |
| --- | --- |
| Funding found | 60,000 sat at receive 0, `c18e355f…2b1e:1`, block 102 |
| Request 1 | all three sign; 201 vB, fee 500 sat (2.49 sat/vB) |
| Request 2 (pays receive 2) | all three refuse, `outputs`; no transaction |
| Spend | `74d94518fefb10428fc2b8ef957e2d1438d890ef2727dab651d4c74141802e0d`, block 103; 41,650 sat to receive 1 and 17,850 sat to change 0, as the request states |
| Recovery by offline Core | `68759dee4d4788e94b3dea52ea17575c2fc747b5bbf4aa2e889b31e1a383edd5`, block 104; spends both of the spend's outputs, 59,000 sat to receive 3, 262 vB, fee 500 sat |

These txids are on a private chain that no longer exists. They show the
path works; they are not public signet transactions.

### Timeout and resume, on the stand-in

The stand-in also ran as its own process (`CHAIN=fake-esplora`, with
`FAKE_MINE_DELAY=25`), and the public path ran against it four times with
`REHEARSE=1 ESPLORA=http://127.0.0.1:PORT` and one `OUT`:

| Run | `WAIT_SECONDS` | Result |
| --- | --- | --- |
| 1 | 4 | Spend `dbd46967…0bcb` broadcast; no block in 4 s; exit 4 |
| 2 | 60 | Resumed from the state file, did not send the spend again, saw it in block 103, built and broadcast the recovery `cfbf45d9…29dc`, confirmed in block 104; exit 0 |
| 3, 4 | 60 | Both transactions already on the chain; nothing built, nothing sent; exit 0 |
| fresh `OUT` | | Receive 0 holds nothing (it is spent): exit 3 |

### The other modes

`CHAIN=regtest` (8 s) and `CHAIN=signet` (89 s) passed after the change,
with the same results as the earlier record.

## What did not run, and why

The public signet broadcast and its recovery: no coins. Everything after
step 1 of the command has run only against the stand-in. Two behaviours of
the real API are assumed from Esplora's documentation and not exercised:
`POST /tx` answers with the txid as text, and `/tx/TXID/hex` answers 404 for
a transaction it doesn't know.

## To finish the chapter

1. Claim the coins by hand and run the one command.
2. Copy `$OUT/treasury-public-signet.json` and `.log` into a new dated
   folder here.
3. Then the chapter's status can change. This record does not change it.
