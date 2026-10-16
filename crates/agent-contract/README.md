# Agent definitions

Pure shared types for account-owned custom agents and the NIP-AGENT private
export payload. `AgentSpec::validate` checks bounded inputs. `Profile::validate`
checks the schema and definition identity. Neither executes, authenticates,
fetches, stores, or grants permissions.

The SDK re-exports these types. Nostr's private publisher and pinned consumer
use them without depending on the HTTP client. Read
[NIP-AGENT](../../nips/openagents/NIP-AGENT.md) for authority and carriage.
