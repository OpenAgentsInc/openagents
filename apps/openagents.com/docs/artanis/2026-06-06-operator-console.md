# Artanis Operator Console

Issue #405 / `ARTANIS-019` adds the first practical private Artanis console
inside `/autopilot`.

## Implementation

Code:

- `workers/api/src/artanis-operator-console-routes.ts`
- `workers/api/src/artanis-operator-console-routes.test.ts`
- `apps/web/src/page/loggedIn/artanis-console/`
- `apps/web/src/page/loggedIn/view.scene.test.ts`

The console reads the persisted Artanis rows from the #403 D1 family and
projects:

- runtime state;
- loop state, last tick, next tick, blockers, and selected context refs;
- health and Forum-publication lag;
- approval gates;
- work-routing proposals;
- Forum publication queue state;
- private operator steering refs by reference only.

## Operator API

`GET /api/operator/artanis/console` returns the private operator snapshot.

`POST /api/operator/artanis/approval-gates/{gateRef}/approve` and
`POST /api/operator/artanis/approval-gates/{gateRef}/reject` record an
operator decision for a pending approval gate and return a refreshed console
snapshot.

Both routes require either the admin API bearer token or an authenticated
OpenAgents admin browser session. Anonymous users receive `401`; signed-in
non-admins receive `403`.

## UI

The `/autopilot` workroom side panel now renders an `Artanis operator` dock
only for admin sessions. It exposes:

- current runtime, loop, health, approval, and Forum lag status;
- Artanis goal create, reprioritize, pause, resume, and cancel controls;
- private evidence and raw workroom refs by reference;
- pending approval gates with approve/reject controls;
- work-routing proposal refs and spend/cost caveat refs;
- Forum publication queue counts.

The goal controls reuse the existing browser Autopilot goal routes with
`agentId: "agent_artanis"`. Approval decisions persist approval-gate evidence
only. They do not dispatch work, mutate providers, spend bitcoin, redeem L402,
launch training/evals, publish Forum posts, promote runtime behavior, or settle
payouts.

## Public Boundary

The public `/artanis`, Forum, and AGENTS-facing surfaces remain downstream of
public-safe projections only. They must not render:

- private evidence refs;
- raw workroom refs;
- operator endpoints;
- operator receipt refs;
- raw timestamps;
- wallet, provider, runner, customer, payment, secret, or private-repository
  material.

Route tests cover unauthorized and non-admin denial, admin bearer access,
operator projection contents, and approval-action persistence. Scene tests
cover the admin-only dock, lifecycle controls, approval controls, and hidden
non-admin state.

## Remaining Work

#405 does not enable production autonomous operation. The next Artanis issues
still need to deliver queued Forum intents, add Forum listening and triage,
connect Nexus/Pylon adapters, add Pylon command packets, and pass the final
production launch gate before the scheduled runner is enabled.
