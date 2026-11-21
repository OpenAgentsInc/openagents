# 8. Environment and repository chips

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 7's conversations all run in one place you can't see. This chapter
adds two chips under the composer: the environment, which says which
computer a conversation's work runs on, and the repository it works in. You
choose both before you send, and you can see both on every message after.

## Draft

A chip is a small component with a label and a menu: `composer.env` shows
the environment, and `composer.repo` shows the repository. Tapping one
opens its picker as a card over the chat page. A chip shows only choices
that work: an environment that is offline is drawn as offline and can't be
chosen, and a repository the environment can't reach is not listed.

An environment is a computer that runs agent work for you: this account's
hosted computer, or one of your own that you enrolled, such as a Mac or a
CoderOS machine. The
[environment application profile](../../docs/protocol/environments-claude-and-world-editing.md)
owns how a phone reaches it. The environment's runtime contract is
[environment-runtime-v1alpha1](../../docs/protocol/environment-runtime-v1alpha1.md).
The list of environments is a resource loaded by an effect, as chapter 7's
history is.

The chips set where a conversation's turns run, and the choice is part of
the conversation. A conversation started on one environment stays on it;
choosing another starts a new conversation, so one turn never runs on two
computers. That is chapter 37's rule, one execution owner per task, as it
first appears in the app. The phone only presents the turn; it never runs
the tool calls itself.

The repository picker lists the repositories the chosen environment has.
The phone never clones or reads a repository. It sends the environment's
answer back as the conversation's setting.

## What we build

A plan for what this chapter ships once the draft is approved:

- The chips `composer.env` and `composer.repo` under the composer, each
  with its picker card.
- The environment and repository lists as resources loaded by effects.
- The choice stored with the conversation, and a new conversation when you
  change the environment.

It builds on what exists today:

- [`docs/protocol/environments-claude-and-world-editing.md`](../../docs/protocol/environments-claude-and-world-editing.md)
- [`docs/protocol/environment-runtime-v1alpha1.md`](../../docs/protocol/environment-runtime-v1alpha1.md)
- [`apps/shared/environment-app.coax`](../../apps/shared/environment-app.coax)
- [`components/zeron/composer.coax`](../../components/kit/form.coax) (its chips)
- `components/zeron/picker.coax`

### The chapter's check

- **Source:** the chips and pickers in the lean app, with their checks, and
  `openagents/examples/cases/chips.ops`.
- **Trace:** `tap composer.env`, `tree` lists two environments, one marked
  offline; `tap env.ID` for the online one; `tap composer.repo`, `tap
  repo.ID`; send; `state conversation` prints the environment and the
  repository.
- **Refusal:** `tap env.ID` on the offline environment answers `env.ID
  can't be pressed now`, and the conversation keeps its environment.
- **Independent check:** the environment runtime's own checks against the
  same recorded lists.

Planned paths: `openagents/examples/cases/chips.ops`. The app is
[`apps/openagents-ios`](../../apps/openagents-ios).

## Sources

- **Episodes:**
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): one suite across computers, cloud and mobile
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across terminal, Android and iOS
- **Also:**
  - [The environment application profile](../../docs/protocol/environments-claude-and-world-editing.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3

## Open questions for the owner

1. Which environments does the first build list: this account's hosted
   computer, your enrolled computers, or both?
