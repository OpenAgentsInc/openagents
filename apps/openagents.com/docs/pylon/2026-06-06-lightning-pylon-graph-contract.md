# Lightning/Pylon Graph Contract

Issue #357 / `OPENAGENTS-L-009` adds a read-only contract for future
Lightning/Pylon graph visualization.

The implementation lives in
`workers/api/src/pylon-lightning-graph-contract.ts`.

## Scope

This is a contract and projection layer. It is not a live graph API route yet.
The implementation status is represented explicitly as `contract_only`,
`projection_available`, or `route_live`, and the current conformance fixture is
`contract_only`.

The graph can represent:

- providers;
- rails;
- peers;
- channels;
- liquidity movement;
- payout events;
- failed routes; and
- settlement receipt refs.

Edges can represent provider-to-rail, provider-to-peer, peer-to-channel,
channel-liquidity, work-payout, failed-route-on-rail, and settlement-evidence
relationships.

## API Contract Shape

The projection includes:

- graph nodes and edges;
- node and edge counts;
- implementation status;
- filters for node kind, edge kind, freshness, status, provider refs, and rail
  refs;
- bounded pagination with safe cursor refs;
- freshness, blocker, caveat, evidence, link, and source refs; and
- friendly `createdAtDisplay`, `updatedAtDisplay`, and per-node/per-edge
  `updatedAtDisplay` values.

The page limit is bounded from 1 through 100. Cursor refs are safe opaque refs,
not raw auth tokens or database cursors.

## Authority Boundary

`PYLON_LIGHTNING_GRAPH_READ_ONLY_AUTHORITY` denies:

- graph mutation;
- channel mutation;
- peer mutation;
- liquidity mutation;
- wallet mutation;
- live wallet spend;
- payout dispatch; and
- settlement mutation.

The contract can describe graph evidence. It cannot open channels, change
peers, rebalance liquidity, spend from a wallet, dispatch payouts, or claim
settlement.

## Redaction

Public, customer, team, and agent projections hide private nodes, private
edges, and private refs according to audience. Edges that reference hidden
nodes are removed from the projected graph.

All projections reject private channel monitor state, raw graph snapshots,
peer secrets, node pubkeys, wallet material, raw bitcoin payment material,
invoices, preimages, payout targets, provider secrets, credentials, private
repo refs, customer data, and raw timestamps.

Operator and private projections can show safe internal refs such as
`channel.private.*` or `peer.private.*`, but they still cannot contain wallet
material, raw payment material, raw channel monitor state, or credentials.

## Tests

`workers/api/src/pylon-lightning-graph-contract.test.ts` covers:

- schema decoding;
- read-only authority;
- explicit `contract_only` implementation status;
- public redaction of private nodes, private edges, private refs, and linked
  edges;
- bounded pagination and redacted filters;
- unique graph IDs and known edge endpoint validation;
- failed, blocked, stale, and attention evidence requirements; and
- rejection of unsafe Lightning, peer, wallet, payment, payout target,
  provider, customer, credential, and timestamp material.
