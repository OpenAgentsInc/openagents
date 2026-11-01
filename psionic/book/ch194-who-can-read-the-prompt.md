# 194. Who can read the prompt

> **Status: unimplemented — design draft.** Nothing here runs yet.

Encryption protects the link, not the request from the peer that serves it. This chapter computes a trust level for each machine from evidence and routes requests by how sensitive they are.

The levels come from the earlier private-inference design and the review of Darkbloom: any peer, a hardened machine and an attested enclave running a logged release.

## What this chapter covers

- **A peer reads what it serves.** Any peer that serves a request can read it.
- **Hardened is not private.** A hardened process on a machine its owner controls is harder to snoop on, but root can read its memory.
- **Only an attested enclave is private.** An attested hardware enclave running a logged release is the only private level. Attestation says which code ran, not that the answer is right. See [NIP-ATT](../../nostr/nips/openagents/NIP-ATT.md).
- **A plaintext middle breaks the claim.** If a coordinator decrypts a request to route or bill it, no downstream privacy claim holds.
- **Routing by sensitivity.** Each request declares a sensitivity. It routes only to machines at the required trust level and fails closed rather than falling back.
- **Weight checks fail closed.** A weight-hash mismatch stops serving.

## What it builds

- A planned trust-level computation and routing rule in an `#authority` Coax file.

## Dependencies

- [Chapter 188](ch188-peers-transport-and-discovery.md): peers and transport.
- [Chapter 189](ch189-place-the-model.md): placement.
- [Chapter 27](ch027-a-public-mesh.md): the trust scope of the public mesh.

## Done means

- A request that requires the private level never reaches a machine without a valid attestation, and fails closed when none is available.
- Each trust level is computed only from recorded evidence. A test removes each piece of evidence and the level drops.
- No coordinator path decrypts a request that is marked private.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [NIP-ATT](../../nostr/nips/openagents/NIP-ATT.md)
