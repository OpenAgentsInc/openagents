# Autopilot Task: Adjutant Site Work Template

Status: template
Target repo: OpenAgentsInc/openagents
Target branch: main
Primary agent: agent_adjutant
Team: team_openagents_core
Project: project_adjutant

## Assignment

- assignmentId: `<adjutant_assignment_id>`
- assignmentKind: `site_generation | site_adjustment | site_review | site_deployment`
- softwareOrderId: `<software_order_id>`
- siteId: `<site_project_id>`
- goalId: `<agent_goal_id>`
- targetUrl: `https://sites.openagents.com/<slug>`

## Objective

Describe the Site work in product terms. The runner should be able to execute
from this packet without hidden chat context.

## Output Contract

- Use the assignment ID as the work receipt.
- Use the software order and Site IDs as durable context.
- Produce reviewable site source, asset manifest entries, and a concise result
  summary.
- Keep public-facing output focused on the customer subject.
- Do not include credentials, OAuth state, callback URLs, bearer tokens, local
  secret paths, or raw runner payloads.

## Safety Rules

- Do not expose secrets, provider grants, callback tokens, OAuth data, billing
  internals, or raw customer private data.
- Do not invent deployment state.
- Do not deploy or widen access without operator review and the Sites launch
  checklist.
- Keep generated artifacts suitable for public review before they are saved as
  a Site version.

## Acceptance Criteria

- The generated Site artifacts are saved through the Sites version lifecycle.
- The operator can review the saved version before deployment.
- The assignment ledger records the run, resulting commit, saved version, and
  deployment decision in later lifecycle steps.
- Focused tests or manual verification are recorded in the run summary.
