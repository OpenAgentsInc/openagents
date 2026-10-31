# NIP-PL push-lease envelope projection

`nostr.block.pl.v1alpha1.PushLease` is the public envelope of a kind 30350
push lease, for Coder host push leases (the consumer the protocol spec names).
It keeps the original signed event and projects `d` (the installation),
`expiration`, `exec` (the executor key id) and the optional `alt`. The NIP-44
content (origin, transport, endpoint, subscriptions) stays ciphertext and is
not projected; a later slice decodes the executor's plaintext. `nostr.pb`
stays frozen; the `block-push-lease` profile holds it.

`coax/lib/nostr/adapters/pl-projection.coax` checks the projection against the
original event: kind 30350, exactly one `d` (1 to 64 bytes), one `expiration`
in canonical unsigned decimal, one `exec` and at most one `alt`, each with
exactly one value, and no other or empty tag. It does not verify the
signature, check the expiration against a clock, `allowed_skew` or
`max_lease_ttl`, look up the executor key, decrypt or accept the lease.
