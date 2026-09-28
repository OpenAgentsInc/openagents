# Microluna (deprecated)

Microcoder replaced Microluna as Coder's loop on 2026-09-25 (issue
[#9666](https://github.com/OpenAgentsInc/openagents/issues/9666)), and the
owner deprecated Microluna on 2026-09-28 (issues
[#9878](https://github.com/OpenAgentsInc/openagents/issues/9878) and
[#9880](https://github.com/OpenAgentsInc/openagents/issues/9880)). Don't
build new work on this crate.

- Coder's terminal and `coder -p` answer through the Microcoder loop in
  `crates/microcoder-loop`; see
  [the delegate door](../../docs/coder/runtime/delegate-door.md).
- The Codex login and transport, list prices, the one-tool-call
  `oneshot`, and the scripted fake transport moved to
  `crates/codex-transport`. This crate re-exports them under their old
  paths.

## Why it's still in the workspace

The crate stays in the workspace build so that recorded Terminal-Bench
evidence stays reproducible. Coder One's mini-handoff loop
(`coder_one::micro`) and its `microluna-*` policies in
`crates/coder-one/policies/` run Microluna sessions, and so does the
`microluna` binary. Removing the crate would leave those records with no
code that reproduces them.

Coder One is the only crate that depends on it. Coder and Verse reach it
only through Coder One, and neither runs a Microluna session.

## What it was

Short GPT-6 Luna sessions on the operator's Codex login, with five native
function tools (run a command, read a file region, apply a patch, write a
file, and finish) under `coder-boundary` and `supervise`, each reply and
call recorded as an ATIF step with usage and list-price cost. See
[the Microluna design](../../docs/coder/design/microluna.md).
