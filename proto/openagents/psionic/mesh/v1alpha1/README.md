# Private mesh replay witness

`openagents.psionic.mesh-witness.v1` is an alpha protobuf carriage for the
memory-only checker pipe, capped at 262144 bytes. It never appears in public
Nostr events, logs, stdout or regular files. Two 76-byte NIP44 message-opening
keys permit authentication of only the original signed job/result ciphertexts;
no buyer signing key or reusable conversation key crosses this boundary.

Presence, key lengths, exact digest/event/recipient/request/attempt/model
binding, private descriptor kind, sampling and comparison policy are Coax
admission decisions. Generated codecs do not establish those facts.

`PublicPoolInputs` is a separate **public-only** projection preserving exact
original signed aggregate/input JSON and the public JCS policy. It never
contains a private witness. `CheckerObservation` names the precise private
inconclusive envelope whose digest is published when no replay result exists;
it is not a model result.

The public bundle transport ceiling is 2 MiB. The current generated Coax wire
runtime has a stricter 1 MiB implementation ceiling; the native reader/writer
refuse larger messages and never truncate them.

`CheckerObservation` projects the named JCS observation: `version` maps to
`v`, `receipt_event_id` to lowercase-hex `receipt`, `comparison_profile` to
`profile`, and present `reference_null` to JSON null. Its reason is the integer
code defined by the NIP-PYLON profile. The public label commits the exact JCS
bytes; protobuf serialization is not substituted for that digest.
