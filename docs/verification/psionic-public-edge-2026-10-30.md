# Public SystemOne edge qualification — 2026-10-30

Scope: the new authenticated `/api/v1/systemone` edge, its generated Coax policy
cache, compatibility API authentication integration and native fleet socket bind.
This record does not claim a production or learned-model inference deployment;
those calls must be recorded independently by the deployment owner.

Qualified compiler: SDK cache `81ad04b7877f29c0f424154fb27589c1b1839ef1d05ed28d724c488fc72f28c5`,
binary SHA-256 `f0f088f04c992655a1c119ba229240288ccf67f755c78b065017c1441e825eca`.
Policy/dependency/generated C hashes are pinned in
`coax/web/api/mesh-policy.sha256`. New changes are based on origin/main
`5bcb91c5af`; an unrelated rebase does not require rerunning these unchanged
checks. Hash drift does.

- `coax test coax/web/api/mesh-policy.coax --steps 250000000`: 34 checks passed,
  including two exhaustive 256-case byte classifications and shared JSON checks.
- `coax mutate coax/web/api/mesh-policy.coax --root-only --jobs 6 --run-steps 250000000`:
  194 mutants, 194 killed, 0 survivors. Mutation selected only new policy source;
  imported JSON checks remained in every baseline.
- `COAX=... coax/web/api/mesh-test.sh`: 41 scoped HTTP/database/upstream checks
  passed against a throwaway PostgreSQL 16 database and HTTP fleet fixture, using
  freshly generated keys. Full compatibility API and signer built with native
  ASan/UBSan and `-Werror`; no sanitizer findings. Coverage: signature method/URL/
  payload/freshness/replay, browser session/expiry/CSRF, explicit bad-header refusal,
  raw body preservation, no credential forwarding, malformed/duplicate models,
  no caller-selected URL, exact 16384-byte admission/16385-byte refusal, actual HTTP 422 retention, model/checkpoint/provider substitution, oversized replies,
  redirect/IAM refusal, upstream availability, per-account and global quotas, expired
  leases and concurrent lease ownership/completion. No live API or Pylon touched.
- `clang -std=c11 -O1 -g -Wall -Wextra -Werror -fsanitize=address,undefined
  psionic/platform/fleet-host-test.c -lpthread -o /tmp/fleet-host-test` then run:
  default and explicit loopback, explicit container bind, preserved Coax argv,
  invalid/missing/duplicate host arguments passed; no sanitizer findings.
- `coax/web/api/mesh-client-test.sh`: exact production/staging URL, existing
  NIP-98 event format, body hash, disposable-key permissions/deletion, explicit
  existing key preservation and invalid arguments passed with a sanitized signer
  and an independent no-network curl fixture.
- `mesh-policy.sh --check` reproduces C/header byte-for-byte. `--check-inputs`
  confirms authored source, JSON dependency and cache hashes. All modified shell
  scripts pass `bash -n`; `git diff --check` passes.

No unrelated paid inference/payment suites or compiler/model suites were rerun.
The unchanged published NIP-DEC/protobuf wire contract has no new schema fields.
Public edge currently accepts JSON only. Authentication still uses the existing
compatibility C/SQL verifier/session boundary; no production Coax signer gate was
changed. Confidential hardware/execution is not asserted by this profile.
