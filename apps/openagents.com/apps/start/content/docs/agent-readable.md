---
title: Agent-readable docs
description: Static Markdown and discovery artifacts for tools and coding agents.
lastModified: 2026-07-19
sidebar:
  order: 8
---

The docs build emits static agent-readable artifacts without running an AI endpoint or MCP server.

## Available artifacts

- [`llms.txt`](https://openagents.com/docs/llms.txt) indexes the public documentation tree.
- [`llms-full.txt`](https://openagents.com/docs/llms-full.txt) provides the complete public documentation corpus.
- [`agent-readability.json`](https://openagents.com/docs/agent-readability.json) describes enabled static formats and content-use signals.
- Add `.md` to a documentation route to request its raw Markdown mirror.

These artifacts contain only the curated public content under this docs package. They do not index the repository-wide `docs/` directory, internal audits, runbooks, evidence, private traces, or operational topology.

## Authority boundary

Instruction files and documentation help tools understand the system. They do not grant authentication, repository, deployment, payment, moderation, or operator authority. Server-side policy remains authoritative for every action.

Ask AI and hosted MCP are disabled. Search is a local static Orama index built with the site.

## Human and agent language

OpenAgents uses ASD-STE100 Simplified Technical English for public technical text.
Human-facing text uses the base profile.

Agent-facing text also uses the base profile when the result is clear and short.
An agent-only record can use the controlled agent compact profile.
Use that profile only when extra technical terms or density improve speed and precision for agents.
It does not permit ambiguity or a weaker safety, authority, or evidence rule.

The [Desktop rc.25 release note](https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.25) shows this separation.
The human changelog explains user-visible changes.
The agent changelog gives compact issue, commit, contract, invariant, and evidence data.

## Product claim authority

Product promises are machine-facing evidence, not human website navigation. Agents that need current claim state should read the structured projections directly:

- [`/api/public/product-promises`](https://openagents.com/api/public/product-promises) is the current registry projection.
- [`/api/public/product-promises/transitions`](https://openagents.com/api/public/product-promises/transitions) is the transition receipt stream.
- [`/api/public/product-promises/audit`](https://openagents.com/api/public/product-promises/audit) is the audit projection.

Read the promise identifier, state, evidence, blockers, and caveat together. Documentation, a screenshot, a candidate receipt, or a ProductSpec does not independently make a capability publicly available. `/docs/product-promises` remains a compatibility redirect to this agent-facing boundary.
