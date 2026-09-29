# A simpler Wallet screen

Simulator (a scratch iPhone 17 Pro, iOS 26.5), offline fixture wallet
(`--tab wallet --wallet-fixture 1`, debug build, no money).

1. `before-01-wallet.png`: the Wallet on `main` before the change: the
   network label, two balances, a refresh button, Receive/Send/Buy and
   Lightning/Spark/Bitcoin/Nostr segments, deposits, history, and settings
   on one page.
2. `after-01-wallet.png`: the new main screen: one balance, the **Back up
   your wallet** card, **Receive** and **Send**, **Recent activity**, and
   **Advanced** (closed, noting "A deposit needs you").
3. `after-02-receive.png`: `--wallet-section receive`: a request for any
   amount appears at once with its QR code, Copy, and Share.
4. `after-03-send-confirm.png`: `--wallet-send alice@example.com
   --wallet-amount 1000`: the plain confirm screen.
5. `after-04-advanced.png`: `--wallet-advanced 1`: Advanced open, with the
   balance in BTC, the network, **Other ways to receive**, and **Buy
   bitcoin** (deposits, people, agent payments, the amount unit, recovery,
   and the exit backup follow).
6. `after-05-info.png`: `--wallet-info 1`: the trust note, one plain
   paragraph first and the details below.
