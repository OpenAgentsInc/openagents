# 59. One release pipeline

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Every platform now has something to ship. This chapter ships it the same way
everywhere: one pipeline builds, signs and publishes each platform's app to
one download page.

## Draft

Episode 289 ends with a rollout: release candidates, a few days of finding
bugs, then stable versions on every platform, from openagents.com/download.
The audit records how that went. Version 1.0 shipped for Coder, and it did not
ship for the whole OpenAgents app on every platform.

Seven platforms can't be released by hand. Each has its own packaging, its own
signing, and often its own store with its own review. A release that needs a
person at seven different consoles will be late on some and wrong on others.

So there is one pipeline. It starts from one commit. It builds every platform
from the same source. It runs each platform's checks. It signs each result in
the way that platform requires. It publishes them together and updates the
download page. One version number means the same code everywhere.

Two rules shape it. A release can be stopped only by a regression that the
release itself causes, so each check compares against what is live now and not
against an ideal. And a release can be undone, so every platform keeps a path
back to the previous version.

Release candidates are part of the pipeline. [Episodes
254](../../docs/transcripts/254.md) and [256](../../docs/transcripts/256.md)
show the pattern we keep: ship a candidate, use it ourselves, find the
failures, fix them, and promote the build that survived.

Apps should update themselves where the platform allows it. A defaults update
(chapter 39) changes what the agent can do without a new app. An app release
changes the app.

## What we build

A plan for what this chapter ships once the draft is approved:

- One pipeline from a commit to signed builds for every platform, published
  together.
- Checks that compare a release with what is live, and a way back on every
  platform.
- The download page, generated from what the pipeline published.

It builds on what exists today:

- [`coax/remote`](../../coax/remote)
- [`coax/web`](../../coax/web)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-desktop`](../../apps/openagents-desktop)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`apps/everglade-mobile`](../../apps/everglade-mobile)
- [`os`](../../os)

Planned paths: `openagents/release/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, release candidates and the download page
  - [Episode 254, *Bug Bash*](../../docs/transcripts/254.md): a bug bash
  - [Episode 256, *Release Candidate*](../../docs/transcripts/256.md): a release candidate
  - [Episode 233, *The Monorepo*](../../docs/transcripts/233.md): one repository
  - [Episode 253, *Goodbye Bun*](../../docs/transcripts/253.md): changing the runtime under a product
- **Old code at `coder-v1.0.0`, crates:** `openagents-desktop`, `openagents-mobile`, `github-actions`, `mac-jobs`
- **NIPs:** [NIP-DEPLOY](../../nostr/nips/openagents/NIP-DEPLOY.md)
- **Also:**
  - The audit's section 5, "What else is needed besides the book", and its step 9
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P16

## Open questions for the owner

1. Which stores do we publish to at 1.0, and which platforms get a direct
   download only?
2. Do apps update themselves, or does the person choose each update?
