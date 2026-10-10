# Security

How we keep people's sensitive data away from everyone who shouldn't see it,
including us, and how we say plainly when we can see it. Tier A of the
vault ("only you") is implemented; the rest is design.

| Document | What it covers | Issue |
| --- | --- | --- |
| [Sensitive data vault](sensitive-data-vault.md) | Three tiers for stored data, each with who can read it: only you (client keys), only sealed hardware with your share (Intel TDX Confidential Space and attestation-bound Cloud KMS), and readable by us (KMS-logged). Also: key hierarchy, passkey and recovery slots, agent leases, crypto-shredding, logging, Connections and attachments, UI words, and milestones with test gates. | [#11240](https://github.com/OpenAgentsInc/openagents/issues/11240) |
| [Private inference](private-inference.md) | The model step: clients verify a Pylon's TEE evidence and publicly logged release, then seal jobs end to end to it over Nostr. Trust levels from evidence; our gateway sees ciphertext only. Ideas from Darkbloom and Tinfoil, our own design. | [#11241](https://github.com/OpenAgentsInc/openagents/issues/11241) |

Wire formats: [NIP-VAULT](../../nips/openagents/NIP-VAULT.md) and
[NIP-ATT](../../nips/openagents/NIP-ATT.md). Earlier analysis:
[confidential hosted inference](../decision-models/service/confidential-inference.md).
