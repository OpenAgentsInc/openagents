# Enterprise And Managed Policy System Audit

Date: 2026-06-11

This is system #62 from the Bun/Effect terminal-agent systems list. It defines
how organizations can manage terminal-agent behavior across teams, devices,
repositories, providers, policies, budgets, retention, integrations, and
public projections.

## Target

Build a managed policy system that lets OpenAgents enforce organization-level
rules without hiding local runtime authority or weakening user-visible
receipts.

Managed policy should constrain and explain behavior. It should not be an
invisible override channel.

## User-Visible Capability

Users and admins should be able to:

- View effective managed policy.
- Understand why an action is allowed, denied, or requires approval.
- Configure team budgets, provider allowlists, data scopes, retention,
  telemetry, plugin policy, update channels, and integration gates.
- Apply policy by organization, team, repository, device, or project.
- Audit policy changes.
- Export policy state.

The terminal should always distinguish user preference from managed policy.

## Policy Model

Each policy record should include:

- Policy ref.
- Scope.
- Version.
- Owner or admin ref.
- Rule kind.
- Effective date.
- Expiration where applicable.
- Enforcement mode.
- Audit refs.
- Conflict priority.
- Public-safe summary.

Policy domains should include filesystem, shell, provider, model, payment,
budget, plugin, MCP, hook, remote bridge, telemetry, retention, update, and
release.

## Bun/Effect Boundary

Use Effect services for:

- `ManagedPolicyService`: loads and validates policy records.
- `PolicyResolutionService`: produces an effective policy snapshot.
- `PolicyEnforcementService`: answers allow, deny, ask, or restrict.
- `PolicyAuditService`: records changes and enforcement decisions.
- `PolicyProjectionService`: shows user, admin, and public-safe views.

Use Schema for policy records, rule kinds, conflicts, and enforcement
decisions. Use Layer to compose local, team, enterprise, and session policy
providers.

## Safety Rules

- Managed policy cannot grant capabilities unavailable to the runtime.
- Policy can restrict user choices but should not silently broaden authority.
- Denials must include user-safe reasons.
- Policy changes are auditable and versioned.
- Secrets are not stored in policy records.
- Public-safe policy summaries must omit private org details.
- Emergency overrides require expiry and receipt refs.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has team budgets, provider policy, product-promise
gates, public projection invariants, and payment boundaries in adjacent
surfaces. The terminal-agent README does not yet include an enterprise/managed
policy audit.

Related open issue anchors:

- #4770 team budgets and spend-to-evidence join.
- #4771 provider peers and terms-compliance review.
- #4773 API parity contract.
- #4778 mission/work-order unification.
- #4780 settlement bridge.
- #4785 settlement visibility law.

No managed-policy claim should be green until effective-policy snapshots,
conflict resolution, denials, audit records, and user-visible explanations are
tested.

## Tests

Minimum coverage:

- Resolve policy across user, project, team, enterprise, and session scopes.
- Deny, ask, restrict, and allow representative actions.
- Prevent policy from silently broadening authority.
- Audit policy changes and enforcement decisions.
- Hide private org details in public-safe projections.
- Enforce update-channel and telemetry policy.
- Enforce provider and budget policy.
- Expire emergency overrides.

## Decision

Enterprise policy should be a typed constraint layer over normal runtime
capabilities. It should make managed behavior explicit, auditable, and
explainable to both users and admins.

