---
id: openagents.traces
version: 1
kind: product
title: "Uploading agent traces to your account"
summary: >-
  `coder trace upload --last` uploads a Coder chat to the signed-in
  openagents.com account as a trace, with passwords and keys taken out;
  it's private unless shared, and Settings → Traces lists, shares, and
  deletes them.
tags: [coder, traces, trace, atif, upload, share, export, settings]
applies_when: >-
  The user asks how to upload, save, keep, view, share, unshare, or delete
  an agent trace or a Coder chat's trace on openagents.com, what `coder
  trace upload` or `coder trace list` does, whether an uploaded trace is
  private, where traces show on the website, or what a trace link at
  openagents.com/trace is. Not how to sync chats to the account
  (openagents.coder-sync) or export a chat to a local file.
answer: >-
  Sign Coder in with `coder login`, then run `coder trace upload --last` for
  your latest chat (or `coder trace upload SESSION_ID`, or `--file PATH` for
  any ATIF file). Coder takes out passwords, keys, your home folder's name,
  and email addresses first and says how many. The trace is private: open
  it in Settings → Traces on openagents.com, where Share gives it a public
  link and Delete removes it. `--share` shares it as you upload, and `coder
  trace list` lists yours.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/content/docs/traces.md
    - crates/openagents-web/src/traces.rs
    - crates/coder-new/src/trace_upload.rs
evidence:
  - "2026-10-09: written from the cited documents and code and checked against them (#11109); the answer text awaits the owner's copy review."
---

## Answer

Sign Coder in with `coder login`, then run `coder trace upload --last` for your latest chat (or `coder trace upload SESSION_ID`, or `--file PATH` for any ATIF file). Coder takes out passwords, keys, your home folder's name, and email addresses first and says how many. The trace is private: open it in Settings → Traces on openagents.com, where Share gives it a public link and Delete removes it. `--share` shares it as you upload, and `coder trace list` lists yours.

## Details

- A trace is the chat's whole record in the ATIF format: messages, each step the agent took, and its result.
- Uploading the same trace twice keeps one copy. A trace can be up to 8 MB, and an account keeps up to 100.
- The website checks again and refuses a trace that still looks like it holds a key.
- A shared trace's link is `openagents.com/trace/` and its id; Stop sharing turns the link off.
- `openagents coder trace upload` and `openagents coder trace list` run the same commands.

## Sources

- `crates/openagents-web/content/docs/traces.md`
- `crates/openagents-web/src/traces.rs`
- `crates/coder-new/src/trace_upload.rs`
