# Lightning Address

[openagents.com/settings/lightning](https://openagents.com/settings/lightning) gets you
name@openagents.com, a Lightning Address anyone can pay. The money goes to a wallet of yours,
never ours. Log in first.

## Claim a name

The page suggests a name from your display name when it's free, and checks each name as you
type. Nothing is claimed until you press CLAIM; we never pick a name for you. Names are 3 to 32
lowercase letters, digits, dots, dashes and underscores. A name that looks like a taken one
(`a1ice` for `alice`) counts as taken.

Where the money goes:

- **An address you already have.** Type it under "Send to an address I already have" (for
  example you@getalby.com) and payments go there.
- **The wallet in the OpenAgents app.** Claim from the app's wallet screen instead. The wallet
  signs that the name is its own in the background. Payments into the app's wallet aren't on
  yet: until they are, an address pointed at it refuses payments, so nothing is lost.

A name with nowhere to send money yet refuses payments until you give it somewhere.

## Limits

Each payment is 1 to 1,000,000 sats. A payer's note is up to 280 characters. You can check 60
names a minute. Every limit is on [/docs/limits](/docs/limits/index.md).

## Use it

The page shows the address, its QR code, COPY and, where the browser has one, SHARE.

TURN OFF stops payments at once; the name stays yours. TURN ON takes payments again.

## Change where the money goes

The first place you give a name takes effect at once. After that, a change waits 24 hours, needs
a fresh signature from your key or passkey, and is sent to every email and key on your account,
so a stolen login can't quietly redirect your money. You can cancel a waiting change.

## Add it to your Nostr profile

With a Nostr key extension (NIP-07) in your browser, ADD TO MY NOSTR PROFILE puts the address in
your profile (`lud16`) and makes it your Nostr name (NIP-05). The key must be one of the keys on
your account. Your extension signs the profile; we check the signature and pass it on to the
relays. We never sign your profile. If we can't read your current profile from the relays, we
don't change it.

More: [docs/auth/lightning-addresses.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/auth/lightning-addresses.md).
