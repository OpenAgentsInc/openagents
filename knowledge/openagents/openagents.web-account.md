---
id: openagents.web-account
version: 2
kind: product
title: "Signing in on openagents.com"
summary: >-
  The web chat works without an account; signing in with GitHub keeps chats
  on an account across browsers and unlocks projects, Settings, and signing
  in to Coder.
tags: [account, sign-in, login, sign-up, github, website, settings]
applies_when: >-
  The user asks whether they need an account, how to sign in, log in, sign
  up, or create an account on openagents.com, what signing in gives them,
  why their chats disappeared in another browser, or how to sign out. Not
  how to connect a GitHub repository as a project
  (openagents.github-projects), and not signing in to Coder in the terminal
  (openagents.coder-sync).
answer: >-
  You don't need an account to chat: without one, your chats belong to this
  browser. To keep them on an account, sign in with GitHub at
  https://openagents.com/login (or **Sign up** at the top right). GitHub
  shares only your profile and email addresses. Signed in, your chats show in
  any browser where you sign in, chats you started signed out move to your
  account, and you can add your GitHub repositories as projects, save your own
  Claude key in Settings, and sign in to Coder.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/auth/README.md
    - docs/auth/github.md
    - crates/openagents-web/src/chat_owner.rs
    - crates/openagents-web/src/auth.rs
evidence:
  - "2026-10-09: written from the cited documents and code and checked against them (#11039, #11045, chat goldens); the answer text awaits the owner's copy review."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
---

## Answer

You don't need an account to chat: without one, your chats belong to this browser. To keep them on an account, sign in with GitHub at https://openagents.com/login (or **Sign up** at the top right). GitHub shares only your profile and email addresses. Signed in, your chats show in any browser where you sign in, chats you started signed out move to your account, and you can add your GitHub repositories as projects, save your own Claude key in Settings, and sign in to Coder.

## Details

- Sign up and sign in are the same step: **Continue with GitHub** on `/login` or `/signup`. Cancelling at GitHub changes nothing.
- Signing in asks GitHub only for your profile and email addresses. Seeing private repositories is a separate step, when you connect GitHub on the Projects page.
- Signed out, a random browser cookie tells your chats apart: only this browser opens them, and clearing your cookies loses them.
- After you sign out, someone else using that browser doesn't see your account's chats.
- Settings, in the account menu at the bottom left, holds your name, email, and avatar, the theme, your Claude credential, and the computers signed in to Coder, each of which you can remove.
- Sign out is in the same account menu.

## Sources

- `docs/auth/README.md`
- `docs/auth/github.md`
- `crates/openagents-web/src/chat_owner.rs`
- `crates/openagents-web/src/auth.rs`
