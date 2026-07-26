---
title: Native workspace
description: Understand how Omega joins editor truth with agent work and evidence.
lastModified: 2026-07-25
sidebar:
  order: 3
---

Omega is an IDE, not a wrapper around one agent or one command-line tool. The native workspace is where project work, agents, code, review, and evidence meet.

## Editor and project truth

The Zed foundation owns editor, project, buffer, language, terminal, and worktree state. Omega must not create a second project graph for agent work.

An agent works against the project that the user opened. File changes, terminal work, and review stay connected to that project context.

## Agent work

Omega is being built to show human and agent work in one inspectable timeline. The target surface includes:

- User and agent messages
- Plans and current work
- Tool and command outcomes
- Questions, approvals, and interruptions
- File changes and review evidence
- Delegated agent activity

External agents keep their own configuration and credential custody. Omega must not become a second home for those credentials.

## Review and evidence

The workspace should connect a result to the code, tools, tests, and evidence that produced it. A status label or generated summary is not proof by itself.

Omega keeps release, payment, deployment, and repository authority outside a UI projection. A view can explain an action without gaining authority to perform it.

## Current status

Native workspace work is active in the Omega repository. Prerelease candidates and component proofs show parts of this direction. They do not prove that every workspace capability is complete or supported.
