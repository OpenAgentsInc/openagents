# Write a plugin

There are two ways to make a plugin: in chat, or as a directory of files.

## In chat

In the OpenAgents app, ask "Help me make a plugin that ...". We draft the
plugin and its tests with you one step at a time, try it once, and then
offer to run the full tests on our computers. A plugin made in chat is a
skill, and it can turn on plugins we already have, such as Project map. A
plugin that needs new code goes to Coder on a computer you connected.

## As files: Explain this error

*Explain this error* is a worked example to copy. You paste a failing
command's output; it finds the file and line in your project the output
points at, shows the code, and says the likely cause and a likely fix. It
lives in one directory:

| File | Part |
| --- | --- |
| `src/lib.rs` | The Wasm: one operation, `explain`, written in Rust against `plugin-pdk`. |
| `programs/explain-error.json` | The workflow: one step that hands the Wasm your request and lets it read the files the request names, at most 16. |
| `package.json` | The plugin's record: its name, a one-line summary, the publisher's key, and the workflow it uses, pinned by hash. |
| `evals/` | The tests: five where it should help, two where it should stay out of the way. |

To make your own from it, in a checkout of the
[openagents repository](https://github.com/OpenAgentsInc/openagents):

1. Copy `crates/plugin-explain-error` to `crates/plugin-NAME` and rename
   the crate, the operation, the step, and the slugs.
2. Replace `explain` in `src/lib.rs` with your operation. The native tests
   run it against files in memory, in a second.
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
- **Say what you couldn't do.** Each example reports what it read, what it
  left out, and what it couldn't check, rather than guessing.

Skills are Markdown files in `skills/` (`skills/NAME.md`), and knowledge
entries are published with `openagents kb publish`. The full guide, with
the other two examples, is on
[GitHub](https://github.com/OpenAgentsInc/openagents/blob/main/docs/plugins/README.md).

Next: [Test a plugin](/docs/test-a-plugin).
