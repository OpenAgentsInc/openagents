# Omega Agent documentation

- Status: active
- Owner: OpenAgents
- Date: 2026-07-27
- Audience: product, engineering, and assurance teams

Omega Agent is the first-party agent of Omega, the Zed-based OpenAgents
desktop application.
This directory owns the slim-agent program records: the evolution of
Omega Agent into a simple five-tool agent on the principles of the Pi
coding agent.

Read these documents in this order:

1. Read the
   [slim-agent audit](./2026-07-27-slim-agent-audit.md).
   It records the current state of the Omega agent stack, the Pi
   principles, the measured gap, and the work-destruction failure audit.
2. Read the
   [slim-agent specification](./2026-07-27-slim-agent-spec.md).
   It records the proposed five-tool contract (`read`, `write`, `edit`,
   `bash`, `delegate`), the work-loss guard, the packet plan, and the
   open owner questions.

The wider Omega Agent program corpus lives elsewhere:

- The admitted product contract is
  `specs/omega/omega-agent.product-spec.md` at `spec_revision: 1`.
- The router-program roadmap is
  [`docs/omega/2026-07-25-omega-agent-roadmap.md`](../omega/2026-07-25-omega-agent-roadmap.md).
- The shape record is
  [`docs/omega/2026-07-25-omega-agent-shape-record.md`](../omega/2026-07-25-omega-agent-shape-record.md).
- The severability trace is
  [`docs/omega/2026-07-25-omega-agent-cloud-severability-trace.md`](../omega/2026-07-25-omega-agent-cloud-severability-trace.md).
- The design analysis is
  [`docs/fable/2026-07-25-omega-agent-analysis.md`](../fable/2026-07-25-omega-agent-analysis.md).

The specification in this directory is a proposal.
It needs a ProductSpec revision and an owner admission before any packet
lands.
Current code, tests, and receipts own implementation truth.
