# Coldcard forensic documentation

This directory contains the Coldcard historical analysis, independent
postmortem comparisons, practice-run instructions, and the current model-panel
and publication audits used by the Omega forensic roadmap.

These documents do not authorize unknown-key search, live wallet recovery,
third-party exploitation, maintainer contact, or public vulnerability claims.

## Current truth

The checked-in contracts, fixtures, deterministic tools, and Omega projections
are substantial implementation evidence. They are not the same as accepted
live execution. The master tracker,
[issue #9300](https://github.com/OpenAgentsInc/openagents/issues/9300), is open after
an independent acceptance audit. In particular, issues
[#9289](https://github.com/OpenAgentsInc/openagents/issues/9289) and
[#9290](https://github.com/OpenAgentsInc/openagents/issues/9290) remain open for
the exact managed-worker and source-delivery evidence.

Until those dependencies close with accepted receipts:

- development fixtures and focused tests may be shown as implemented;
- a live GCE forensic run, dependency-complete delivery, cleanup, or production
  readiness must not be claimed;
- Omega should expose the evidence reader and blocked states before it exposes
  live launch as available; and
- every source, artifact, exploitability, fingerprint, entity, movement, and
  identity conclusion stops at its independently supported rung.

## Reading order

1. [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md) — technical explanation
   of the historical RNG failure.
2. [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
   — source-grounded comparison with the pinned independent postmortem.
3. [`2026-08-01-omega-coldcard-forensic-practice-runbook.md`](2026-08-01-omega-coldcard-forensic-practice-runbook.md)
   — benchmark arms, stopping rules, evidence ladder, and operating sequence.
4. [`2026-08-01-bitcoin-node-forensic-capability.md`](2026-08-01-bitcoin-node-forensic-capability.md)
   — private historical replay and node boundary.
5. [`2026-08-02-wallet-security-posts-and-omega-thread-audit.md`](2026-08-02-wallet-security-posts-and-omega-thread-audit.md)
   — corrected visual observations, disputed claims, delegation provenance,
   and Omega UI consequences.
6. [`2026-08-02-forensic-model-panel-and-publication-gates-audit.md`](2026-08-02-forensic-model-panel-and-publication-gates-audit.md)
   — diverse model roster, typed fallbacks, independent verification, holdouts,
   and publication gates.
7. [`2026-08-04-x-posts-recent-summary.md`](2026-08-04-x-posts-recent-summary.md)
   — live X API recent-search snapshot of Coldcard discourse (external
   observation only). Access path:
   [`../grok/2026-08-04-x-api-and-xai-x-search-access.md`](../grok/2026-08-04-x-api-and-xai-x-search-access.md).
8. [`2026-08-04-cto-inside-job-thesis-analysis.md`](2026-08-04-cto-inside-job-thesis-analysis.md)
   — morning “CTO / inside job” thesis: linked posts, GPG/switck identity
   check, and which claims hold vs overreach.

The broader rationale and current product sequence live in the
[`docs/loupe` entry point](../loupe/README.md), the
[entropy-first Omega dashboard roadmap](../loupe/2026-08-02-entropy-first-omega-dashboard-roadmap.md),
the
[forensic roadmap](../loupe/2026-08-01-omega-forensic-analysis-roadmap.md), and
the
[implementation/operator guide](../loupe/2026-08-01-omega-forensics-implementation-and-operator-guide.md).

## Evidence labels

- **Repository observation:** a fact read from a cited repository source; not
  necessarily re-executed by the current document.
- **External observation:** a post, screenshot, or report that motivates a
  check but does not prove the underlying claim.
- **Fixture or test evidence:** an implemented and reproducible development
  path; not a live deployment or target result.
- **Accepted live evidence:** an independently reviewed receipt from the exact
  admitted environment and operation.
- **Recommendation:** a proposed product or operating control; not a statement
  that the control ships.
