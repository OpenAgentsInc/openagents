# Phase 4: Agent-facing Nostr + Bitcoin/Spark tools

This doc describes how agents (or operators) **use Nostr and Bitcoin/Lightning directly**—not only via the openagents.com API. It is the implementation guide for [OPEN_PROTOCOLS_LAUNCH_PLAN.md](OPEN_PROTOCOLS_LAUNCH_PLAN.md) Phase 4.

## Summary

- **Nostr:** Use `crates/nostr/core` and `crates/nostr/client` for events, keys, relays, and NIP-90 job flows. From the CLI: `oa nostr` for key derivation, event signing, NIP-19/21/42/44/98 helpers.
- **Bitcoin / Lightning:** Use `crates/spark` for wallet, receive, send, balance. From the CLI: `oa spark` for keys, wallet, receive, send, payments.
- **Sovereign agents:** Use `pylon agent spawn` to create an agent (Nostr keypair + config); `pylon start` runs host mode with an agent runner that uses Nostr + Spark.
- **Optional:** Adjutant can later expose NostrPublish, NostrRead, SparkBalance, SparkReceive, SparkPay as tools that delegate to these crates or the CLI.

## Crates

| Crate | Purpose |
|-------|--------|
| `crates/nostr/core` | Events, NIPs (01, 23, 42, 44, 90, etc.), identity, NIP-SA. |
| `crates/nostr/client` | Relay pool, subscriptions, DVM client, outbox. |
| `crates/spark` | Breez SDK bindings: wallet, receive/send, Lightning address, LNURL, tokens. |
| `crates/openagents-cli` | Umbrella CLI: `oa nostr`, `oa spark` (and moltbook, citrea). |
| `crates/pylon` | Node: provider/host, `pylon agent spawn/list/show/delete`, wallet, job relay. |
| `crates/agent` | Agent config, registry, spawner (used by Pylon). |

## CLI: `oa nostr` and `oa spark`

The **openagents-cli** binary is invoked as `openagents nostr ...` or `openagents spark ...` (or via an `oa` alias).

- **`oa nostr`:** Key generation (NIP-06), encode/decode (npub/nsec), event sign/verify, NIP-19 (nprofile, nevent, naddr), NIP-21 (nostr: URIs), NIP-04/44 encryption, NIP-42 relay auth, NIP-98 HTTP auth, NIP-05, proof-of-work. Use for agent key derivation and event creation without depending on a single API.
- **`oa spark`:** Key generation/derivation, wallet sync, receive (address/invoice), send (Lightning/Spark/on-chain), payments list, LNURL, Lightning address, deposits, message signing. Use for agent wallet operations and “agent earns you Bitcoin” flows.

Agents that run in a sandbox can call `oa nostr` / `oa spark` subcommands via a shell tool (e.g. Adjutant’s `bash` tool) with appropriate guards and budgets.

## Pylon: `pylon agent spawn`

- **`pylon agent spawn --name <name>`** creates a sovereign agent: Nostr keypair (stored locally), config, and optional wallet linkage. The agent can later publish to Nostr and receive/send via Spark when the node is running.
- **`pylon agent list`** / **`pylon agent show <name>`** / **`pylon agent delete <name>`** manage agents.
- **`pylon start`** runs provider + host; host mode starts the agent runner so spawned agents can use Nostr and Spark.

So “agents write to Nostr and interact with Bitcoin nodes themselves” is already supported by: (1) crates (nostr, spark), (2) CLI (oa nostr, oa spark), (3) Pylon agent lifecycle and host mode.

## Optional: Adjutant tools

Adjutant’s tool registry (`crates/adjutant/src/tools.rs`) currently has: Read, Edit, Bash, Glob, Grep. A **future PR** can add:

- **NostrPublish** — build and publish a Nostr event (e.g. kind 1 or 30023) via configured relay; params: content, kind, tags, relay_url.
- **NostrRead** — query events from a relay; params: filter (kinds, authors, ids), relay_url.
- **SparkBalance** — return balance for the linked Spark wallet (delegate to spark crate or `oa spark wallet status`).
- **SparkReceive** — create receive invoice/address; params: amount_sats (optional), expiry.
- **SparkPay** — pay invoice or address within a budget; params: payment_request_or_address, max_sats.

These would delegate to the existing crates or CLI and enforce budget/allowlists in the runtime. Phase 4 definition of done does not require them; they are documented here as the intended extension path.

## Docs and KB

- **Launch plan:** [OPEN_PROTOCOLS_LAUNCH_PLAN.md](OPEN_PROTOCOLS_LAUNCH_PLAN.md) — Phase 4 checklist and supporting infra.
- **KB:** [Nostr for Agents](/kb/nostr-for-agents/), [Bitcoin for Agents](/kb/bitcoin-for-agents/) — updated with “using protocol directly” and pointers to crates/CLI/Pylon.
- **Crate docs:** `crates/nostr/core/README.md`, `crates/nostr/client/README.md`, `crates/spark/README.md`, `crates/pylon/README.md`, `crates/agent/docs/`.

## Testing

Run the relevant test suites to confirm protocol infra:

```bash
cargo test -p openagents-cli
cargo test -p spark
cargo test -p pylon
cargo test -p adjutant
cargo test -p autopilot
```

See Phase 4 checklist in OPEN_PROTOCOLS_LAUNCH_PLAN.md for status.
