# Edge Gateway Rate-Limit Service

Owner: Platform Traffic Core (`traffic-edge`)

## Architecture Notes
The gateway evaluates per-tenant token buckets using JSON configuration files under `config/`.
All production quota changes must update `config/rate_limits.json` and include a matching
`RELEASE_NOTE.md` entry citing the change ticket, baseline commit SHA, and updated key/value pair.

## Operational Runbook
1. Validate JSON formatting prior to rollout.
2. Confirm window duration remains unchanged unless explicitly requested.
3. Preserve existing unrelated keys when introducing a new quota key.
4. Archive release notes in repository root as `RELEASE_NOTE.md`.
