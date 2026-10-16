# Exact Lightning payments

This crate adds durable proof consumption and HTTP and MCP service bindings
to the pure invoice and payment checks in `nostr::x402`.

## Signet research

The default network policy admits only the pinned upstream mainnet and
testnet networks. For the isolated #11861 signet lab, explicitly select
`nostr::x402::NetworkPolicy::SignetResearch` on the facilitator with
`with_network_policy`, and on the payer with
`validate_challenge_with_network_policy`. HTTP `Resource` and MCP `PaidTools`
then bind `extra.researchProfile: "openagents:signet:1"`. The policy admits
only signet's genesis identifier and `lntbs` invoices, and refuses missing
or changed markers. Standard admission continues to refuse signet.

This is an OpenAgents research extension, not upstream signet support.
Use `Challenge::issue_with_network_policy` for the companion HTTP Payment
encoding on whole-sat prices; sub-sat prices remain x402 only. The same
invoice and replay store apply to both encodings. The specification is in
[NIP-X402](../../nips/openagents/NIP-X402.md#signet-research-extension).
