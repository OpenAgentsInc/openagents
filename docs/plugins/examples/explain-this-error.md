# Explain this error

*Explain this error* reads a failing command's output, finds the file and
line in your project it points at, shows the code there, and says the
likely cause and a likely fix. It's one of the
[example plugins](README.md): copy it to make your own.

```text
You:   cargo build fails with this. What is wrong?
       error[E0308]: mismatched types
        --> crates/probe-core/src/redact.rs:24:28
       ...
Coder: **Error:** `E0308: mismatched types`
       **Where:** `crates/probe-core/src/redact.rs:24`, in `register`
         22 |     pub fn register(&mut self, secret: impl Into<String>) {
         23 |         let secret = secret.into();
       > 24 |         if secret.len() >= "8" && !self.values.contains(&secret) {
       **Likely cause:** The value here has a different type than the code
       expects (expected `usize`, found `&str`).
       **Likely fix:** Convert the value …, or change the declared type so
       both sides agree.
```

## What's in it

| Part | File | What it does |
| --- | --- | --- |
| Wasm | [`src/lib.rs`](../../../crates/plugin-explain-error/src/lib.rs) | The `explain` operation: reads the output, picks the first error, matches its frames to the files it may read, reads the responsible one, and explains it from a table of error kinds per language. |
| Workflow | [`programs/explain-error.json`](../../../crates/plugin-explain-error/programs/explain-error.json) | One `module` step that hands the Wasm the request (`"request": "text"`) and lets it read the files the request names (`"read_named": true`). |
| Plugin record | [`package.json`](../../../crates/plugin-explain-error/package.json) | The name, the one-line summary, the publisher, and the workflow's digest. |
| Tests | [`evals/`](../../../crates/plugin-explain-error/evals/) | Five tests where it should help and two where it should stay out of the way. |

## What it reads

The output comes from the request, so you paste it, or from a saved log
the request names (`the output is in ci/test-output.log`). The workflow
grants the Wasm the files the request names and the files a named log
names, at most 16, and only ones inside the workspace, so it never needs
the whole repository: it works the same on a project with a million
files.

It recognizes, from the text and never from a file name:

- `rustc` errors (`error[E0308]` and ` --> path:line:col`) and Rust panics;
- Python tracebacks, and the `path:line: Error` lines pytest prints;
- JavaScript stack traces from Node.js and the browser, including
  `file://` paths;
- TypeScript (`tsc`) errors in both of its formats;
- Go compiler errors and panics with their goroutine traces;
- Java exceptions with their `at` frames;
- the `path:line:col: error: message` shape of gcc, clang, and most
  linters.

It explains the first error and counts the rest. In a stack trace it
picks the innermost frame that is your code, so a frame in the standard
library or `node_modules` isn't blamed. A path printed on another machine,
such as a CI runner's `/home/runner/work/…`, still finds the file here by
its trailing components.

## How it explains

The explanation is deterministic: a table of error kinds per language,
plus what the code adds. For example:

- A missing name (`NameError`, `E0425`, `ReferenceError`, `undefined:`)
  lists the names in the file that are close to it, including
  abbreviations (`qty` and `qty_ordered`).
- A `KeyError` lists the keys the file uses and the one closest to the
  missing key.
- `Cannot read properties of undefined (reading 'discount')` names the
  expression on the line that was undefined (`order.loyaltyAccount`).
- An index out of range looks for a loop bound with `<=` against a length
  near the line.
- A division by zero names the divisor on the line.

When it doesn't know an error kind, it says the message and points at the
line, and it never invents a cause. It says when the file has fewer lines
than the output names (the file changed since the command ran), when it
read only part of a file, and when the output names no file it could read.

## Copy it

To make a plugin of your own from this one:

1. Copy `crates/plugin-explain-error` to `crates/plugin-NAME` and rename
   the crate, the operation, the step, and the slugs.
2. Replace `explain` in `src/lib.rs` with your operation. Keep the native
   tests: they run against `MemoryHost` in a second.
3. Add `NAME` to `scripts/build-plugin-guests.sh` and run
   `./scripts/build-plugin-guests.sh NAME`.
4. Try it on a project: `openagents plugin run crates/plugin-NAME --in PROJECT
   --request "…"`.
5. Write the test set, run it with `openagents plugin test run`, and publish
   the result whatever it says.

## Read-only run on a real repository

On 2026-10-01, on a scratch copy of the owner's `probe` repository
(`git archive HEAD`; the real checkout was only read), one comparison was
changed to `secret.len() >= "8"` in `crates/probe-core/src/redact.rs`, and
`cargo check -p probe-core` failed with `E0308`. `openagents plugin run
crates/plugin-explain-error --in SCRATCH --request-file failure.txt` named
`crates/probe-core/src/redact.rs:24`, in `register`, showed lines 20 to 28,
and gave the expected and found types from the compiler's notes. The
release gate runs the same path on a planted Python failure
([`explain-error`](../../release/acceptance.md#scenarios)).
