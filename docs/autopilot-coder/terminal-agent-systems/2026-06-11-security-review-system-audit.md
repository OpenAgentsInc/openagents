# Security Review System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #53 from the Bun/Effect terminal-agent systems list. It defines
the security review path for terminal-agent capabilities, provider adapters,
workspace access, remote bridges, plugins, updates, and public projections.

## Target

Build a security review system that turns risky changes into explicit threat
models, policy refs, redaction checks, approval gates, and regression tests.

Security review should be routine, not a one-time launch checklist.

## User-Visible Capability

Users should be able to:

- See why a risky action needs approval.
- Trust that secrets and private data are excluded from public projections.
- Disable or restrict high-risk integrations.
- Inspect security-relevant settings and connected accounts.
- Export a redacted security diagnostic bundle.
- Understand when a capability is experimental, restricted, or blocked.

## Review Domains

Review domains:

- Filesystem and workspace boundary.
- Shell execution.
- Provider credentials and account leasing.
- Payment, wallet, payout, and settlement refs.
- Remote session bridge.
- MCP, plugins, hooks, and skills.
- Browser and desktop integration.
- Update and release artifacts.
- Public projections and product claims.
- Data retention and deletion.

Each high-risk domain should have an owner policy, test fixture, and redaction
scan.

## Bun/Effect Boundary

Use Effect services for:

- `SecurityPolicyService`: resolves capability and risk policy.
- `ThreatModelRegistry`: stores review records.
- `SecurityGateService`: blocks unsafe capability use.
- `RedactionScannerService`: scans artifacts, logs, projections, and docs.
- `SecurityReceiptService`: records review, exception, and denial receipts.

Use Schema for threat models, risk ratings, exceptions, and gate verdicts. Use
Layer to make review gates reusable in CLI, TUI, worker, and Pylon surfaces.

## Safety Rules

- Risk exceptions require expiry and receipt refs.
- No secret or private payload is allowed in public projection.
- Provider terms and credential ownership are reviewed per adapter.
- Plugin and MCP capabilities cannot bypass native approvals.
- Release artifacts require integrity checks.
- Security denial is a valid successful policy outcome, not a runtime crash.
- Security review cannot be replaced by model judgment.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has data classification docs, redaction suites,
provider policy work, payment boundaries, product-promise gates, and public ref
scanner work. The terminal-agent README does not yet include a security review
system audit.

Related open issue anchors:

- #4771 provider peers and terms-compliance review.
- #4773 API parity contract.
- #4780 settlement bridge.
- #4785 settlement visibility law.
- #4772 MVP exit review.

No high-risk terminal capability should be marked production-ready without a
threat model, policy gate, redaction scan, and regression fixture.

## Tests

Minimum coverage:

- Gate shell, filesystem, remote, plugin, provider, and payment actions.
- Reject public artifacts containing forbidden private material.
- Enforce exception expiry.
- Verify provider adapter credential policy.
- Generate security receipts without raw secrets.
- Treat policy denial as a typed outcome.
- Block unsigned update artifacts.
- Keep security diagnostics redacted.

## Decision

Security review should be a typed, enforceable part of the runtime. The same
policy refs that explain user prompts should also drive tests, release gates,
and public-claim boundaries.

