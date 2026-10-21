# Kiln demo findings

Short notes from building and running the demo on 2026-10-21, in headless
Chrome on the owner's Mac (through `openagents browser run`) against
`wss://relay.openagents.com`. Numbers are in
[measurements.md](measurements.md).

## What worked

- **The whole loop runs in one tab.** A fresh page key asked a DeepSeek V4.1
  Flash model group on a Pylon for each step; the agent wrote `fib.rhai`
  and `fib_test.rhai`, ran the tests in a worker, read the result, checked
  the files, and said it was done in three steps. A harder task (Roman
  numerals with a 300-number round trip) also finished with passing tests.
- **Ordinary Rust compiles to useful WASI tools.** The tools are plain `std`
  programs built for `wasm32-wasip1`; nothing in them knows about the
  browser. Rhai (MIT or Apache-2.0, pure Rust) gives the agent a language to
  write and test, and its file module resolver works over WASI, so a test
  can `import "fib"`.
- **Our crates carry over.** `nostr` signs, verifies, and encrypts in the
  page, and `nostr::pylon` parses and validates beacons there, so the
  buyer's choice matches `crates/pylon`'s rules without the Tokio parts.
- **No `SharedArrayBuffer` is needed for a demo.** Posting a snapshot and
  the compiled module to a worker and taking back the new snapshot keeps
  every WASI call synchronous and local.

## WASI host limits

- Preview 1 only, and only the calls Rust's `std` uses for files, streams,
  arguments, environment, clocks, random bytes, and exit. `poll_oneoff`,
  sockets, symlinks, and hard links return `ENOSYS` (`readlink` returns
  `EINVAL`). A program that sleeps or opens a socket fails.
- No threads (`wasm32-wasip1-threads` is not supported).
- Standard input is a buffer given before the program starts; there is no
  interactive input and no pipe between two programs.
