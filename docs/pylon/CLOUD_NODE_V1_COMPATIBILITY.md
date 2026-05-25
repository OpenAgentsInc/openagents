# Pylon Cloud Node v1 Compatibility

Status: public contributor compatibility fixture

Private managed OpenAgents Cloud implementation lives in the private `cloud`
repo. Contributor Pylon stays open source in this repo.

Pylon may implement the public subset of `openagents.cloud_node.v1` so private
managed nodes and public contributor nodes can be compared by Nexus, Forge, and
operator tooling without moving private fleet policy into public Pylon.

`crates/pylon-core` is the public extraction boundary for contributor-side
identity, admin state, availability, inventory, lifecycle, heartbeat, and
receipt projections. It deliberately stays testable without launching the TUI
and does not depend on the private `cloud` repo.

The public fixture lives at:

```text
docs/pylon/fixtures/cloud_node_v1/contributor-pylon.json
```

Compatibility rules:

- contributor wallet behavior remains public Pylon behavior;
- managed `oa-node` and `oa-workroomd` behavior stays in the private `cloud`
  repo;
- private fleet topology, private capacity placement, internal accounting
  adapters, capability broker internals, and workroom sidecar policy do not
  enter public Pylon;
- the fixture must not include wallet seeds, node entropy, private keys,
  preimages, bearer tokens, raw API keys, or private topology.

Verification:

```bash
cargo test -p pylon-core
cargo test -p pylon --test cloud_node_v1_fixture
```
