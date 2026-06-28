---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, docs/DEPLOYMENT.md, apps/openagents.com/package.json
informed: OpenAgents contributors and agents
---

# Land through PR review and the check:deploy gate

## Context and Problem Statement

OpenAgents changes land through pull requests, with verification expected before
merge. Root instructions require relevant tests and `check:deploy` to be green
for PRs, and root invariants ban GitHub-hosted CI workflows. The deployment
runbook names `check:deploy` as the pre-deploy gate for the main product
surface.

## Decision Drivers

* Keep merge evidence explicit and reviewable.
* Avoid GitHub-hosted CI and scheduled automation.
* Ensure product, Worker, contract, architecture, and projection guards run
  before deploy-sensitive changes land.
* Keep deployment separate from ordinary PR verification.

## Considered Options

* Pull requests with local or agent-run `check:deploy`
* GitHub Actions as the merge gate
* Merge without a full deploy gate

## Decision Outcome

Chosen option: "Pull requests with local or agent-run `check:deploy`", because
it matches the no-GitHub-Actions invariant and the existing `apps/openagents.com`
deploy gate.

### Consequences

* Good, because contributors and agents produce concrete verification evidence.
* Good, because the same gate protects public product, Worker, API, and shared
  package changes.
* Bad, because the full gate can take longer than targeted tests.

### Confirmation

The root no-GitHub-Actions invariant is enforced by `check:no-github-actions`
inside `check:deploy`. The `apps/openagents.com/package.json` script lists the
current guard, typecheck, and test suite.

## Pros and Cons of the Options

### Pull requests with local or agent-run `check:deploy`

* Good, because it keeps the repo free of GitHub-hosted workflow files.
* Good, because PR evidence can include the exact command run.
* Bad, because agents need enough local capacity to run the full gate.

### GitHub Actions as the merge gate

* Good, because it is common for public repositories.
* Bad, because it violates the root invariant banning GitHub-hosted CI.

### Merge without a full deploy gate

* Good, because it is faster in the short term.
* Bad, because it increases the chance of breaking public routes, contracts, or
  deployment assumptions.

## More Information

* `INVARIANTS.md` ("No GitHub-Hosted CI / Cloud Actions")
* `AGENTS.md`
* `docs/DEPLOYMENT.md`
* `apps/openagents.com/package.json`
