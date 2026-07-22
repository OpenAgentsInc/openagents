# SBX-09 independent admission disposition (#9033)

This record is the independent-reviewer disposition for managed-agent-sandbox
SBX-09. The reviewer is distinct from the producer of the staging acceptance
aggregate and of the independent-verifier packet.

- Reviewer ref: `authority_delegated_independent_reviewer_grok_sbx09_2026_07_21`
- Producer ref: `assurance_packet_producer_sbx09_9033`
- Authority: root `AUTHORITY.md` revision 6,
  `grant.independent_assurance` and `grant.google_cloud_operations`
- Program: `program.managed_agent_sandboxes`
- Trigger: `issue.9033.independent_admission`
- Worktree base: `fe89a057cb` at review start

This record does not enable a production managed-sandbox flag. It does not make
a public availability claim. It does not start SBX-10.

## Authority decision receipt

The machine-readable admission receipt is:

[`docs/assurance/receipts/authority.decision.31b694c16d11ae30e10d86f0052692dd.json`](../assurance/receipts/authority.decision.31b694c16d11ae30e10d86f0052692dd.json)

The managed-sandbox AssuranceSpec lifecycle is `admitted` with
`admitted_by` equal to the reviewer ref above.

## Structural honesty fix

The managed-sandbox AssuranceSpec named
`independent_reviewer` under `verifier_roles` but listed only
`operating_agent` under `admitted_roles`. The FA-style independent path requires
`owner_designated_independent_reviewer` in `admitted_roles` for
`assessStructuralHonesty`.

This disposition adds `owner_designated_independent_reviewer` to
`admitted_roles` and to `verifier_roles`. That change is an honesty fix so an
owner-designated independent reviewer can admit the revision. It does not change
evidence tiers, proof rungs, or live gate claims.

## R1 through R8 (runnable, no live cost)

Reproduced from a clean detached worktree at current `origin/main` after
`pnpm install --frozen-lockfile`.

| ID | Result | Notes |
| --- | --- | --- |
| R1 | pass | `packages/managed-sandbox-contract/src`: 5 files, 19 tests |
| R2 | pass | six worker managed-sandbox suites: 6 files, 38 tests |
| R3 | pass | `managed-sandbox-authority.test.ts`: 1 file, 3 tests |
| R4 | pass | `managed-sandbox-store.test.ts`: 1 file, 7 tests |
| R5 | pass | `cargo test -p oa-codex-control managed_sandbox`: 19 unit + 2 integration, including default-off route |
| R6 | pass | production `deploy-cloudrun.sh` omits the three enable flags (grep exit 1) |
| R7 | pass | live harnesses refuse without `--apply` and `I_ACCEPT_LIVE_GCP_COST` |
| R8 | pass | aggregate verdicts and limitations match the packet. Digests stay public-safe only |

## O1 through O6 (live Google Cloud)

Owner cost gate: `OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST` with
`--apply`. Project `openagentsgemini`. Automation service account. Every guest
sandbox created during this review was deleted. Final managed-sandbox guest
inventory: zero instances and zero `oa-msb-*` firewall rules. Production stayed
default-off.

| ID | Disposition | Evidence |
| --- | --- | --- |
| O1 | incomplete_independent_rerun | Box live harness was launched against staging with the unmodified SDK path (`BOX_BASE_URL` ends in `/v1`). A guest sandbox was provisioned and ran through the control-plane restart window. The harness then failed during the independent residue oracle when local `gcloud` lost DNS to `compute.googleapis.com`. No complete public-safe proof JSON was written for that run. Residual guest compute and firewall rules were deleted by the reviewer. Producer aggregate still records `boxCompatibility.passed=true` with 26 proof bits. This row is not rounded up to independent green. |
| O2 | pass_independent | Native lifecycle harness `scripts/cloud/managed-sandbox-live-acceptance.ts --apply` passed. Journey create → probe → stop → resume → probe → stop → delete. Final generation 2. `cleanupObserved=true`. Measured cost 1501 microusd. Residue all zero. Evidence path process-local only. |
| O3 | pass_independent | On a live guest VM before teardown: no external IP. Internal IP only. Five generation firewall rules place priority-900 broker, SSH, and metadata allows above priority-1000 full ingress and egress denies. SSH source is limited to the staging control internal IP. Matches `network-policy-ref://openagents/managed-sandbox/broker-only-v1`. |
| O4 | pass_independent | After O2: global managed-sandbox guest inventory zero. After O1 cleanup: zero. Residual from earlier concurrent sessions was also deleted before the final runs. |
| O5 | pass_posture_corroborated | Production Cloud Run service omits `MANAGED_SANDBOX_BROKER_ENABLED`, `MANAGED_SANDBOX_BOX_V1_ENABLED`, and `OA_MANAGED_SANDBOX_IMAGE_DIGEST`. Invalid bearer to production `/v1/boxes` returns 404 route-not-enabled. Staging has the flags enabled and returns 401 for invalid bearer. Full staging enable/disable deploy flip was not repeated in this session (producer rollback matrix remains the prior live cutover record). Production was not enabled. |
| O6 | observed_producer_pending_owner | Sarah and Desktop journey proofs in the producer aggregate remain green (`sarahJourney.journeyProofPassed`, `desktopJourney.passed`). This session did not re-run those long owner journeys. Owner live-observation disposition remains a separate human gate where the product still requires it. |

## AssuranceSpec admission judgment

`review-admit` classified the managed-sandbox AssuranceSpec as:

- 13 executable local-unit oracle criteria, all green on independent reproduction
- 5 unclassified criteria (native Rust seams and live harness script paths, not claimed as executable)
- 0 smoke-gated, 0 receipt-backed blockers under the FA-style classifier
- structural honesty satisfied after the `admitted_roles` fix

Admission means this revision overclaims no evidence tier. It is not a claim
that every live row was independently re-run to green. It is not a claim that
production may be enabled. It is not a claim that SBX-10 is admitted.

## Residuals

1. O1 full Box SDK independent matrix remains incomplete in this session. A
   future independent re-run should complete only when local Google Cloud DNS
   and `gcloud` stay healthy through the residue oracle.
2. O6 owner live observation of Sarah and Desktop is still a human disposition
   when the product requires it for a public claim.
3. Production managed-sandbox flags stay off until a separate, explicit enable
   decision.
4. SBX-10 (#9032) stays open as deferred Phase-2 work. Do not start it from this
   disposition.
5. Staging remains the only environment with broker and Box facade flags on.

## Public safety

This disposition and the authority receipt carry digests, counts, HTTP status
codes, revision names, and public-safe observations only. They do not carry raw
sandbox refs, credentials, topology secrets, prompts, or runtime output.
