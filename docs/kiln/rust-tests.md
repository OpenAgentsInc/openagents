# Rust tests from the browser

Kiln's second milestone (#12437): the agent works on a real crate from this
repository. The tab holds the crate's files and edits them; a runner on a
computer we control builds and tests the change, and the result comes back
over Nostr. Compiling the workspace in the browser isn't realistic (see
[the `rustc` experiment](workstation.md#experiment-rustc-in-the-tab)), so this is the
hybrid.

## How it works

```text
 tab                                         relay                 kiln-runner (this Mac)
 ┌───────────────────────────────┐                                ┌──────────────────────────────┐
 │ load crates/NAME at a commit  │                                │ own key, own state directory │
 │  (GitHub API + raw files)     │                                │ shallow clone + one worktree │
 │ agent edits files in OPFS     │  25900 sealed job: base commit │ checkout base, clean, write  │
 │ `cargo test -p NAME` ─────────┼──▶ changed files, cargo args ──▶│ files                        │
 │                               │                                │ cargo --offline inside       │
 │ progress line in the log  ◀───┼── 27000 processing + progress ◀│  coder-boundary: no network, │
 │ summary + log to the agent ◀──┼── 26900 result ◀───────────────│  confined reads and writes,  │
 └───────────────────────────────┘                                │  time cap, log cap           │
                                                                  └──────────────────────────────┘
```

1. **Load a slice.** **Load** (or `?load=crates/NAME&ref=REF`) resolves the
   reference to a commit with `GET /repos/OpenAgentsInc/openagents/commits/REF`,
   lists the directory with one `git/trees/COMMIT:crates/NAME?recursive=1`
   call, and fetches each file from `raw.githubusercontent.com` at that
   commit. Both send CORS headers, so the cross-origin-isolated page can read
   them. No token: two API calls per load, inside GitHub's 60-an-hour
   anonymous limit. The files land in the workspace at their repository
   paths, and a second OPFS snapshot keeps the untouched copy and the
   commit.
2. **Edit in the tab.** `ls`, `cat`, `grep`, `write`, and `patch` run as
   WASI tools on the crate's files, as before.
3. **`cargo test -p NAME`.** The page compares the workspace with the
   untouched copy and sends a `kiln::remote::Job`: the commit, the changed
   and deleted files, and the Cargo arguments, as the task of a NIP-CJ
   conversation request (`25900`) sealed to the runner's key. The page's
   key is its own fresh key. Only `cargo test -p CRATE [-- FILTER]` and
   `cargo check -p CRATE` are accepted, only for the runner's listed crates,
   only for files inside the crate, and never `Cargo.toml` or `build.rs`.
   Both ends check this.
4. **The runner** (`crates/kiln-runner`) holds its own key and state
   directory. For each job it fetches the commit into its shallow clone if
   it lacks it, checks it out in its one worktree, removes every change and
   untracked file, writes the job's files, and runs
   `nice -n 10 cargo test --offline -p CRATE` inside `coder-boundary`:
   - no network (`Spec::offline`);
   - writes only to the worktree, the runner's own target directory, and
     its own Cargo home, whose `registry` and `git` entries link to this
     computer's Cargo caches;
   - reads only the worktree, the Cargo caches, and the toolchains
     (`coder_boundary::Toolchains`), so a test can't read the owner's files
     and print them into its log;
   - a cleared environment (no tokens), no `RUSTC_WRAPPER`, and
     `CARGO_BUILD_JOBS=4`;
   - a 600-second wall-clock cap through `supervise`'s process-group wait,
     and a 32 MB log cap.

   It sends `processing` feedback with the log's latest line about every
   two seconds and then the result: Cargo's exit code, a one-line summary
   (test counts or compile errors), and the log's end without the
   `Compiling` and passing-test lines, at most 24 KiB.
5. **The agent** sees the summary and the log as the command's output and
   edits the Rust until `cargo test` passes.

The runner publishes no beacon, so no Pylon buyer sends it chat. The page
learns its key from `kiln-dev --runner KEY` (served as `/config.json`), a
`?runner=` parameter, or the **Test runner key** field.

## Run it

```sh
# The runner, with its own state directory (a shallow clone, about 3 GB with its worktree).
cargo build -p kiln-runner
kiln-runner --state "$(openagents scratch)/kiln-runner" --jobs 4
# The page, pointed at the runner's key.
scripts/build-kiln.sh
kiln-dev --dir "$CARGO_TARGET_DIR/kiln-web" --port 8787 --runner "$(cat "$(openagents scratch)/kiln-runner/pubkey")"
open "http://127.0.0.1:8787/?load=crates/memory-stream"
```

`kiln-runner --crates a,b` names the crates it tests (default
`memory-stream`, `xp-ledger`, `atif`).

## Reused, and not

- **Reused:** `coder-boundary` for the sandbox and its toolchain read list,
  `supervise` for the process-group deadline, `nostr-transport` for the
  runner's NIP-42 relay connection, and the NIP-CJ conversation kinds.
- **Not reused:** `eval-runner` is an execution worker whose one target is
  the `ext-eval` program, with a catalog, quota, and blob store; it would
  need a new target type and pulls in `coder`. `nostr::execution`'s
  `openagents.execution.v1` wants pinned definitions, a dependency lock,
  context and requirements manifests, and a durable claim; that's the right
  home for this once it graduates, but too much for the demo. There's no
  `crates/coder-worker`; the `coder-worker` binary in `crates/coder` answers
  conversation jobs through a model door.
