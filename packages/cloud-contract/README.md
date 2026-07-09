# `@openagentsinc/cloud-contract`

Hand-maintained Effect Schema mirrors of the OpenAgents Cloud Rust contracts
in `crates/openagents-cloud-contract`. Used by Worker / Pylon TypeScript callers
so shapes are not re-declared ad hoc.

Authoritative validators remain the Rust crate + `fixtures/cloud/` conformance
tests. Start here for:

- `openagents.codex_placement_assignment.v1`
- `openagents.cloud_vm_provisioner.v1`
- `openagents.gce_capacity_class.v1`
- `openagents.resource_usage_receipt.v1`
- `openagents.agent_computer_isolation_policy.v1`

See `docs/cloud/MIGRATION.md` and issue #8591.
