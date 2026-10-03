# `openagents wallet`

The wallet on a computer is the same Spark wallet as the OpenAgents app on
the phone: one balance on every device, Bitcoin only. The code is
`crates/openagents-cli/src/wallet.rs` over `crates/spark-wallet`; the seed
lives in `~/.openagents/spark` (`OPENAGENTS_SPARK_HOME` moves it), and
`openagents doctor` shows that path as `wallet`.

## Getting a wallet on a computer

```sh
openagents wallet create              # a new wallet; shows 12 recovery words once
openagents wallet link                # use your phone's wallet (approve on the phone)
openagents wallet restore             # type your recovery words (they don't show)
```

`create` is for a person or agent without the phone app. On a terminal it
shows the words and waits until you type `saved`; nothing is kept if you
don't. Off a terminal, or with `--json`, it refuses unless `--show-words`
asks for the words to be printed, since anyone who reads them can spend.

## Using it

```sh
openagents wallet balance             # "Your balance is ₿12,000 (0.00012000 BTC)."
openagents wallet address             # a Spark address to get paid at
openagents wallet receive --amount 1000
openagents wallet receive --bitcoin   # a Bitcoin address instead
openagents wallet send TO --amount 1000   # asks first; --yes pays without asking
openagents wallet history             # newest first, with the time (UTC) and fees
```

Amounts are whole sats, written ₿1,000 (BIP 177). Set
`OPENAGENTS_AMOUNT_FORMAT=btc` to show BTC first and type amounts in BTC.
`send` refuses before asking when the balance can't cover the amount and its
fee, and refuses the wallet's own address.

## Paying for calls

`openagents x402 fetch`, `call`, and `buy` pay from this wallet. `--pay-with
node` pays from this computer's own Lightning node (`openagents x402 node`)
instead, and `--pay-with phone` asks the phone to approve. Selling from this
computer (`x402 serve`) needs that node; `x402 publish` sells through
OpenAgents with no node.
