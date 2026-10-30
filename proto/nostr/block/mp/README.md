# NIP-MP project projection

`nostr.block.mp.v1alpha1` is a source-preserving representation for the
existing project-page consumer. It stores the original signed event alongside
its candidate `d`, metadata and ordered member coordinate/hint projections.
It is not a replacement for Nostr event JSON and does not sign, verify or
admit an event. No live page uses this profile yet.

The project page's existing consumers are `coax/web/project/model.ts`,
`coax/web/project/list.ts` and `apps/openagents-web/project-live.coax`. The
separate `block-project` descriptor profile is for later migration of those
consumers; published `nostr.pb` and previous profile descriptors stay frozen.

Protobuf's presence distinguishes absent metadata and relay hints from empty
strings. Ordered and duplicate original tags remain in the signed event's raw
JSON and event projection. `members` is bounded to the source's 64 tags;
metadata bounds come from the pinned NIP. The codec does not enforce source
identity, exact tag arity/cardinality, coordinate grammar or uniqueness, the
nonempty `d` rule, listing/fold semantics, deletion or current membership.
Clients must compare any projection with the original before use and apply
those rules separately. A project signer gains no authority over a member
repository merely by naming it. Relay hints are advisory and never authority.

The message's `original` contains the existing shared Event projection and its
original JSON; preserve those bytes for NIP-01 signing and verification. Do not
publish credentials or attach trust flags to the data. The test in
`coax/tests/nostr/block_project.py` uses pinned source examples, protoc and
actual generated Coax readers/writers on native sanitizers and Wasm; it is
codec interoperability evidence, not NIP-MP or security conformance.
