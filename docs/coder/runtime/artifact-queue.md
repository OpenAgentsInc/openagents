# The artifact queue

Some files in this repository are one digest over many sources. The
Everglade pack is one `.vtp` file pinned by `PACK_SHA256` in
`everglade_pack.rs`, and the Grid pack's fingerprint is
`assets/verse/grid/pack.json`. When two agents each change a source and
repin, the second push has to rebase and repin again, and a repin made on a
stale `main` drops the other agent's change from the pack.

The artifact queue serializes those changes, like a merge queue. An agent
submits a branch, and the queue lands submitted changes one at a time onto
current `origin/main` with a single repin per batch. It replaces the old
rule to rebase right before pushing and repin once. Issue #10763 holds the
design, and `crates/coder-lease/src/artifact/` holds the code.

## Submit a change

Commit your change on a branch in any checkout of the repository, then
submit it:

```sh
openagents artifact submit everglade-pack --summary "the belvedere"
```

- `--branch B` names the branch; the checked-out branch is the default.
- `--summary TEXT` names the change in the repin commit:
  `Repin the Everglade pack with the belvedere`. The branch name is the
  default.
- `--no-run` only queues the change.

The submission queues the commits on the branch that `origin/main` lacks.
If no other process runs the queue, `submit` then runs it, and reports
whether your change landed or why it was rejected. It exits 1 when your
change was rejected. When another process holds the queue's lease, that
process lands your change, and `submit` returns at once.

You don't need to repin on your branch. If you do, the queue leaves your
pinned files and pin lines out and writes its own.

## Watch the queue

```sh
openagents artifact queue            # pending changes, oldest first
openagents artifact queue --all      # with landed and rejected ones
openagents artifact queue everglade-pack --json
```

A rejected change keeps its reason: the files that conflict, or the last
lines of the failing regenerate or check command. Rebase the branch on
`origin/main`, fix it, and submit it again.

## What a run does

`openagents artifact run NAME` runs the queue. `submit` runs it too. The
run takes the exclusive `artifact/NAME` lease without waiting. When another
process holds the lease, that process applies the pending changes. A run
does the following:

1. Fetches `origin/main` into a scratch worktree under the lease root,
   `artifacts/NAME/worktree`.
2. Applies each pending change in submission order as one commit, with its
   author and message. A change's edits to pinned files are left out, and
   so are its pin lines and digest history. A change that conflicts with
   `main`, or with an earlier change in the batch, is rejected.
3. Runs the artifact's regenerate command once, writes the pin lines it
   prints, and moves the old digest into the history.
4. Runs the artifact's check. If the batch fails it, the run applies the
   changes again one at a time, regenerating and checking after each, and
   rejects the ones that fail.
5. Commits the repin and pushes to `main`. If the push is refused because
   `main` moved, the run rebuilds the batch on the new `main`, up to three
   times.

The run keeps going until nothing is pending. After it releases the lease,
it looks at the queue once more, so a change submitted during the run
can't be left behind.

The regenerate and check commands run under a `build` lease when the
registry asks for one. They keep your `CARGO_TARGET_DIR` when it's set, and
otherwise build in `artifacts/NAME/target` under the lease root.

## The registry

Each queue-managed artifact is one file, `artifacts/NAME.json`, at the top
of the repository. The run reads the registry from the `main` it fetched,
so a submitted branch can't change the commands that check it.

| Field | Meaning |
| --- | --- |
| `schema` | `openagents.artifact.v1`. |
| `name` | The artifact's name, which is also the file's stem. |
| `regenerate` | The shell command, run at the top of the repository, that rebuilds the artifact and prints the pin lines. |
| `check` | The shell command that fails unless the sources build the pinned artifact. |
| `build_lease` | Whether the two commands run under a `build` lease. |
| `pinned` | Path patterns of the files that the regenerate command writes. `*` matches within one path segment. |
| `pin` | Optional. The source `file` whose `lines` (line prefixes) hold the pin, and the `history` list that keeps earlier digests. |
| `message` | The repin commit's subject. `{changes}` becomes the summaries. |

The registry has three entries:

- `everglade-pack` runs
  `cargo run --release -p verse --example everglade_pack -- assets/verse/everglade`
  to regenerate, and the same command with `--check` to check. It pins
  `assets/verse/everglade/*.vtp`, plus `PACK_SHA256` and `PACK_BYTES` in
  `crates/verse-zone-everglade/src/zones/everglade_pack.rs`, with
  `EVERGLADE_PACK_HISTORY` as the history.
- `grid-pack` runs `cargo run -p verse --example grid_pack` to regenerate,
  and the `the_pinned_pack_is_what_the_sources_compile_to` test in `verse`
  to check. It pins `assets/verse/grid/pack.json` and the white texel
  beside it.
- `everglade-kit` runs `scripts/unreal/medieval_kit_build.py` and the
  `everglade_kit` example in `verse-zone-everglade` to regenerate, and the
  example with `--check` to check. The kit pack is licensed, so the entry
  pins no file, only `KIT_SHA256` and `KIT_BYTES` in
  `crates/verse-zone-everglade/src/zones/everglade_pack/kit.rs`, and it runs
  only on a machine with the private export
  (`docs/verse/everglade-medieval-refactor.md`).

A generated fixture or a lockfile can join the queue with a file of its
own.

## Where the queue lives

Under the lease root, `~/.openagents/leases` by default, or
`OPENAGENTS_LEASE_ROOT`:

- `artifacts/NAME/queue/ID.json` is one submission: the branch, its commit,
  the summary, the session, the status (`pending`, `landed`, or
  `rejected`), the reason for a rejection, and the commit on `main` that
  landed it.
- `artifacts/NAME/worktree` is the scratch worktree.

The submitting checkout keeps each pending commit alive under
`refs/artifact-queue/NAME/ID`, and the run deletes that ref when it decides
the change.

## Tests

`cargo test -p coder-lease artifact` runs the queue over a scratch
repository and a bare remote, with stand-in regenerate and check commands,
a temporary `HOME`, and a temporary lease root. The tests cover two
submissions that both repin landing as one repin, a conflicting
submission rejected with its reason, a change that fails the check, and
the pending list.
