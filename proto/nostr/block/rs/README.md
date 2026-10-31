# NIP-RS read-state envelope projection

`nostr.block.rs.v1alpha1.ReadStateEvent` is the public envelope of a kind
30078 read-state event, for the CTRL and WS read-state consumers the protocol
spec names. It keeps the original signed event and projects the slot id from
its `d` tag. The NIP-44 blob (`client_id`, contexts, overrides) stays
ciphertext and is not projected; a later slice decodes it. `nostr.pb` stays
frozen; the `block-read-state` profile holds it.

`coax/lib/nostr/adapters/rs-projection.coax` is consistent only for an event
the NIP says to keep: kind 30078, exactly one `d` tag whose value is
`read-state:` and the projected 32 lowercase hex characters, and exactly one
`["t", "read-state"]`. Other tags and other `t` values are allowed (kind 30078
is shared application data). It does not verify the signature, decrypt,
detect a `client_id` conflict or merge frontiers.
