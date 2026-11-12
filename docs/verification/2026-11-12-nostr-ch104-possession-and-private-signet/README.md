# Chapter 104: possession proofs, and the treasury on a private signet

Run on 2026-11-12 on coderos-4080 (Linux x86-64), headless, everything on
127.0.0.1. Test keys only (SHA-256("chapter 104 <name>")), coins with no
value.

## What changed since the 2026-11-05 run

Each enrollment's `possession` field is now a real BIP-322 signature, and
admission verifies it (`coax/lib/nostr/nipkeys_possess.coax`). The proofs
change each enrollment's digest, so the wallet record's digest and the
request's wallet reference change too. The keys, the descriptor, the scripts
and the signing code are the same. This run repeats the
[2026-11-05 network run](../2026-11-05-nostr-ch104-treasury-network/README.md)
with the new records, twice: in regtest, and on a private signet.

## What ran

`coax/tests/nostr/treasury_regtest.py`, with `CHAIN=signet` for the second
run, and:

- Bitcoin Core 31.1 from nixpkgs (`nix build nixpkgs#bitcoind`, store path
  `w5jdmd9gqdrdjrpvczs2lq8k28ygc15b-bitcoind-31.1`; `bitcoind` SHA-256
  `69849ac042a5ec374d1c85dee9996ba61dd8fe65d952de647b88efd987a7e9a8`);
- the Coax relay from `coax/relay/build.sh`, started empty;
- `nostr/tools/treasury.coax` built with `coax/web/tool.sh`: three key agents
  and a coordinator.

The steps are the 2026-11-05 record's. Results:

| | regtest | private signet |
| --- | --- | --- |
| Record | [treasury-regtest.json](treasury-regtest.json), [log](treasury-regtest.log) | [treasury-signet.json](treasury-signet.json), [log](treasury-signet.log) |
| Core started with | `-regtest` | `-signet -signetchallenge=51` |
| Request 1 | all three sign; 675-byte transaction | the same |
| Request 2 (another payee) | all three refuse, `outputs`; no transaction | the same |
| `testmempoolaccept` | allowed | allowed |
| Spend | confirmed, outputs are the request's | confirmed, outputs are the request's |
| Recovery with Core's descriptor wallet | 79,000 sat found and sent, confirmed | the same |
| Time | 9 s | 104 s (signet's proof of work, 105 blocks) |

## What the private signet is, and is not

Signet (BIP-325) is a chain whose blocks must satisfy a challenge script.
`-signetchallenge=51` makes that script `OP_TRUE`, so this node signs its own
blocks. It is a signet in every other way: signet's proof-of-work limit and
signet's address prefix, `tb1`, which is what the wallet record's
`network: signet` means. In regtest the harness had to write the treasury's
scripts as `bcrt1` addresses for Core. So this run adds one thing to the
regtest one: Core funds, spends to and recovers from the treasury's own `tb1`
addresses. The script rules and the mempool's standardness rules are the same
on both chains.

It is not the public signet. The node has no peers and nothing was broadcast
to anyone. A spend on the public signet still needs coins from a faucet,
which hand them out behind a captcha or a sign-in. The treasury's receive 0
is `tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2`.
