# Noncoding workbench slice: meeting notes to action items

Status: implemented and checked on 2026-10-06
([#10671](https://github.com/OpenAgentsInc/openagents/issues/10671)). The
execution below ran the real Wasm workflow on this Mac under a scratch home
from a Cargo test. It did not use the owner's host, home, or Keychain. No
Grid or standalone screenshot was taken; both mount the same
`terminal_core::Application`, so they open the same pages through the same
code.

## The task

Turn synthetic meeting notes ([`standup.md`](standup.md), no real meeting
data) into a list of action items, through an admitted plugin, with an
independent check of the output.

| Part | Where |
| --- | --- |
| Source material | `notes/standup.md` in the workspace, a copy of [`standup.md`](standup.md) |
| Component | The *Action items* plugin: [`crates/plugin-action-items`](../../../../crates/plugin-action-items/), Wasm built by `./scripts/build-plugin-guests.sh action-items` (receipt `crates/plugin/fixtures/action-items.receipt.json`) |
| Exact release | `a7cff3ee...:action-items` version `0.1.0`, package digest `sha256:7a9ece16789edd2a2b6bf8c0052a681f7e6d8871592a6bbdd25fb21b9da14c13` |
| Request and thread | `List the action items in notes/standup.md`, thread `terminal` |
| Route and execution record | `plugin` route admitted by `openagents_chat::route::admit`, dispatched by `openagents_chat::capability::dispatch`, journaled under `~/.openagents/routes/terminal.jsonl` |
| Output artifact | The run's JSON, kept by digest under `~/.openagents/route-artifacts/` |
| Checker | `openagents plugin use`'s `check`: re-reads each cited line from the request or workspace file and requires the item to quote it |
| Cost | `$0`: a local Wasm run with no model or paid call |

## Actual execution

`cargo test -p openagents-cli --bin openagents -- meeting_notes` with
`OPENAGENTS_PRINT_RECEIPT=1` installs the plugin from its crate into a
scratch layout, turns it on, and runs `use_plugin` with the real workflow
runner. [`use-receipt.json`](use-receipt.json) is its answer, with the
scratch path replaced by `SCRATCH_HOME`. The output digest differs per run
because the run records its scratch workspace path.

- `dispatched: ran`, `reuse: admitted`, `state: completed`, `check:
  verified`.
- Five items: three with owners and due dates, one unassigned, one owned
  through `@dana`; one done checkbox left out.
- Asking again with the same request answers `followed` and the journal
  still holds one run.
- Deleting the kept output makes `plugin inspect` show it as `missing`.

## Scripted fixture

`terminal-core`'s
`a_noncoding_plugin_turns_notes_into_one_checked_artifact_from_the_page`
drives the page: F12, then F2 lists the plugin by exact release; a typed
request and ENTER arm one use, ENTER confirms it, and the page shows the
reply and the run (`RUN ... completed release 0.1.0 (this release)`, `check
verified, output ... retained, cost $0.0000`). Closing and reopening the
page, and asking again, never runs it twice; a lost output shows as
`missing`.

## Failure and unsupported cases

- A misquoted item fails the check (`check_failed`); a cited file that is
  gone is `unverifiable`; a plugin output with no checker is `unchecked`
  and the answer says no checker ran
  (`the_check_fails_a_misquoted_item_and_leaves_other_outputs_unchecked`).
- Another version, rebuilt bytes, a plugin that is off or removed, or a
  revoked release is refused and nothing runs (#10664's tests).

## Limits

- Revocation is not checked at run time on this computer; `install` checks
  it against the registry.
- The plugin is not in the hosted eval runner's catalog and has no
  published release or test set yet.
