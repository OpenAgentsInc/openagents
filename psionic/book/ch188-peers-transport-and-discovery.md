# 188. Peers, transport and discovery

> **Status: unimplemented — design draft.** Nothing here runs yet.

Stages on different machines need encrypted connections, a way to find each other and a control path that is separate from inference traffic.

Mesh-LLM encrypts peer traffic end to end over QUIC and discovers public meshes through Nostr. This chapter builds the same capabilities in Coax and records how each connection travels.

## What this chapter covers

- **Encrypted peer connections.** Each connection records whether its path is direct or relayed, with the latency measured. A relay forwards ciphertext only.
- **Private meshes by invitation.** A private mesh is joined only with an invitation.
- **Public discovery through beacons.** Public meshes are found through Part IV's [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md) beacons. A model group advertises a head and its stages.
- **An owner control plane.** Owner commands travel on a lane separate from the inference plane.
- **Limits of encryption.** Encrypted transport protects the link. A peer that serves a request still reads it; [chapter 194](ch194-who-can-read-the-prompt.md) handles that.

## What it builds

- Planned peer transport and discovery code in Coax, on the Nostr code in `coax/lib/nostr`.

## Dependencies

- [Chapter 24](ch024-a-pylon-answers-decisions.md): a Pylon with its own key and beacon.
- [Chapter 25](ch025-the-api-fans-out.md): beacon expiry, generation counters and draining.
- [Chapter 27](ch027-a-public-mesh.md): authenticated public access.
- Joining or publishing a public mesh and the Nostr event kinds for discovery are owner decisions.

## Done means

- Two peers connect directly and through a relay, and each connection records its path and measured latency.
- A peer without an invitation cannot join a private mesh.
- A stale or expired beacon never produces a route.
- Owner control commands and inference traffic travel on separate lanes, checked by inspecting each lane's messages.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md)
