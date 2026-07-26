---
title: Full Auto
description: Understand the bounded automation direction that Omega is developing.
lastModified: 2026-07-25
sidebar:
  order: 4
---

Full Auto is the Omega direction for sustained agent work in a project. It is designed to continue useful work while the user can inspect, steer, or stop the run.

Full Auto is in active development. Current engine and interface evidence does not prove general release availability.

## Intended experience

Full Auto should:

- Use the project and execution context that the user selected.
- Keep agent work visible in the same workspace.
- Show plans, tools, changes, blockers, and delegated work.
- Preserve one causal record across continuations.
- Stop when a safety, authority, or evidence gate requires a decision.
- Make interruption and recovery explicit.

It must not create a hidden workroom or convert a UI control into deployment, payment, repository, or release authority.

## Agent routing

Omega is not limited to one agent provider. The current direction supports a first-party Omega agent and external agents through explicit routing boundaries.

Each agent keeps its own configuration and credential custody. Omega records the route and result without claiming that one provider owns the project.

## Safety boundary

Automation can act only within the authority that the selected environment already grants. A running agent does not gain more authority because Full Auto is on.

The product must keep these states visible:

- Running work
- A pending question or approval
- A failed or interrupted continuation
- An unavailable provider or engine
- A completed result that still needs review

## Proof status

OpenAgents has current Full Auto contracts, engine work, and proof tooling. Installed replay, sustained owner use, and independent release evidence still have open gates. Treat Full Auto as a development capability until one exact candidate passes those gates.
