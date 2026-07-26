---
title: Agent-readable docs
description: Static Omega documentation and discovery artifacts for tools and coding agents.
lastModified: 2026-07-25
sidebar:
  order: 8
---

The docs build emits static agent-readable artifacts. It does not run an AI endpoint or an MCP server.

## Available artifacts

- [`llms.txt`](https://openagents.com/docs/llms.txt) indexes the public documentation tree.
- [`llms-full.txt`](https://openagents.com/docs/llms-full.txt) provides the complete public documentation corpus.
- [`agent-readability.json`](https://openagents.com/docs/agent-readability.json) describes enabled static formats and content-use signals.
- Add `.md` to a documentation route to request its raw Markdown mirror.

These artifacts contain only the curated public content in this docs package. They do not index internal audits, runbooks, private traces, operational topology, or the complete repository `docs/` tree.

## Authority boundary

Documentation helps a tool understand Omega. It does not grant authentication, repository, deployment, payment, moderation, or release authority.

Search uses a local static index that the site builds. Ask AI and hosted MCP are not part of this docs surface.

## Availability language

Omega is in active development. A signed prerelease candidate, source test, screenshot, or producer record does not prove general availability.

Agents must read the version, candidate digest, result, open gates, and reviewer status together. The [Omega releases](https://github.com/OpenAgentsInc/omega/releases) page contains candidate artifacts and notes. The OpenAgents repository contains supporting plans and evidence records.

## Product claim authority

Product promises are machine-facing evidence. Agents that need current claim state should read the structured projections:

- [`/api/public/product-promises`](https://openagents.com/api/public/product-promises) is the current registry projection.
- [`/api/public/product-promises/transitions`](https://openagents.com/api/public/product-promises/transitions) is the transition receipt stream.
- [`/api/public/product-promises/audit`](https://openagents.com/api/public/product-promises/audit) is the audit projection.

Read the promise identifier, state, evidence, blockers, and caveat together. Documentation does not independently make a capability available. `/docs/product-promises` remains a compatibility redirect to this agent-facing boundary.
