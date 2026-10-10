---
id: openagents.coder-memory
version: 1
kind: product
title: "Coder's memory: instruction files and saved notes"
summary: >-
  Coder reads the repository's AGENTS.md and CLAUDE.md on every turn and
  keeps small notes about the user and each project across sessions, under
  ~/.openagents/memory on their computer. The user asks Coder to remember or
  forget something; `/memory` lists the notes and `/memory forget NAME`
  deletes one.
tags: [coder, memory, remember, forget, notes, agents-md, claude-md, terminal]
applies_when: >-
  The user asks what memory is in Coder, how Coder's memory works, whether
  Coder remembers things between sessions, how to make Coder remember or
  forget something, how to see or delete what Coder remembers, where those
  notes are kept, or whether Coder reads AGENTS.md or CLAUDE.md. Not whether
  we train on, store, or keep their chats, or who can see their messages
  (openagents.chat-privacy).
answer: >-
  Coder, our coding agent, keeps memory across sessions on your computer. On
  every turn it reads the repository's AGENTS.md and CLAUDE.md and your own
  ~/.openagents/AGENTS.md, and it keeps small notes about you (for every
  project) and about each project in ~/.openagents/memory. Tell Coder to
  remember or forget something; type `/memory` in Coder to list its notes and
  `/memory forget NAME` to delete one, or run `coder memory list`. Notes that
  look like a password or key are refused. Get Coder:
  `curl -fsSL https://openagents.com/cli/install.sh | bash`.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/coder-new/README.md
    - crates/coder-new/src/memory.rs
evidence:
  - "2026-10-09: written from the cited README and code (#11176) for the chat goldens' privacy fix (#11106); the answer text awaits the owner's copy review."
---

## Answer

Coder, our coding agent, keeps memory across sessions on your computer. On every turn it reads the repository's AGENTS.md and CLAUDE.md and your own ~/.openagents/AGENTS.md, and it keeps small notes about you (for every project) and about each project in ~/.openagents/memory. Tell Coder to remember or forget something; type `/memory` in Coder to list its notes and `/memory forget NAME` to delete one, or run `coder memory list`. Notes that look like a password or key are refused. Get Coder: `curl -fsSL https://openagents.com/cli/install.sh | bash`.

## Details

- Instruction files are read from the working directory up to your home folder, nearest first. Instruction files in other directories of the checkout are named so Coder reads them before working there.
- Project notes are shared by all worktrees of the same project. Notes are markdown files with a `MEMORY.md` index.
- `coder memory list|show|forget|instructions` does the same from scripts. `OPENAGENTS_MEMORY=off` turns instructions and memory off.
- Notes stay on your computer. The OpenAgents chat itself doesn't remember earlier conversations: each reply reads only that conversation.

## Sources

- `crates/coder-new/README.md`
- `crates/coder-new/src/memory.rs`
