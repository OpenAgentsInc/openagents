# Omega Agent documentation

- Status: active
- Owner: OpenAgents
- Date: 2026-07-27
- Audience: product, engineering, and assurance teams

Omega Agent is the first-party agent of Omega, the Zed-based OpenAgents
desktop application.
This directory owns the slim-agent program records: the evolution of
Omega Agent into a simple six-tool agent on the principles of the Pi
coding agent.
The six tools are `read`, `write`, `edit`, `bash`, `delegate`, and
`plugin`.

Read these documents in this order:

1. Read the
   [slim-agent audit](./2026-07-27-slim-agent-audit.md).
   It records the current state of the Omega agent stack, the Pi
   principles, the measured gap, and the work-destruction failure audit.
2. Read the
   [slim-agent specification](./2026-07-27-slim-agent-spec.md).
   It records the five-tool contract (`read`, `write`, `edit`,
   `bash`, `delegate`), the work-loss guard, the packet plan, and the
   open owner questions.
3. Read the
   [plugin tool specification](./2026-07-27-plugin-tool-spec.md).
   It records the sixth tool: deterministic, typed, sandboxed
   functionality with a receipt per run, its plugin-economy, DSE,
   Blueprint, and Khala lineage, and the phased path to the registry
   and the paid market with revenue sharing for contributors.
4. Use the
   [hosted inference lanes runbook](./2026-07-27-hosted-inference-lanes-runbook.md)
   to operate Omega Agent's explicitly selectable Gemini Flash and
   Fireworks Kimi K3 lanes, including access controls, request shapes,
   testing, Cloud Run deployment, smoke checks, diagnosis, and rollback.

The wider Omega Agent program corpus lives elsewhere:

- The product contract is
  `specs/omega/omega-agent.product-spec.md` at `spec_revision: 3`.
  Revision 2 records the basic agent: out-of-box reliability
  on the default Google provider, delegation to installed harnesses, and
  Exo as a named delegate target.
  Revision 3 records the six-tool surface with `plugin`.
- The router-program roadmap is
  [`docs/omega/2026-07-25-omega-agent-roadmap.md`](../omega/2026-07-25-omega-agent-roadmap.md).
- The shape record is
  [`docs/omega/2026-07-25-omega-agent-shape-record.md`](../omega/2026-07-25-omega-agent-shape-record.md).
- The severability trace is
  [`docs/omega/2026-07-25-omega-agent-cloud-severability-trace.md`](../omega/2026-07-25-omega-agent-cloud-severability-trace.md).
- The design analysis is
  [`docs/fable/2026-07-25-omega-agent-analysis.md`](../fable/2026-07-25-omega-agent-analysis.md).

The specifications in this directory operate under ProductSpec
revision 3, which the owner directions of 2026-07-27 set.
Current code, tests, and receipts own implementation truth.
