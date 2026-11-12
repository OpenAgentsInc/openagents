# openagents.payment.v1alpha1

The Lightning Address records (#13208): `LightningAddress` (one per person
account) and `AddressChange` (a change of destination that waits 24 hours, 72
for an email-only account). They mirror `pay.addresses` and
`pay.address_changes` (`coax/web/api/migrations/0149_pay_lightning_addresses.sql`)
and the card at `GET /api/lightning/address`. The wallets' wire is LNURL JSON
(LUD-06/16/12/09/18/21), served by `coax/web/api/lnaddr.c` and checked in
`coax/lib/lnaddr/lnurl.coax`.

The descriptor is `proto/descriptors/lightning-address.pb`; its generated Coax
codec is imported by `coax/lib/lnaddr/records.coax`, whose checks read protoc-written
bytes of both records. Alpha: field numbers may change until Spark mode ships.
