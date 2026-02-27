# Mezod And Validator Kit

Use this reference for node/validator/operator workflows.

## mezod Quickstart

Prereqs:
- Go 1.21+
- Git
- Make
- Optional Docker

Commands:

```bash
git clone https://github.com/mezo-org/mezod.git
cd mezod
go mod download
make build
make dev
```

Quality checks:

```bash
make test
make lint
```

## Validator Kit Modes

Repository: `https://github.com/mezo-org/validator-kit`

Primary deployment modes:
- `docker` (recommended default path)
- `native`
- `helm-chart`
- `manual`

Use latest validator-kit release, not `main`, for stable operations.

## Node Types And Minimum Sizing

- Validator: `4 vCPU / 16 GB RAM / 256 GB disk`
- RPC: `8 vCPU / 32 GB RAM / 512 GB disk`
- Seed: `2 vCPU / 8 GB RAM / 128 GB disk`

## Synchronization Strategy

- **Block sync from genesis**:
  - complete history
  - slower
  - may require version stepping through historical upgrades
- **State sync from snapshot**:
  - faster bootstrap
  - incomplete historical data
  - trust assumptions on snapshot source

## PoA Validator Onboarding

Only for validator application flows:

```bash
mezod --home=<mezod_home_path> --rpc-url <rpc_url> poa submit-application <key_name>
```

## Operational Notes

- Ensure required external ports are explicitly configured and exposed per node role.
- For non-validator nodes, skip PoA submission.
- Treat monitoring and allowlist requirements as operational requirements, not optional documentation.
