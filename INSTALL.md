# Install OpenAgents Software

This is the **canonical install guide** for what OpenAgents ships. If you are
a person, follow it top to bottom. If you are an AI agent installing on your
owner's behalf, aim at this file and follow it exactly — including the
[notes for agents](#notes-for-ai-agents) at the bottom. A fetchable copy of
the current install truth is also served at
<https://openagents.com/INSTALL.md>.
The former Khala Code desktop and mobile applications are deprecated and have
no active install or release lane. Their historical install and promise routes
remain only where promise/service-deliverable integrity requires them, and must
not advertise a new release.

Quick map — what do you want to install?

| Product             | What it is                                                  | Fastest path                                                                                            |
| ------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| **OpenAgents apps** | Web app and Omega on Desktop                                 | Web and the Omega alpha are live — [section 1](#1-openagents-apps) |
| **OpenAgents CLI**  | Released Rust CLI (`openagents`)                             | [section 2](#2-openagents-cli) |

## 1. OpenAgents apps

The supported product surface today is <https://openagents.com>. Omega is the
Desktop application, released from the Omega repository and downloaded from
<https://openagents.com/download>. The OpenAgents mobile application in this
repository is retired. Do not restore it.

The Electron OpenAgents Desktop application was deleted at owner direction on
2026-08-04 (#9325). It has no install, download, or update path. All former
`clients/` applications have been removed. Do not recover or distribute any of
them as current products. Use Omega or the Rust OpenAgents CLI.

The TypeScript Pylon package (`@openagentsinc/pylon`) is retired with the
TypeScript lane. Do not install it from this repository.

## 2. OpenAgents CLI

The supported installed path from this repository is the Rust `openagents`
binary. Build, checksum, and promote it through
[`docs/ops/2026-08-25-cli-release-runbook.md`](docs/ops/2026-08-25-cli-release-runbook.md).

From a clone of this repository:

```sh
cargo build --release -p openagents-cli
```

Coding work is delegated through `openagents coder`. Honest scope: installing
or running the CLI is a **capability, not an automatic earning path** — paid
work and settlement stay behind their own gated public promises.

## Notes for AI agents

- **Clone shallow.** Always `git clone --depth 1` this repo. A full clone
  downloads the entire history (~460 MB of `.git`, shallow is ~40 MB) for
  zero benefit — the working tree at HEAD is all any build needs.
- **Do not sparse-checkout.** A full working tree at HEAD is the supported
  cheap path.
- **This is a Cargo workspace.** Do not run `pnpm install` or look for
  `package.json`. The completion gate is `cargo fmt --all -- --check` then
  `cargo test --workspace`.
- **Never disturb an existing Codex login.** Do not run `codex login` (or
  any device-auth flow) against the default `~/.codex` home if a session
  already exists there, unless the owner explicitly asks — the flow wipes
  the stored login at start and kills the owner's live session.
- **Report honestly.** After installing, report what you ran and what you
  observed (versions, the command that launched, any errors) — do not claim
  earning, payout, or settlement capability from an install alone.
- Onboarding to the OpenAgents network itself (registration, Forum, earning
  paths) is a different document: <https://openagents.com/AGENTS.md>.

## When these instructions change

This file is the single source of install truth for the repo and is updated
in place as recommendations change (installer releases, version bumps, new
products). If a command here disagrees with an older doc, blog post, or
video, **this file wins**. Found a broken step? File it via the strict bug
form: <https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml>.
