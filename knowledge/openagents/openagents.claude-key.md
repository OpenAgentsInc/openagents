---
id: openagents.claude-key
version: 1
kind: product
title: "Your own Claude key"
summary: >-
  Signed in on openagents.com, Settings → Claude credential keeps your own
  Anthropic API key, or a Bedrock, Vertex, or Foundry credential, encrypted,
  for Claude Code runs on your own environments; never paste it into a chat.
tags: [claude, anthropic, api-key, key, credential, settings, byo, bedrock, vertex]
applies_when: >-
  The user asks where or how to add, save, change, or remove their own
  Claude or Anthropic API key, or a Bedrock, Vertex, or Foundry credential,
  what we do with it, whether it is safe, or who pays for its usage. Not the
  chat's own model keys, and not OpenRouter, Vercel AI Gateway, or TypeSafe
  keys on a computer (openagents.pricing).
answer: >-
  Sign in on openagents.com, open Settings from the account menu at the
  bottom left, and choose **Claude credential**. There you can add or
  remove your own Anthropic API key, or a Bedrock, Vertex, or Foundry
  credential. We keep it encrypted for your account and use it only for
  Claude Code runs on your own environments, never in a chat, a saved
  image, or a log; usage bills to your own Anthropic or cloud account.
  Never paste a key into a chat.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/web/cloud-reset.md
    - crates/openagents-web/src/cloud/byo.rs
    - docs/cloud/environments-local.md
    - docs/cloud/claude-code-byo.md
evidence:
  - "2026-10-09: written from the cited documents and code and checked against them (#11041, #11052, chat goldens); the answer text awaits the owner's copy review."
---

## Answer

Sign in on openagents.com, open Settings from the account menu at the bottom left, and choose **Claude credential**. There you can add or remove your own Anthropic API key, or a Bedrock, Vertex, or Foundry credential. We keep it encrypted for your account and use it only for Claude Code runs on your own environments, never in a chat, a saved image, or a log; usage bills to your own Anthropic or cloud account. Never paste a key into a chat.

## Details

- The page is `/settings/claude`. One credential is kept at a time; adding another replaces it.
- A saved credential is kept for up to 90 days unless you add it again. Removing it takes effect at the next run's start.
- For a run, it is decrypted in memory for that run only and never logged.
- We never meter, pay for, or resell its usage.
- A key pasted into a chat is treated as exposed: we won't use it, and we suggest rotating it.

## Sources

- `docs/web/cloud-reset.md`
- `crates/openagents-web/src/cloud/byo.rs`
- `docs/cloud/environments-local.md`
- `docs/cloud/claude-code-byo.md`
