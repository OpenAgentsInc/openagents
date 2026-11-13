# 76. Releases from one source

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Every client passes the conformance run of chapter 75. This chapter ships
them: one source and one provenance record for every release, published to
each platform on that platform's own schedule, with a way back on every one.

## Draft

Episode 289 ends with a rollout: release candidates, a few days of finding
bugs, then stable versions on every platform, from openagents.com/download.
The audit records how that went. Version 1.0 shipped for Coder, and it did
not ship for the whole OpenAgents app on every platform.

The fix is not one pipeline that waits for everything. It is one source and
one record of where each build came from. Every release starts from one
commit. Each platform's build records that commit, the compiler that built
it and the checks it passed. One version number means the same source
everywhere. It does not mean the same day everywhere.

Platforms ship when they are ready. The website ships when its checks pass.
The iPhone app ships when App Store review approves it, which can take days,
and Android follows Google Play's own review. The download page lists a
build when its store has it, not when it was uploaded. Windows and other
additional platforms ship on their own evidence (chapter 77) and never hold
up the web or the phone.

An installed app is older than the server. People update late, and some
never do. The API answers every app version still in use (chapter 9), and a
release that changes a record adds a version beside the old one.

Some changes can reach an installed app without a store release.
[Coax Native Updates](../../coax/native-updates.md) describes signed
over-the-air delivery: content and compatible portable code that run inside
the installed runtime, inside each store's rules. It can't replace native
code, add a permission or change what the store reviewed. A change that
needs those is a store release. Each release says which kind it is.

Every release has a way back. On the web, the previous build is one step
away. On a phone, a store can't take back an installed app, so the way back
is a fix released forward, or, for an over-the-air update, the signed
previous package that the runtime keeps. A release can be stopped only by a
regression it causes, so each check compares against what is live now.

Release candidates stay part of the process. [Episodes
254](../../docs/transcripts/254.md) and [256](../../docs/transcripts/256.md)
show the pattern we keep: ship a candidate, use it ourselves, find the
failures, fix them, and promote the build that survived.

## What we build

A plan for what this chapter ships once the draft is approved:

- A provenance record per platform build: commit, compiler, checks and the
  target evidence of chapter 75.
- Per-platform release steps that publish independently, and a download page
  generated from what each store actually lists.
- The version rule between installed apps and the API, and the rule for
  which changes may go over the air.
- A way back for each platform: the previous web build, a fix forward for a
  store app, and the previous signed package for an over-the-air update.

It builds on what exists today:

- [`coax/remote`](../../coax/remote)
- [`coax/native-updates.md`](../../coax/native-updates.md)
- [`coax/web/release.sh`](../../coax/web/release.sh)
- [`apps/openagents-ios/release.sh`](../../apps/openagents-ios/release.sh)
- [`coax/web`](../../coax/web)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-desktop`](../../apps/openagents-desktop)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`apps/everglade-mobile`](../../apps/everglade-mobile)
- [`os`](../../os)

### The chapter's check

- **Source:** the provenance records the release scripts write.
- **Trace:** for one commit, each platform's record: commit, compiler,
  checks passed, targets qualified and where it is published.
- **Refusal:** a build whose commit has no passing conformance run for its
  target is refused for publishing.
- **Independent check:** rebuild the web build from its record on a second
  host and compare its bytes.

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Release status | `openagents.deploy.v1` `DeployStatus`, `LaneHead` ([spec 28.3](../../docs/protocol/openagents-protocol-spec.md#283-openagentsdeployv1)) | Nostr kinds 3205 and 30205 | Spec sketch; no `.proto` file |
| A signed build others can rebuild | `openagents.attest.v1` `Release`, `Rebuild` ([spec 27.2](../../docs/protocol/openagents-protocol-spec.md#272-openagentsattestv1)) | Nostr kinds 3202 and 3203 (NIP-ATT) | Spec sketch; no `.proto` file |

Planned paths: none. Releases extend [`coax/remote/`](../../coax/remote), each
app's own release script and [Coax Native Updates](../../coax/native-updates.md).

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
2. Which kinds of update may install on their own, and which ask the person
   first?
