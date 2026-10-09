# Write a plugin

There are two ways to make a plugin: in chat, or as a directory of files.

## In chat

In the OpenAgents app, ask "Help me make a plugin that ...". We draft the
plugin and its tests with you one step at a time, try it once, and then
offer to run the full tests on our computers. A plugin made in chat is a
skill. A plugin that needs new code goes to Coder on a computer you
connected.

## As files

A plugin with code lives in one directory:

| File | Part |
| --- | --- |
| `src/lib.rs` | The Wasm: one operation, written in Rust against `plugin-pdk`. |
| `programs/NAME.json` | The workflow: the steps that hand the Wasm your request and say which files it may read. |
| `package.json` | The plugin's record: its name, a one-line summary, the publisher's key, and the workflow it uses, pinned by hash. |
| `evals/` | The tests: tasks where it should help, and one or two where it should stay out of the way. |

To make one, in a checkout of the
[openagents repository](https://github.com/OpenAgentsInc/openagents):

1. Create `crates/plugin-NAME` with that layout. The `crates/plugin-*`
   directories already there are the test runner's fixtures and show the
   shape.
2. Write your operation in `src/lib.rs`. Native tests run it against
   files in memory, in a second.
3. Add `NAME` to `scripts/build-plugin-guests.sh` and run
   `./scripts/build-plugin-guests.sh NAME`. It builds the Wasm, puts it in
   the workflow, and updates the hash in `package.json`.
4. Try it once on a project, with reads only:
   `openagents plugin run crates/plugin-NAME --in PROJECT --request "..."`.

The `openagents` command is built from the same repository
(`cargo build --release -p openagents-cli`).

## Good habits

- **Grant the least you can.** Name the files the Wasm reads. A grant
  holds at most 1,024 files, and one call reads at most 64 KiB.
- **Write the summary as a request.** The workflow's summary is what Coder
  reads when it decides whether a request is for your plugin.
- **Say what you couldn't do.** Report what the plugin read, what it
  left out, and what it couldn't check, rather than guessing.

Skills are Markdown files in `skills/` (`skills/NAME.md`), and knowledge
entries are published with `openagents kb publish`. The full guide is on
[GitHub](https://github.com/OpenAgentsInc/openagents/blob/main/docs/plugins/README.md).

Next: [Test a plugin](/docs/test-a-plugin).
