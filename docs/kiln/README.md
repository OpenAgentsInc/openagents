# Kiln tech demo

Kiln is a coding agent that runs entirely in a browser tab. This demo
validates the hard part of the design in
[`docs/research/browser-agent/README.md`](../research/browser-agent/README.md):
a sandboxed workspace, real tool programs running in the page, an agent loop
that drives them, and inference from Pylons with no vendor key. It is a tech
demo, not a product. Issue: #12432.

## What the demo shows

You open the page and type a task, such as "write `fib.rhai` with a test and
make it pass". Then:

1. The page generates a fresh Nostr key for the agent. The key never leaves
   the tab.
2. The agent connects to `wss://relay.openagents.com`, reads the Pylon
   beacons, and sends its prompt as a NIP-44 encrypted NIP-CJ job to a
   Pylon serving `deepseek-v4.1-flash`. If no Pylon serves that model, it
   falls back to any Pylon.
3. The model answers with JSON commands: write a file, patch a file, or run
   a tool.
4. Each command runs as a `wasm32-wasip1` program in a Web Worker against
   the workspace filesystem: `write`, `patch`, `ls`, `cat`, `grep`, `rhai`
   (a Rhai interpreter), and `test` (a test runner).
5. The agent reads what the tools printed and takes the next step until the
   tests pass and it says it's done.
6. The workspace is saved to the Origin Private File System (OPFS) after
   every change, so a reload brings it back.

The **Sandbox self-test** button runs the tools without a model: it writes a
module with a planted bug and a test, runs the test (which fails), finds the
bug with `grep`, fixes it with `patch`, runs the test again (which passes),
and runs a script.

## Run it

```sh
scripts/build-kiln.sh --serve 8787
open http://127.0.0.1:8787/
```

The script builds the tools for `wasm32-wasip1`, the page module for
`wasm32-unknown-unknown`, and `kiln-dev`, the local server. It writes the
site to `$KILN_OUT` (default `$CARGO_TARGET_DIR/kiln-site`) and, with
`--serve PORT`, serves it on `127.0.0.1:PORT`. It needs the two WebAssembly
targets, `wasm-bindgen` 0.2.128, and a clang with the WebAssembly target for
`secp256k1` (Homebrew's `llvm`).

To serve an existing build:

```sh
$CARGO_TARGET_DIR/debug/kiln-dev --dir $CARGO_TARGET_DIR/kiln-site --port 8787
```

Two query parameters help automated checks: `?selftest=1` runs the
self-test on load, and `?task=TEXT&auto=1` runs the agent on `TEXT`. The page
sets `window.kilnSelftest`, `window.kilnAgentDone`, and
`window.kilnMetrics` for a driver to read.

Tests: `cargo test -p kiln -p kiln-tools`.

## Architecture

```text
 page (main thread, kiln.wasm)                       tool worker (kiln.wasm)
 ┌──────────────────────────────────────┐            ┌─────────────────────────┐
 │ agent loop (kiln::agent)             │  snapshot  │ WASI host (kiln::wasi)  │
 │ workspace (kiln::vfs) ── OPFS file   │ ─────────▶ │  over kiln::vfs         │
 │ tools: fetched by SHA-256, compiled  │  + module  │ runs one wasm32-wasip1  │
 │ NIP-CJ buyer (kiln::cj) ─ WebSocket ─┼─▶ relay    │ tool's _start           │
 └──────────────────────────────────────┘ ◀───────── └─────────────────────────┘
                                          output + new snapshot
```

- **`crates/kiln`** is one crate with portable modules and a browser layer:
  - `vfs`: the workspace, a tree of directories and files under one root.
    Paths can't climb above the root. A compact snapshot format carries it
    to OPFS and to the worker.
  - `wasi`: a WASI preview 1 host in plain Rust. `Process::call` takes an
    import's name, its arguments, and the guest's memory. It implements
    arguments, environment, standard streams, files and directories, clocks,
    random bytes, and exit. Everything else returns `ENOSYS`.
  - `agent`: the Microcoder-style loop: the prompt (task, files, recent
    steps, kept within the Pylon's input limit), the reply parser, and the
    command-to-tool mapping. It has no I/O.
  - `cj`: the NIP-CJ buyer without I/O: the page's key, NIP-42 sign-in,
    beacon choice (the preferred model first, then any Pylon), the sealed
    `25900` request with `pylon.deadline.v1` when the Pylon declares a
    longer deadline, and reading `27000` feedback and the `26900` result.
  - `web` (`wasm32` only): the page, the worker entry, OPFS, and the relay
    WebSocket, through `wasm-bindgen`, `js-sys`, and `web-sys`.
  - `kiln-dev`: a standard-library HTTP server that sends
    `Cross-Origin-Opener-Policy: same-origin` and
    `Cross-Origin-Embedder-Policy: require-corp`.
- **`crates/kiln-tools`** holds the tools as small `std` Rust programs. The
  build packages each one as `tools/<sha256>.wasm` with a `manifest.json`;
  the page checks every hash before it compiles a tool.
- **JavaScript** is limited to the `wasm-bindgen` glue it generates, a
  six-line `worker.js`, a four-line module script in `index.html`, and a
  one-line shim per WASI import, created at run time, that forwards
  arguments and turns `proc_exit` into a thrown error.

### Why a snapshot and not `SharedArrayBuffer`

A WASI program expects blocking calls. Wanix keeps one filesystem in a
kernel and has each worker block on `Atomics.wait` while the kernel answers.
For a demo, Kiln does something simpler: the page posts the workspace
snapshot with the compiled module, the worker runs the program against its
own copy, and it posts back the output and the new snapshot. The page applies
the difference and saves it. Every call is synchronous inside the worker,
and no call waits on the page. The cost is that two tools can't share a
filesystem while both run; see [findings](findings.md).

## Status and documents

- [findings.md](findings.md): what works, and what's still fake or
  missing.
- [measurements.md](measurements.md): measured numbers.
- [rust-tests.md](rust-tests.md): a crate from this repository in the tab,
  with `cargo test` on a sandboxed runner over Nostr.
- [transcript.md](transcript.md): the agent run drawn with the OpenAgents
  app's transcript, standalone and as an overlay in the Grid.
- [production.md](production.md): serving the page under a path prefix
  such as `/admin/kiln/`, and how it reaches a runner over the relay.
- [workstation.md](workstation.md): the spec for the Kiln workstation in the
  Verse, and NIP-34 patches and pull requests from it.
