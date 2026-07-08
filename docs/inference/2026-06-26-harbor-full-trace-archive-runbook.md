# Harbor Full Trace Archive Runbook

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-26

Scope: operator-only capture of a Harbor / Terminal-Bench job directory for
forensic debugging of epic #6253 runs.

## Boundary

The archive contains raw private evidence: prompts, responses, commands, logs,
local paths, endpoint hints, and per-task Harbor artifacts may all be present.
It is not a public ATIF trace and is not safe for `/gym`, `/trace/{uuid}`,
product promises, issue comments, or public docs.

The production path is:

- `POST /api/operator/gym/full-trace-archives`
- `GET /api/operator/gym/full-trace-archives`
- `GET /api/operator/gym/full-trace-archives?archive_ref=...&download=1`

Every verb requires `Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN`.
Tarball bytes go to private R2 under
`private/gym/harbor-full-trace-archives/...`; metadata goes to D1 table
`gym_harbor_full_trace_archives`.

## Capture A Live Job

```sh
cd apps/openagents.com

OPENAGENTS_ADMIN_API_TOKEN=... \
bun run gym:harbor-full-trace-archive \
  --job-dir /tmp/khala-tb/khala-tb-1782410587 \
  --run-ref run.gym.terminal_bench.khala-live \
  --job-ref job.gym.harbor_terminal_bench.khala-tb-1782410587 \
  --json
```

Use `--dry-run --json` first when checking the local tarball and digest without
uploading.

The script creates a local `.tar.gz`, computes SHA-256 and byte length, then
uploads the archive body with:

- `x-openagents-run-ref`
- `x-openagents-job-ref`
- `x-openagents-archive-sha256`
- `x-openagents-archive-bytes`
- optional `x-openagents-archive-ref`

The server marks every archive as:

- `visibility: operator_only`
- `demandKind: internal`
- `demandSource: harbor_terminal_bench`
- `containsRawPrompts: true`
- `containsRawLogs: true`
- `containsPrivateMaterial: true`

## List Archives

```sh
curl -fsS \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  "https://openagents.com/api/operator/gym/full-trace-archives?run_ref=run.gym.terminal_bench.khala-live" \
  | jq .
```

The list response is metadata only. It includes a `downloadUrl`, but that URL
still requires the admin bearer.

## Download An Archive

```sh
ARCHIVE_REF="archive.gym.harbor_full_trace.<sha-prefix>"

curl -fL \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  "https://openagents.com/api/operator/gym/full-trace-archives?archive_ref=${ARCHIVE_REF}&download=1" \
  -o "${ARCHIVE_REF}.tar.gz"
```

Verify the downloaded bytes before unpacking:

```sh
shasum -a 256 "${ARCHIVE_REF}.tar.gz"
```

Compare the digest with `artifactSha256` from the list response.

## Promotion Rules

An operator may inspect the archive locally, but any material copied out of it
must be treated as private until it passes a separate public-safe path:

1. redact raw prompts, logs, local paths, provider payloads, secrets, payment
   material, PII, and endpoint hints;
2. convert the remaining material into the public-safe ATIF subset or another
   typed projection;
3. run the existing public-safety tripwire;
4. record explicit ownership/consent when the output is intended for training
   or sharing.

The archive metadata itself grants no accepted-work, payout, settlement,
provider mutation, spend, training-consent, or public-claim authority.
