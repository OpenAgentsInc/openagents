# Chapter 104: the treasury over a Coax relay, and recovery with Bitcoin Core

Run on 2026-11-05 on the Mac (Apple M5), headless, everything on 127.0.0.1.

## What ran

`coax/tests/nostr/treasury_regtest.py` with:

- Bitcoin Core 32.1 (`bitcoin-32.1-arm64-apple-darwin.tar.gz`, SHA-256
  `cd3dcdacb4ff861e862b2d7df4786ecef66472e2468dca1673a70dd761e064e7`, equal to
  the release's `SHA256SUMS`), in regtest;
- the Coax relay from `coax/relay/build.sh`, started empty;
- `nostr/tools/treasury.coax` built with `coax/web/tool.sh`: three key agents
  and a coordinator.

The steps and their results are in [treasury-regtest.json](treasury-regtest.json)
and [treasury-regtest.log](treasury-regtest.log):

1. Core funds the treasury's receive 0 and receive 1 with 60,000 and 40,000 sat.
2. The holder writes two signing requests. Each states a 70,000 sat payment to
   a Core address and 29,000 sat change to change 0, fee 1,000. The second
   request's PSBT pays a different address.
3. The coordinator seals each request to all three key agents as kind 3188
   artifacts (NIP-44 v2, the artifact envelope) and publishes them through the
   Coax relay after NIP-42. Each agent opens its artifact, reads the PSBT from
   the artifact store by digest, runs `ky_sign`, and seals a receipt back. All
   three sign the first request. All three refuse the second with `outputs`.
   The coordinator reads each signed PSBT back with `ky_signed_read` and
   combines them with `ky_combine`. For the second request it has no
   signatures and exits 3.
4. Core's `testmempoolaccept` accepts the combined transaction, and
   `sendrawtransaction` and one block confirm it. Its outputs are the
   request's.
5. Recovery without the coordinator: Core funds receive 2 with 50,000 sat. A
   new Core descriptor wallet imports the descriptor from the wallet record,
   with alice's and keyco's account keys in place of their xpubs. It rescans
   and finds 79,000 sat: the receive 2 coin and the spend's change. `sendall`
   signs both with Core's own signer, and one block confirms it.

## What it shows, and what it does not

- Bitcoin Core runs the treasury's 2-of-3 witness script on the spend that the
  Coax key agents signed. Before this run, only Python and libsecp256k1 had
  checked those signatures.
- Core's descriptor wallet derives the same scripts from the descriptor alone.
  This is the independent wallet tool: it shares no code with the Coax signer.
- The run is regtest, not signet. A signet run needs signet coins, and the
  public signet faucets give them only behind a captcha or a GitHub sign-in.
  `NEEDS_OWNER.md` asks for one faucet claim to an address the treasury
  derives. The same tools then broadcast through a public signet explorer.
- The key agents and the coordinator use the chapter's test keys.
