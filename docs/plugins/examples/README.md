# Example plugins

Three plugins that do useful work and that you can copy to make your own
([#10086](https://github.com/OpenAgentsInc/openagents/issues/10086)):

| Plugin | What it does | Directory |
| --- | --- | --- |
| [Explain this error](explain-this-error.md) | Reads a failing command's output, finds the file and line in your project it points at, shows the code, and says the likely cause and a likely fix. | [`crates/plugin-explain-error`](../../../crates/plugin-explain-error/) |
| [Release notes](release-notes.md) | Groups the commits between two releases into user-facing release notes (breaking changes, features, fixes), each line citing its commit. | [`crates/plugin-release-notes`](../../../crates/plugin-release-notes/) |
| [Dependency check](dependency-check.md) | Reads manifests and lockfiles offline and flags duplicate versions, loose or unpinned version ranges, and licenses your declared policy doesn't allow. | [`crates/plugin-dependency-check`](../../../crates/plugin-dependency-check/) |

Each one is a piece of Wasm, the workflow that runs it, and a test set.
None needs a skill or knowledge; [Add a skill](#add-a-skill) says where
one goes. [Plugins](../README.md) describes every part a plugin can have.

## The layout to copy

```text
crates/plugin-explain-error/
├── Cargo.toml                    the Wasm crate: cdylib, depends on plugin-pdk
├── src/lib.rs                    the Wasm: one handler, native tests against MemoryHost
├── fixtures/                     files the native tests read
├── programs/explain-error.json   the workflow: a definition and a host binding
├── package.json                  the plugin's record: name, summary, publisher, the workflow it pins
└── evals/<test>/                 the test set: prompt.md, graders/, fixtures/
```

### The Wasm

The Wasm is ordinary Rust compiled to `wasm32-unknown-unknown`. It
exports one handler with `plugin_pdk::export_guest!`, which takes the
request (the operation and its input) and a `Host`, and returns a JSON
value or a refusal. `plugin_pdk::guest::{list, read}` read the files the
workflow grants. `plugin_pdk::guest::MemoryHost` answers the same calls
from memory, so the logic is tested natively:

```rust
plugin_pdk::export_guest!(handle);

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "explain" => explain(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}
```

It runs in Coder's Wasm host with no network, clock, process, or write
access, under fuel, memory, read, and output limits. Return a `markdown`
field, and the reply Coder gives the person leads with it. Say what you
couldn't do: each example reports what it read, what it left out, and
what it couldn't check, rather than guessing.

### The workflow

The workflow is a NIP-PRG program file. Its `definition` names one
`module` step and pins the Wasm by digest; its `binding` tells this host
how to run the step:

```json
"binding": {
  "steps": {
    "explain_error": {
      "module": {
        "profile": "snapshot-read",
        "operation": "explain",
        "request": "text",
        "read_named": true,
        "input": {"max_frames": 8, "context_lines": 4},
        "bytes_base64": "…"
      }
    }
  }
}
```

| Binding field | What it does |
| --- | --- |
| `profile` | `pure` (no files) or `snapshot-read` (reads granted files). |
| `operation` | The operation the Wasm's handler answers. |
| `input` | The fixed part of the Wasm's input. |
| `request` | The input field that receives what the person asked, held to 32 KiB. |
| `read` | Workspace paths the Wasm may read; a missing one refuses the step. |
| `read_present` | Paths the Wasm reads when they exist, such as `Cargo.lock`. |
| `read_named` | `true`: the Wasm may also read the files the request names, and the files a named log names, at most 16. |

Grant the least you can. A step granted `.` on a large repository is
refused (a grant holds at most 1,024 files), and one call reads at most
64 KiB in all, so name what you read. The definition's `summary` is what
Coder reads when it decides whether a request asks for this workflow:
write it as the request a person would make.

### The plugin record

`package.json` names the plugin, says what it does in one sentence, names
its publisher key, and pins the workflow by the SHA-256 of its file's text
as a JSON string. `scripts/build-plugin-guests.sh NAME` builds the Wasm,
writes its build receipt to `crates/plugin/fixtures/NAME.receipt.json`,
inlines the bytes into the workflow, and restates the workflow's digest in
`package.json`.

### The test set

Each test is a folder under `evals/`: `prompt.md` (TOML front matter and
the prompt), `graders/*.md`, and `fixtures/` (the files the run's empty
workspace starts with). Write at least four tests where the plugin should
help (`should-fire`) and one or two where it should stay out of the way
(`should-not-fire`). Grade what the run found, mechanically where you can:
a regular expression for a fact that is only in the fixtures, never in the
prompt, so a run without the plugin can't pass by echoing the question.
[Extension evaluation](../../extensions/evaluation.md) has every field and
grader.

## Build, try, and test one

1. Build the Wasm and pin it:

   ```sh
   ./scripts/build-plugin-guests.sh explain-error
   cargo test -p plugin-explain-error -p plugin --test guests
   ```

2. Run the workflow once on a project, the way Coder runs it once it picks
   it, with reads only and nothing published:

   ```sh
   openagents plugin run crates/plugin-explain-error --in ~/code/shop --request-file failure.txt
   ```

3. Run the test set with and without the plugin, on your computer or, from
   the app, on our computers:

   ```sh
   openagents plugin test run crates/plugin-explain-error --trust --grant write
   ```

4. Publish the result to the Gym (`openagents plugin test publish`, or
   **Add to the Gym** on the phone). A result is public whatever it says.

## Add a skill

Put guidance in the plugin's `skills/` folder. The test run adds it to
Coder's instructions in the run with the plugin, and its digest is part of
what the result names.

## What the examples measured

Each page ends with its test result on our computers and a read-only run
on a scratch copy of a real repository.
