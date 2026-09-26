# Nostr implementation notes

[`crates/nostr`](../../crates/nostr/src/lib.rs) provides pure protocol parsing,
signature verification, encryption, and contract checks. It has no storage or
network transport. [`crates/nostr-relay`](../../crates/nostr-relay/) supplies
Postgres persistence and relay behavior; client and host crates own their
separate authority and execution rules.

| Document | Scope |
| --- | --- |
| [Cryptographic primitives](crypto-primitives.md) | Differential tests, official vectors, property checks, and the retained Miri limits for NIP-44. |
| [Protocol implementation index](../protocol/README.md) | All three NIP lanes, implementation coverage, application limits, and retained verification. |
| [Normative NIPs](../../nips/README.md) | Exact upstream pins and the authored OpenAgents contracts. |
| [Relay operations](../deployment/README.md) | Configuration, database, deployment, and import. |
| [Nostr contributor guide](../../.agents/skills/nostr/SKILL.md) | Protocol flow and repository ownership boundaries. |

Primitive conformance does not establish application privacy, replay safety,
remote authority, or correct execution. Verify the consuming role as well as
the signature and encrypted envelope. Historical test commands in the crypto
record describe that run; [current verification policy](../verification.md)
determines the checks for a new change.