- Each output stream keeps at most 256 KiB.
- Rights are not enforced: every descriptor has all rights. The sandbox
  boundary is the workspace root (paths can't climb above it) and the
  worker (a tool can't reach the page, the network, or OPFS; it sees only
  the imports Kiln gives it).
- A tool that runs past 30 seconds is stopped by terminating its worker.
- No existing crate fit as a browser WASI host. `wasmer-js` has Rust inside
  but ships as a JavaScript package; `browser_wasi_shim` is JavaScript. The
  host in `kiln::wasi` is about 500 lines and has native unit tests.

## OPFS behavior

- The page saves the whole workspace as one snapshot file,
  `kiln-workspace-v1.bin`, through `createWritable` on the main thread after
  every tool run that changed something. A reload restores it.
- Saves are a few milliseconds for a small workspace; the first open of
  OPFS on a new profile took 30 to 160 ms.
- The page asks for persistent storage (`navigator.storage.persist()`); a
  browser may still evict the data under quota pressure.
- When OPFS is unavailable (for example, some private windows), the page
  says so and keeps the workspace in memory.
- One snapshot file is fine for a demo. A real workspace needs per-file
  storage or a log, so a save doesn't rewrite everything.

## The worker model

- The page compiles each tool once (`WebAssembly.compile`) and posts the
  compiled module to a worker with each run; a module is structured
  cloneable, so the worker only instantiates it.
- One worker is kept and reused between runs. Its first run pays for loading
  the page's own `kiln.wasm` in the worker; later runs take about 1 ms
  plus the program's own time.
- The worker gets a copy of the workspace and returns the new one. Two tools
  running at once would each see their own copy, and the second result
  would overwrite the first. Kiln runs one tool at a time.
- Inference: for long-running programs, shells with pipes, or several
  programs sharing files, the next step is Wanix's design: one filesystem
  in a kernel worker, and task workers that block on `Atomics.wait`. The
  page is already cross-origin isolated, so `SharedArrayBuffer` is
  available.

## Pylon round trips

- The relay's NIP-42 challenge and the beacon query take a few hundred
  milliseconds. First contact (the Pylon's `processing` feedback) arrives
  in about 0.3 s.
- The DeepSeek V4.1 group head has one serial slot and a 600-second
  deadline, so Kiln sends `pylon.deadline.v1` with a 300-second budget.
  Answers took 24 to 61 seconds a step; that's most of each step's time.
- When the group's slot was taken, the first version fell back to a
  `qwen3.5-0.8b` Pylon, which repeated itself and wrote invalid JSON. Kiln
  now waits up to 90 seconds for the preferred model's slot and falls back
  only when no Pylon serves that model.
- The relay admitted a brand-new key for requests and answers; no
  allowlist or payment was needed.

## Still fake or missing

- No receipts: Kiln doesn't publish `3201` receipts or check Pylon
  standings (`check-fail` labels).
- No payment: only free Pylons. NIP-X402 isn't wired.
- No NIP-OA link from the player to the agent key; the key is
  per page load and anonymous.
- No hosted-door fallback; the Pylon path didn't block, so it wasn't
  needed.
- No relay rotation: one WebSocket for the whole run; a dropped connection
  is reopened on the next step, and an answer sent while it was down is
  lost.
- One language (Rhai). No shell, pipes, Git, or compiler.
- The prompt history is the last steps' output, clipped to the Pylon's
  input limit; there's no durable turn journal, so a reload mid-run loses
  the run (the workspace stays).
- The page module is about 2.2 MB uncompressed and isn't run through
  `wasm-opt`.

## The owner's run

The owner opened the page on the Mac and ran the default fib task. It
finished in 3 steps and 78 seconds, with 3 of 3 tests passing, every step
answered by the DeepSeek V4.1 group `DeepSeek-V4.1 w16-g5c`:

1. The model wrote `fib.rhai` and `fib_test.rhai` and ran `test`: three
   tests passed on the first try.
2. It ran `cat` on both files to check them.
3. It said it was done.

What this run shows, and what it doesn't:

- **Real:** the inference (a Pylon chosen from live beacons, with a key made
  in the tab), the file edits, and the test run, all of which happened in
  the tab, in WebAssembly workers, against the OPFS workspace.
- **Limited:** the only language is Rhai, a small scripting language
  embedded in the tools; there's no Rust, shell, or Git in the tab.
- **Limited:** the model wrote both the code and its tests, and the tests
  passed on the first run, so the run didn't exercise a fix loop. The
  self-test plants a bug to show fail, patch, and pass; the agent itself
  hasn't yet had to repair a failing test in a measured run.

## The Rust milestone (#12437)

- **The hybrid works.** The tab loads a crate at a commit, the agent edits
  real Rust there, and `cargo test` runs on `kiln-runner` inside
  `coder-boundary` and comes back over the relay in seconds once the
  runner's target is warm.
- **The model's window is the constraint.** The DeepSeek V4.1 group head
  refuses prompts over 2,872 tokens, so the agent can't see a 378-line
  file at once. Kiln now reads the head's `max_input_tokens`, asks again
  smaller when refused, and gives the agent `view PATH START END` so it
  reads ranges found with `grep`.
- **The model doesn't always follow the reply format.** Seen in real runs:
  two JSON objects in one reply (the next step planned ahead), commands
  written as tool objects (`{"view": PATH, "start": N, "end": M}`), and
  DeepSeek's own tool-call markup (`<｜DSML｜ calls>`) instead of JSON. The
  parser now takes the first object and reads tool objects; markup replies
  are skipped and cost a step.
- **Small models can't do this.** When the DeepSeek slot was taken, the
  first version fell back to `gemma-4-e4b` and `qwen3.5-0.8b`, which wrote
  invalid JSON every step. Kiln now waits for the preferred model and
  treats `rate_limited` as "wait", not as a broken pylon.
- **The runner's sandbox needed two reads it didn't expect:** Cargo reads
  the `config.toml` in every directory above the worktree (including the
  owner's `~/.cargo/config.toml`, which names the `kache` wrapper), and
  `xcrun` wants a cache file under `/var/folders`, which the boundary
  refuses with a harmless warning. The runner grants the config files
  read-only and switches the wrapper off.

## The NIP-34 demo milestone

From the Kiln page, an agent on DeepSeek V4.1 added a function and a test
to `crates/memory-stream`, got a green remote `cargo test`, and published
the change as a kind `1617` patch. The runner applied that exact patch
with `git apply`, tested it again, and signed a "tests pass" comment on it;
`review.html` shows the patch as reviewable. What's still missing for real
use, in phase order in [workstation.md](workstation.md#phases):

- The patch is signed by the page's throwaway key and labelled "unverified
  author"; player signing (NIP-07 or NIP-46) and agent mandates are phase 3.
- The repository announcement is the runner's demo announcement, not the
  owner's `openagents` one.
- No one can merge from Nostr yet: `openagents git apply EVENT` and the
  GitHub bridge are phases 4 and 5.
- The runner admits any key; it needs an admission list and quotas before
  a public page uses it.
