# Official NIP ledger

`crates/nostr/src/lane.rs` is the ledger for the pinned official lane. The
commit it names is the `official` entry in `nips/manifest.json`. A test
fails when a file under `nips/official/` other than `README.md` has no check.

## What a row proves

Event-shaped files are a `Shape`: the kind and one tag that occur in the
pinned text. The check signs an event with that kind and tag, accepts it,
and refuses the same event with the tag removed. That is the client builder
and verifier for that file. It does not re-state every optional field.

Files whose body says the rules moved to NIP-01 are not a second protocol.
The check requires that sentence and still verifies a signed NIP-01 event.

The other files call the function that implements them: identifiers,
bech32, encryption, search, expiration, delegation, sync, management method
names, and the browser and Android signer method lists.

## Advertisement

NIP-11 lists a numeric NIP only when the relay runs that path. 77 is listed
because `NEG-OPEN` is handled. OpenAgents profile names stay off that list
until `NOSTR_RELAY_OPENAGENTS_PROFILES` is set.

## Owner

The `nostr` and `nostr-relay` crates own these checks. Live Postgres
acceptance of the gateway remains `crates/nostr-relay/tests/gateway_postgres.rs`
and runs only against a disposable database.
