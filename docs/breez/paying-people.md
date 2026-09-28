# Paying people: npubs, Lightning addresses, and QR codes

How the phone Wallet pays another OpenAgents user, or anyone with a Nostr
key or a Lightning address, without copying invoices around
([#9860](https://github.com/OpenAgentsInc/openagents/issues/9860)). Code:
`crates/openagents-mobile/src/payees.rs` (routing, resolution, publishing)
and `wallet.rs` (the Send flow), drawn by `bins/openagents-ios/host/App/WalletTab.swift`.

## How an npub resolves to a payable destination

An npub is paid through what its owner published under that key, never
through a server of ours:

1. **Spark address, preferred.** The owner's NIP-A3 payment targets: the
   newest kind-10133 event signed by the key, tag
   `["payto", "spark", "spark1…"]`. A Spark transfer between Spark wallets is
   free and instant.
2. **Lightning address, otherwise.** `lud16` in the owner's newest kind-0
   profile, paid as LNURL-pay (see the [wallet design](wallet-design.md#lightning-addresses-and-lnurl-shipped-on-the-phone-9859)).
3. **Neither: refused**, naming the person: "Alice hasn't published a way to
   be paid: no Spark address and no Lightning address in their Nostr
   profile."

Rules that make it safe to show:

- Only events whose ID and BIP-340 signature verify and whose `pubkey` is the
  npub count; the newest of each kind wins.
- A value is used only with the right shape: a mainnet Spark address
  (`spark1`, bech32 characters, one case) or `name@domain`. Regtest
  (`sparkrt1…`) and anything else are ignored.
- The profile's name (`display_name`, else `name`) is shown as plain text,
  at most 60 characters.
- A published address is a destination to show, not authority to pay it.
  The confirm screen names the person ("Alice (npub1abc…wxyz)"), the kind
  and address being paid, and where it came from ("Their published Spark
  address, spark1…"), then pays only on the person's tap, like any quote.
- Zaps stay separate: paying an npub is a plain Spark transfer or LNURL-pay,
  never a NIP-57 zap request, and never an x402 or labor payment.

Relays read: `relay.openagents.com` (NIP-42 authenticated as the device
key, which that relay requires), `relay.damus.io`, `nos.lol`,
`relay.primal.net`, and `purplepag.es` (public reads, no authentication).
Every relay is read at once with an 8-second bound; resolution needs at
least one answer.

`nprofile` codes are not read yet: the shared NIP-19 decoder in
`crates/nostr` keeps bech32's 90-character bound, which an `nprofile` with
relay hints exceeds.

## Publishing the phone's Spark address

Receive → **Nostr** shows this device's npub as a `nostr:` QR code and a
**Publish my Spark address** switch, off by default. Turning it on signs a
kind-10133 event with the device key carrying `["payto", "spark", <this
wallet's Spark address>]`, keeping any other `payto` targets the key already
published, and sends it to the relays above; turning it off publishes the
event without the Spark target. The screen says how many relays accepted it.
Nothing is published without that switch. The published address can be read
by anyone and linked to the npub; the switch says so.

The phone publishes no `lud16` and registers no Lightning address; that
waits for a self-hosted LNURL server with its own service issue.

## QR codes and pasted text

The Send screen's scanner and text field take BOLT11 invoices, Spark
addresses and invoices, Bitcoin addresses, BIP21 `bitcoin:` links, LNURL
codes, Lightning addresses, and `nostr:npub…` or bare `npub…`, in either
case and behind `lightning:`. `payees::classify` routes a Nostr key to
resolution; everything else goes to the SDK's `parse` as typed.

## Contacts

The Send screen offers, as one row of chips:

- **Contacts** from the SDK's contact list (Lightning addresses with names).
  After paying a Lightning address that isn't a contact yet, the result
  offers to save it with a name.
- **People paid by npub**, newest first, up to 20, kept in the wallet's
  folder (npub and name only) and cleared by a restore.

"People the owner already talks to" has no source on the phone yet: its
chats are with the owner's own computers, which are not payees. When the app
gains person-to-person messaging, those contacts join this row.

## Checked by

- `scanned_and_pasted_codes_route_by_kind` (QR payload routing),
  `an_npub_resolves_to_its_spark_address_then_its_lightning_address_or_is_refused`,
  `only_the_owners_newest_valid_events_count`,
  `publishing_replaces_only_the_spark_target`, and
  `addresses_have_their_shapes` in `payees.rs`.
- `an_npub_is_paid_at_its_published_address_after_the_person_confirms`,
  `a_lightning_address_paid_can_be_saved_as_a_contact`, and
  `the_spark_address_is_published_only_by_the_setting_and_can_be_removed` in
  `wallet.rs`.
- Live, ignored: `a_throwaway_key_publishes_and_resolves_its_spark_address_on_live_relays`.
  Run on 2026-09-28 with a throwaway key
  (`npub1lrxnnah8kylk0g3hfx73xtz7aa35cx9w8sef26nz84muwvqs4xuskq6xac`): the
  payment-target event was accepted by 4 of the 5 relays, read back and
  resolved to its Spark address, then removed on all 5. No wallet was
  opened and nothing was paid.
- The mainnet Spark-address payment between two phones moves real sats, so
  it is an owner step in the workspace's `NEEDS_OWNER.md`.
