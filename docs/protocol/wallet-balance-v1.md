# Native wallet balance v1

`openagents.wallet.balance.v1` is the read-only Spark observation carried by
the OpenAgents native `wallet.balance` service. Its only codec is
`pay/wallet-balance-wire.coax`, shared by the Swift adapter's generated C
and the Coax extras module. The exact JSON fixture is
`proto/coax-native/wallet-balance-v1.json`; check it with
`coax/web/tool.sh pay/wallet-balance-profile.coax`.

The record has exactly `schema`, integer `status` and integer `sats`.
Statuses are 0 loading, 1 available, 2 missing seed, 3 locked, 4 unreadable
seed and 5 unavailable. Only status 1 carries an amount, bounded inclusively
by 0 and 2,100,000,000,000,000 sats. Every other status carries `sats: -1`.
Malformed or unknown records become unavailable. No seed, recovery words,
wallet identity, SDK error text or payment authority crosses this boundary.

`pay/wallet-balance-policy.coax` owns admission of an observation; it reuses
the existing wallet's Bitcoin supply limit. This profile grants no ability
to send, claim a deposit, generate a receive request or alter a stored seed.
