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
kiln-runner --state "$(openagents scratch)/kiln-runner" --jobs 4 --open
# The page, pointed at the runner's key.
scripts/build-kiln.sh
kiln-dev --dir "$CARGO_TARGET_DIR/kiln-site" --port 8787 --runner "$(cat "$(openagents scratch)/kiln-runner/pubkey")"
open "http://127.0.0.1:8787/?load=crates/memory-stream"
```

`kiln-runner --crates a,b` names the crates it tests (default
`memory-stream`, `xp-ledger`, `atif`).

## Which crates, and their limits

For a lasting runner, keep the crate list in a file an admin curates and
pass `--crates-file FILE`. The runner reads it again for every job, so a
crate can be added or dropped without a restart:

```text
# kiln-runner crates: NAME [timeout=SECS] [memory=MB]
memory-stream
xp-ledger
atif timeout=300
nostr timeout=900 memory=6000
```

- A name is both the package and its directory under `crates/`; list only
  crates where the two match.
- `timeout` and `memory` cap a run that names the crate. A run that names
  several crates gets the largest of their caps, never more than the
  runner's own `--timeout` (default 600 s) and `--memory` (default
  8192 MB).
- The memory cap covers the run's whole process tree: the runner sums its
  children's resident memory twice a second and stops the run past the
  cap, with "stopped: the run passed its N MB memory cap".
- Add a crate only after its tests pass offline in the boundary on the
  runner's machine, and keep crates with network or GPU tests off the list.

## Changes that touch several crates

Load several crates at once (`?load=crates/a,crates/b`, or a comma list in
the field). The page tests and publishes them together: the patch carries a
`t` tag `crate:NAME` for each crate it changes, and its runner check runs
`cargo test -p a -p b`. The runner refuses a patch run that leaves out a
crate the patch changes, so a passing check always covers the whole
patch.

## Who may run jobs

The runner refuses to start until you say who may use it:

- `--allow FILE` names the keys that may run jobs, one a line, hex or
  `npub`, each with an optional daily quota: nothing for the member quota
  (`--member-jobs`, default 30), `jobs=N`, `unlimited`, or `each=N` for a
  key that vouches for many agents (the site's Kiln key): each agent gets
  its own N jobs a day, counted apart from the key's own. The runner reads
  the file again for every job, so you can add or remove a key without a
  restart.
- An agent key gets in when its job request carries a valid NIP-OA `auth`
  tag from a listed key, and draws from that key's quota.
- Keys on the self-edit admin list (`--admins`) are always unlimited.
- `--open` also takes jobs from any other key, each within the anonymous
  quota (`--anonymous-jobs`, default 5 a day). Use it only for a local demo.

A key that isn't let in gets "this runner only takes jobs from keys on its
list; ask an admin to add npub1...". Counts are per UTC day and live in
`STATE/quota.json`.

```text
# kiln-runner admission list
npub1...              # the member quota
npub1... jobs=200
npub1... unlimited    # the Grid Foreman, maintainers
```

## Many runners, chosen like Pylons

Workstation phase 7 (#12457). A runner announces itself the way a Pylon
does, and a Kiln page chooses among runners the way it chooses Pylons:

- **Beacon.** Every 60 seconds the runner publishes a NIP-PYLON `30200`
  beacon in the runner lane (`t` `oa:kiln-runner-beacon:v1`, service
  `<key>:kiln/cargo-test`), naming its repository, crates, admission, and
  whether its one slot is free. `--name SLUG` and `--label TEXT` name it;
  `--no-beacon` keeps it quiet. See NIP-PYLON, "Kiln runners".
- **Choice.** With no runner set under **Settings** (or in the host's
  `config.json`), the page reads runner beacons once a minute and sends a
  job to the best runner that builds every crate the job names, for its
  repository: free first, then the newest beacon. When a runner refuses
  (busy, quota, not listed) or doesn't answer, the page tries the next, up
  to three, and skips the one that failed for five minutes. A runner set
  under Settings is used as before, alone.
- **Receipts.** After each job the page publishes a NIP-PYLON `3201`
  receipt signed by its key: one job, free, and accepted, rejected, or timed
  out, with the digests of the request and the result. Runner work is
  counted the way Pylon work is.
- **Builds per tenant.** Each tenant (the key a job counts against: the
  requester, or the owner of an authorized agent) builds in its own folder
  under `STATE/targets/`, so one tenant's build output can't feed another's
  run. The runner keeps the six most recently used (`--tenant-targets N`)
  and removes older ones. A tenant's first run on a runner is a cold build.

Defaults, by decision for this phase: runs are free (`free-v1`, no
NIP-X402 price), admission and quotas are each runner's own list and
quotas as above, and a runner's check comment on a patch still counts only
when the runner is on the maintainers' list.

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

## Publishing the change (phase 2)

When `cargo test` passes, the agent sends
`{"publish": "summary", "body": "why"}`. The page:

1. refuses unless the crate's files are exactly those of the last passing
   `cargo test`;
2. builds `git format-patch` text from the untouched copy and the
   workspace (`kiln::patch`), and refuses it if it holds a key-shaped
   string or passes NIP-34's 60 KB patch size;
3. signs a kind `1617` patch with the page's key, with `a` naming the
   repository, `p` its owner, `t` `root` and `crate:NAME`, and
   `parent-commit` the base, and publishes it;
4. sends the runner a job that carries the signed patch instead of files.
   The runner checks the signature and that the patch names its
   repository and touches only the allowed crate, applies the text with
   `git apply`, runs `cargo test`, and publishes a NIP-22 comment (`1111`)
   on the patch: pass or fail, the summary, the base, and the patch's
   SHA-256.

`review.html` reads the patches to the repository back from the relay and
shows each one's subject, author, diff, the runner's check, comments, and
status. It counts checks and status only from the repository's
maintainers: the announcement's author and its `maintainers` tag.

For the demo, the runner publishes a kind `30617` announcement under its
own key with `d` = `openagents-kiln-demo`, so demo patches don't land on
the owner's future `openagents` announcement. `kiln-dev --repo ADDRESS`
tells the page and the viewer which repository to use.
