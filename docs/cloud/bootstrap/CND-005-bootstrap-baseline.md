# CND-005 Cloud Repo Bootstrap Baseline

Status: bootstrap scaffold verified for Cloud MVP

This repo is the private implementation home for managed OpenAgents Cloud node
and workroom infrastructure. Contributor Pylon remains open source in the
public `openagents` repo.

## Acceptance Mapping

- `AGENTS.md`: repo-local agent rules and public/private boundary.
- `INVARIANTS.md`: managed node, workroom, capability, secret, settlement, and
  receipt invariants.
- `README.md`: clone path, scope, current scaffold, and start-here pointers.
- `docs/ARCHITECTURE.md`: component boundaries across Autopilot, Forge, Nexus,
  Pylon, `oa-node`, `oa-workroomd`, Psionic, and Treasury.
- `docs/ISSUES.md`: CND issue list and ownership rules.
- `docs/contracts/openagents.cloud_node.v1.md`: node contract scaffold.
- `docs/contracts/openagents.workroom.v1.md`: workroom contract scaffold.
- `crates/oa-node`: managed node daemon placeholder.
- `crates/oa-workroomd`: workroom sidecar placeholder.
- `crates/openagents-cloud-contract`: executable contract fixtures and
  validation.

## Verification

Run the bootstrap proof from the repo root:

```bash
scripts/verify-bootstrap.sh
```

The script checks required files, runs `cargo check`, runs the contract tests,
checks both binary help paths, and verifies the JSON status contract versions
emitted by `oa-node` and `oa-workroomd`.
