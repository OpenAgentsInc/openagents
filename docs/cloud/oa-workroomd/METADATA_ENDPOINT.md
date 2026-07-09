# oa-workroomd Local Metadata Endpoint

Status: Cloud MVP scaffold for `CND-013`

`oa-workroomd metadata` exposes non-secret workroom context to agents and tools.

```bash
oa-workroomd metadata init \
  --workroom workroom.local.echo \
  --program program.local.smoke \
  --repo repo.openagents.echo \
  --template template.posix.echo \
  --budget runtime_ms=60000,cost_microusd=1000000 \
  --deadline 2026-05-25T12:00:00Z \
  --trust-tier internal_test \
  --capability repo.read \
  --capability artifact.write \
  --json

oa-workroomd metadata get --json
```

The metadata file lives at:

```text
workroom-metadata.json
```

It includes:

- workroom id;
- program id;
- repo;
- template id;
- budget;
- deadline;
- trust tier;
- capability names.

It excludes raw secrets, tokens, wallet material, private keys, and private
topology markers. `metadata init` rejects fields containing those markers.

Every `metadata get` appends an access event to:

```text
metadata-access.jsonl
```

The MVP exposes the same local contract through a CLI command. A later HTTP or
Unix-socket endpoint should preserve the same response shape and access log.
