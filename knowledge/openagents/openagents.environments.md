---
id: openagents.environments
version: 1
kind: product
title: "Environments: running Claude Code on your repository"
summary: >-
  An environment is a GitHub repository set up on a cloud machine by our
  setup agent, checked, and saved, so Claude Code tasks run on a fresh
  machine made from it; environments are in early testing and not open to
  everyone on openagents.com yet.
tags: [environments, environment, claude-code, cloud, repository, setup, run]
applies_when: >-
  The user asks what an environment is, how to set one up, how to run
  Claude Code or an agent on their repository in the cloud, whether we can
  run their code or tests for them on our machines, how long setup takes, or
  when environments open. Not how to connect a GitHub repository as a
  project (openagents.github-projects), and not running Coder on their own
  computer (openagents.install-coder).
answer: >-
  An environment is one of your GitHub repositories, set up on a cloud
  machine so code can run there. You pick a repository and branch, watch
  our setup agent install it and answer its questions, and save the checked
  result. Claude Code then runs your tasks on a fresh machine made from that
  saved environment, using the Claude key you saved in Settings.
  Environments are in early testing and aren't open to everyone on
  openagents.com yet; they'll come with our Pro plan, $20 a month with 100 machine-hours
  included.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/cloud/environments-local.md
    - docs/web/cloud-reset.md
    - crates/openagents-web/src/environments/mod.rs
    - crates/openagents-web/src/lib.rs
evidence:
  - "2026-10-09: written from the cited documents and code and checked against them (#11032, #11037, #11052, chat goldens). /environments and a chat's Claude Code run answer only on the server's local address (crates/openagents-web/src/lib.rs guard), so the answer says they aren't open to everyone yet. The Pro plan with environment hours is the owner's decided plan (#11006, pricing note); the answer text awaits the owner's copy review."
---

## Answer

An environment is one of your GitHub repositories, set up on a cloud machine so code can run there. You pick a repository and branch, watch our setup agent install it and answer its questions, and save the checked result. Claude Code then runs your tasks on a fresh machine made from that saved environment, using the Claude key you saved in Settings. Environments are in early testing and aren't open to everyone on openagents.com yet; they'll come with our Pro plan, $20 a month with 100 machine-hours included.

## Details

- Setup: the agent checks out the exact commit of your branch, works out how to install it, writes an install recipe, and declares checks. Each step shows in the setup chat; you can type to it, and it stops to ask when it needs you.
- Finish builds a clean image from the recipe on a fresh machine, then checks that image on another fresh machine. Saving makes the checked result the environment's selected version.
- A chat in a project can run Claude Code in that repository's environment and shows the run in the chat.
- Each setup, build, check, and run machine is deleted when its step ends.
- Today Coder, our coding agent, also works on your own computer, in your terminal: get it at openagents.com/download.

## Sources

- `docs/cloud/environments-local.md`
- `docs/web/cloud-reset.md`
- `crates/openagents-web/src/environments/mod.rs`
- `crates/openagents-web/src/lib.rs`
