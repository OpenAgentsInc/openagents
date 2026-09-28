# BIP 177 amounts in the Wallet (#9881)

Simulator (iPhone 17 Pro clone), offline fixture wallet (`--wallet-fixture 1`,
no money), built from `main` at the BIP 177 commits.

1. `01-bip177-with-note.png`: first launch. `₿250,000` with `0.00250000 BTC`
   below it, the one-time note, and the format choice.
2. `02-legacy-btc.png`: `--amount-format btc`. The balance, the deposit, its
   claim fee, history, and the amount field in legacy BTC; `₿250,000` below
   the balance.
3. `03-bip177.png`: `--amount-format bip177` after the note was read: every
   amount as `₿N`, with **Show amounts as** at the bottom.
