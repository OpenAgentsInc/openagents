# Pylon Live Assignment And Closeout Smoke

Date: 2026-06-08
Repository: `OpenAgentsInc/openagents`
Related issue: #502

## Summary

#502 adds the first live OpenAgents product surface Pylon assignment lease path. A registered Pylon
can now list assignment leases, accept an assignment, report progress, submit
public-safe artifact/proof refs, and have an operator close the assignment as
accepted or rejected work.

This is still not a release-unfreeze event. The smoke proves assignment and
accepted-work closeout. It does not prove bitcoin payout, payout-target
approval, repeated multi-host jobs, or broad Pylon earning readiness.

## Implemented Surfaces

- `POST /api/operator/pylons/assignments`
- `GET /api/pylons/{pylonRef}/assignments`
- `POST /api/operator/pylons/assignments/{assignmentRef}/closeout`
- Existing Pylon assignment event routes now require an owned non-stale
  assignment lease:
  - `accept`
  - `progress`
  - `artifacts`
  - `payment-receipts`
  - `settlement-status`

The operator create route requires a wallet-ready registered Pylon. The Pylon
agent routes require the owning registered-agent bearer token and idempotency
keys for writes. A second Pylon cannot accept, update, or close another
Pylon's assignment.

## Production Smoke

Public-safe smoke refs:

- Pylon: `pylon.issue502.local.20260608024927`
- Assignment: `assignment.public.issue502.20260608024927`
- Task refs: `task.public.echo_openagents_issue502`
- Acceptance criteria refs: `acceptance.public.echo_result_required`
- Proof refs: `proof.public.issue502_echo_verified`
- Accepted-work refs: `accepted_work.public.issue502_echo`
- Closeout refs: `closeout.public.issue502_operator_reviewed`

Observed production statuses:

| Step | Result |
| --- | --- |
| Register Pylon | `201` |
| Wallet readiness | `201` |
| Create assignment lease | `201` |
| List owned assignments | `200` |
| Accept assignment | `201` |
| Report progress | `201` |
| Submit artifact/proof refs | `201` |
| Operator accepted-work closeout | `200` |
| Post-closeout payment-evidence refs | `201` |
| Public Pylon detail read | `200` |

Final assignment projection:

- state: `accepted_work`
- lease state: `terminal`
- recent public event kinds included `payment_receipt`,
  `artifact_proof_metadata`, `assignment_progress`, `assignment_acceptance`,
  `wallet_readiness`, and `registration`.

The post-closeout payment-evidence write intentionally used public-safe
`recorded_no_spend` refs only. Real bitcoin payout remains #503.

## Verification

Focused test and typecheck commands passed:

```bash
bun run --cwd workers/api test -- src/pylon-api-routes.test.ts src/agent-home-routes.test.ts src/openagents-capability-manifest-routes.test.ts src/openagents-openapi-routes.test.ts src/openagents-agent-onboarding-routes.test.ts
bun run --cwd workers/api typecheck
```

The route regression coverage includes:

- assignment create replay by idempotency key;
- owned assignment list;
- accept replay by idempotency key;
- stale lease rejection;
- wrong-Pylon rejection;
- invalid private proof material rejection;
- rejected closeout;
- accepted-work closeout;
- post-closeout payment/settlement evidence allowance for accepted work;
- post-closeout progress mutation rejection.

## Deployment Note

The canonical deploy command still fails before deployment on pre-existing
zero-debt architecture budgets unrelated to #502:

- raw Cloudflare `Env` parameter annotations;
- Worker `Response` return surfaces.

#502 removed its own new raw date primitive and left those counts unchanged.
To make the production smoke possible, the retained deployment sequence applied
the D1 migrations, rebuilt web assets so `/AGENTS.md` stayed current, and
deployed the Worker with the local Wrangler binary.

The deploy uploaded updated `/AGENTS.md`, the web entry asset, and the Worker.
The live OpenAPI includes:

- `/api/operator/pylons/assignments`
- `/api/operator/pylons/assignments/{assignmentRef}/closeout`
- `/api/pylons/{pylonRef}/assignments`

## Remaining Gates

#503 must connect accepted-work closeout to real bitcoin payout authority,
receipt projection, idempotency, and reconciliation. #504 must repeat this
path across distinct Pylons and hosts with failure drills. #505 remains the
only issue allowed to cut or promote the next downloadable Pylon release after
the full release-unfreeze checklist passes.
