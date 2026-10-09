---
id: openagents.github-projects
version: 2
kind: product
title: "Connecting a GitHub repository"
summary: >-
  On openagents.com, sign in with GitHub, connect GitHub on the Projects
  page, and add a repository as a project; chats started in it are grouped
  under it in the left panel.
tags: [github, repository, repo, project, projects, connect, private, sidebar, website]
applies_when: >-
  The user asks how to connect, add, or link a GitHub repository or repo,
  how to see their private repositories, what a project is, how to start a
  chat in a project, how to move a chat into a project, or what "Reconnect
  GitHub" means on openagents.com. Not how Coder uses git on a connected
  computer, and not signing in itself.
answer: >-
  Sign in with GitHub at https://openagents.com/login, then open Projects at
  https://openagents.com/projects and connect GitHub. GitHub asks for access
  to private repositories only if you include them; public only asks for
  nothing new. Pick a repository to add it as a project. Chats you start from
  the project, or with it picked under the message box, are grouped under it
  in the left panel, and a chat's menu can move it there. If GitHub access
  ends, your projects stay and say Reconnect GitHub.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/auth/github.md
    - docs/web/sidebar.md
    - crates/openagents-web/src/projects/mod.rs
    - crates/oa-auth/src/repos.rs
evidence:
  - "2026-10-09: written from the cited documents and code and checked against them (#11034); the answer text awaits the owner's copy review."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
---

## Answer

Sign in with GitHub at https://openagents.com/login, then open Projects at https://openagents.com/projects and connect GitHub. GitHub asks for access to private repositories only if you include them; public only asks for nothing new. Pick a repository to add it as a project. Chats you start from the project, or with it picked under the message box, are grouped under it in the left panel, and a chat's menu can move it there. If GitHub access ends, your projects stay and say Reconnect GitHub.

## Details

- Signing in with GitHub asks only for your profile and email addresses. Seeing private repositories, and those of your organizations, asks GitHub for the `repo` and `read:org` permissions, once, when you connect.
- Projects are yours: someone who isn't signed in, or another account in the same browser, never sees their names.
- Each project group shows five chats, then Show more; a closed group stays closed in that browser.
- Disconnecting GitHub on the Projects page forgets the access; your projects stay.
- The GitHub access is stored encrypted and is never shown or logged.

## Sources

- `docs/auth/github.md`
- `docs/web/sidebar.md`
- `crates/openagents-web/src/projects/mod.rs`
- `crates/oa-auth/src/repos.rs`
