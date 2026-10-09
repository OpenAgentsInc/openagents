---
id: openagents.connect-codebase
version: 2
kind: product
title: "Connecting your codebase"
summary: >-
  Two ways to connect a codebase: add a GitHub repository as a project at
  openagents.com/projects, or run Coder in the repository on your own
  computer, signed in with `coder login` and synced with `/sync on`.
tags: [codebase, code, repository, repo, project, connect, github, coder, install, login, sync]
applies_when: >-
  The user asks how to connect, link, hook up, or add their codebase, code,
  project, or repository so we can work on it, or how to get started with
  their own code. Not a specific change to make in a repository, and not
  only how GitHub sign-in or projects work (openagents.github-projects).
answer: >-
  Two ways: add a GitHub repository at https://openagents.com/projects, or
  use Coder in it on your own computer.
ui: |
  root = Columns([web, computer])
  web = Card("On the web", [Text("Sign in, connect GitHub, and pick a repository to add as a project."), github, login])
  github = Button("Connect GitHub", href="/auth/github/repos?access=private", show="signed_in")
  login = Button("Log in to connect", href="/login?return_to=/projects", show="signed_out")
  computer = Card("On your computer", [Steps([install, signin, sync])])
  install = Step("Install Coder", [Command("curl -fsSL https://openagents.com/cli/install.sh | bash", windows="irm https://openagents.com/cli/install.ps1 | iex")])
  signin = Step("Sign in, then approve it on the web", [CodeBlock("coder login", "bash"), Button("Approve sign-in", href="/device", style="secondary")])
  sync = Step("Run Coder in your repository and turn on sync to see its chats on the web", [CodeBlock("coder", "bash"), CodeBlock("/sync on")])
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/projects/mod.rs
    - crates/openagents-web/src/pages/download.rs
    - scripts/install/coder.sh
    - crates/coder-new/README.md
    - docs/auth/github.md
evidence:
  - "2026-10-09: written for the starter question 'How do I connect my codebase?' (#11095) from the cited code and documents, and checked against them; it matches the prepared answer meta.codebase."
  - "2026-10-09: v2 (#11187): the answer is a one-line lead, and its steps are components (ui): a Connect GitHub button, the install command for each system with Copy, coder login with an Approve sign-in button, and /sync on; the same as meta.codebase v2, checked against the cited sources."
---

## Answer

Two ways: add a GitHub repository at https://openagents.com/projects, or use Coder in it on your own computer. On the web, sign in, connect GitHub at https://openagents.com/projects, and pick a repository. On your own computer, install Coder (`curl -fsSL https://openagents.com/cli/install.sh | bash`, or on Windows `irm https://openagents.com/cli/install.ps1 | iex`), run `coder login` and approve it at https://openagents.com/device, then run `coder` in your repository and type `/sync on` to see its chats on the web.

## Details

- On the web, the Projects page shows Connect GitHub. GitHub asks for access to private repositories only if you include them. Pick a repository under Add a repository to make it a project; chats you start in a project are grouped under it in the left panel.
- The website's chat can't read or change your files itself. Work in the code happens with Coder, our coding agent, in your terminal on your own computer, using that computer's own git and GitHub login.
- The install command installs `coder` and the `openagents` command together. Run it again to update. The download page is https://openagents.com/download.
- `coder login` (or `/login` inside Coder) shows a short code; approve it at https://openagents.com/device while signed in on the website.
- `/sync on` saves Coder's chats to your account, where they show in the website's left panel with your computer's name. `/sync all` adds earlier chats and `/sync off` stops.

## Sources

- `crates/openagents-web/src/projects/mod.rs`
- `crates/openagents-web/src/pages/download.rs`
- `scripts/install/coder.sh`
- `crates/coder-new/README.md`
- `docs/auth/github.md`
