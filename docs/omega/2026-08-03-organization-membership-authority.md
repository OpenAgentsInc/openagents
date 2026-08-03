# Organization membership authority

- Date: 2026-08-03
- Status: generated read boundary and durable empty-by-default authority
- Owner: Omega OAW-013 (`OpenAgentsInc/omega#218`)

## Decision

Organization membership is a separate authority. It is not inferred from an
Organization named in the planning graph, a principal authorized to submit a
Work command, a NIP-29 group, a fixture row, or a legacy billing Organization.

The All Work contract exposes `organization.membership.read`. Each read binds:

- the selected account ref;
- the current account generation; and
- the Effective Principal ref.

Only rows that match all three values are returned. A generation change cannot
reuse an older membership. Each row independently names the membership,
Organization, bounded display name, source revision, observed time, and
verified/stale/revoked state.

## Provisioning and startup

omega-effectd persists the authority at
`all-work/organization-memberships.v1.json` below its private data root. Startup
creates an empty revision-zero ledger. This is fail-closed: starting the app,
rendering OpenAgents fixture data, or negotiating the method grants no
membership.

Provisioning is an owner-local action over a validated complete state and an
exact expected revision. It is not exposed as an Omega Work command. The file
store uses restrictive permissions and atomic replacement. A later operator
step must enroll the real account/principal membership before Omega can render
an Organization or enable Organization-scoped commands.

## Remaining OAW-013 work

- Sync and consume the generated Rust artifact in Omega.
- Map the selected account and account generation to the generated request.
- Convert only an exact returned row into
  `OrganizationMembershipProjection`.
- Execute the eight-consumer scope-clear transaction for switching.
- Provision a real enrolled membership and run installed local/offline,
  stale/revoked, restart, multi-window, and multi-Organization isolation proof.

The generated model and an empty ledger are not installed membership evidence.
