---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, package.json, apps/openagents.com/package.json
informed: OpenAgents contributors and agents
---

# Use Bun as the workspace runtime and toolchain

## Context and Problem Statement

OpenAgents is organized as a Bun workspace with apps and shared packages. The
root package scripts delegate checks into app workspaces, and
`apps/openagents.com` uses Bun scripts for typechecking, tests, architecture
guards, web builds, Worker checks, and deployment gates.

## Decision Drivers

* Keep local development and verification commands consistent across the
  monorepo.
* Use one fast TypeScript-oriented runtime for scripts, tests, and tooling.
* Align app and package instructions with the existing lockfiles and scripts.

## Considered Options

* Bun as the workspace runtime and package tool
* Node/npm as the primary workspace runtime
* Mixed runtime ownership per app

## Decision Outcome

Chosen option: "Bun as the workspace runtime and package tool", because the
repo, scripts, lockfiles, and operating instructions already assume Bun for
production TypeScript work and verification.

### Consequences

* Good, because contributors can use a consistent command shape such as
  `bun run --cwd apps/openagents.com check:deploy`.
* Good, because package and app checks share one default runtime.
* Bad, because Node-only packages or CLIs need explicit compatibility handling
  instead of becoming the default.

### Confirmation

The root `package.json`, app `package.json` files, `bun.lock`, app lockfiles,
and `check:deploy` scripts confirm this decision. Pull requests should keep new
workspace commands on Bun unless an app-specific runbook documents otherwise.

## Pros and Cons of the Options

### Bun as the workspace runtime and package tool

* Good, because it matches the current monorepo setup.
* Good, because the verification and deploy commands are already Bun scripts.
* Bad, because contributors need Bun installed even for documentation-adjacent
  changes that run full verification.

### Node/npm as the primary workspace runtime

* Good, because it is widely installed.
* Bad, because it conflicts with current scripts and lockfile ownership.

### Mixed runtime ownership per app

* Good, because each app could pick its preferred tool.
* Bad, because it would fragment verification and increase agent instructions.

## More Information

* `package.json`
* `bun.lock`
* `apps/openagents.com/package.json`
* `apps/openagents.com/AGENTS.md`
