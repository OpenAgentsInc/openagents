# Execution boundary and workspace snapshots

Issue: [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427).
Status: the component is implemented in `crates/coder-boundary`, shared
capability trust has landed, and CoderBench compares independent workspace
snapshots. Coder's delegation boundary and admission integration, followed by
observed Devin runs, remain required before #9427 can close.

## What this is

Two pieces, deliberately independent of Coder and CoderBench:

- `coder_boundary::boundary` is an RAII write boundary around one
  command. On macOS it wraps the command in `sandbox-exec` with a
  Seatbelt profile that denies `file-write*` everywhere and then permits
  exactly the paths the caller named. On any platform without a backend,
  `Spec::build` returns `Error::Unsupported` — there is no construction
  that produces an unrestricted command, and no public setter can point
  the backend anywhere but `/usr/bin/sandbox-exec`.
- `coder_boundary::snapshot` observes a directory tree before and after
  a run and reports what changed: creation, removal, renames, retypes,
  and content changes — including to files that were already dirty,
  which is the case a `git status` comparison cannot see.

## The two policies

`Boundary::readonly()` permits writes only beneath paths named with
`Spec::writable` — private scratch and adapter state the executor is
trusted to keep — plus an optional `Spec::owned_scratch_under` directory
the boundary creates and owns. Its writable paths must be disjoint from
every denied path in both directions.

`Boundary::writing(checkout)` adds one isolated checkout to the writable
set. A delegate's worktree lives inside the checkout it branched from
(`.coder/worktrees/…`), so the profile expresses that exception by
ordering: every deny lands before every allow. The checkout is the only
path granted that exception, and only beneath a *protected* path. The
denied set divides in two:

- `Spec::protecting` — a path that stays unwritten but may contain the
  one isolated checkout: the main checkout.
- `Spec::sealed` — a path that accepts no exception at all: the common
  Git directory. No writable path may sit beneath a sealed path, and a
  writing boundary's checkout may not either. A sealed path inside the
  checkout is refused as well — a deny inside an allow is unreachable
  and would only pretend to seal.

A writable path equal to, nested under, or containing a protected or
sealed path is refused at `build`, in both policies. A protected path
nested under the checkout is refused for the same reason: the deny
would be unreachable.

All configured paths are resolved before the boundary exists. Every
one — protected, sealed, writable, checkout, and the scratch parent —
must already exist and canonicalize successfully. A canonicalization
failure of any kind is `Error::Resolve`, not a guess: the boundary never
walks up to a deepest existing ancestor and never constructs a missing
suffix, so a permission error, a dangling link, and a genuinely absent
path all refuse the same way. A caller that wants an adapter's state
directory writable creates the directory before it approves the
boundary. Symlinked aliases resolve to the real directory rather than
widening the grant, and a path that is not UTF-8 or that carries a
control character has no safe spelling in the profile and is refused
rather than quoted wrong.

## Confined reads

Added 2026-09-25 for the `checks.oracle` writer
([#9656](https://github.com/OpenAgentsInc/openagents/issues/9656)), whose
commands searched the host with `find /` at tier 0.

`Spec::confining_reads` limits what the command can read, and
`Spec::readable` names a path it can read but not write (naming one also
confines reads). A read-confined command can read only these paths:

- The system's program directories in `SYSTEM_READS`: on Linux, `/usr`,
  `/bin`, `/sbin`, the `/lib` directories, `/etc`, `/nix/store`, and
  `/run/current-system`, when they exist.
- The paths named with `Spec::readable`.
- The paths it can write: the checkout, the writable paths, and the
  owned scratch directory.

Everything else is unreadable, including every protected and sealed path,
home directories, and other workspaces.

- **On Linux**, `bwrap` starts from an empty root instead of binding `/`
  read-only. It binds the system directories and the readable paths
  read-only, then the writable paths writable. It mounts an empty `/tmp`,
  and `--unshare-pid` gives the command its own process namespace, so
  `/proc` doesn't show other processes or their working directories.
  A system directory that is a symbolic link, such as `/bin` on a
  merged-`/usr` system, is recreated as the same link.
- **On macOS**, the profile adds `(deny file-read*)` after the write
  rules, then allows the same paths. File metadata stays readable
  everywhere because the loader needs it, so a command can test whether a
  path exists, but it can't list a directory or read a file outside the
  set. This profile has not yet run on a macOS host.
- **Elsewhere**, `build` refuses as it does for writes.

`Boundary::search_path` rewrites a `PATH` for the confined command: it
resolves each directory through its symbolic links and drops the ones the
command can't read. A Nix profile in the home directory resolves into
`/nix/store` and stays on the path.

Microluna applies the policy through `Workspace::confining_reads`. A
session in a task container can't confine reads, so every command there is
refused.

Tests, Linux (NixOS, `bwrap`), 2026-09-25:

- `a_read_confined_boundary_reads_only_what_it_names`: the command reads
  a readable file and writes its scratch. It can't read or list a
  directory beside the readable one, `find /tmp` doesn't find that
  directory's file, and it can't see the test process in `/proc`.
- `a_read_confined_boundary_hides_the_home_directory` and
  `the_profile_denies_reads_first_and_allows_after`.
- `a_read_confined_spec_with_a_missing_backend_is_refused` and
  `a_platform_without_a_backend_refuses_to_confine_reads`: no backend
  means a refusal, never an open command.
- In `crates/microluna` and `crates/coder-one`, a session and an oracle
  writer with confined reads read their own files and a granted task
  directory. They can't copy a candidate's file from a directory beside
  them, and a session in a task container runs no command.

## Ownership

`sandbox-exec -f` reads the profile from a file, and a scratch directory
the boundary owns is removed when the boundary drops. Both must stay
alive until the supervisor has reaped the child — removing a scratch
while its writer still runs is the failure `run_holding` exists for.
`Boundary` is `Send`; the intended wiring is:

```rust
let boundary = Boundary::readonly()
    .protecting(main_checkout)
    .owned_scratch_under(private_parent)
    .build()?;
let mut command = boundary.command(&executor_binary, &argv)?;
let job = supervise::Job::from_command(command);
let outcome = job.run_holding(boundary.hold()).await;
```

`Boundary::hold` returns the `Held` guard — the profile file and the
owned scratch, and nothing else — for the caller that builds its
commands first and then needs a resource for `Job::run_holding`.
`Boundary::profile_file` exposes the path for a caller that assembles a
`supervise::Job` argv itself over `SANDBOX_EXEC` and `-f`.

## Snapshots

`Snapshot::observe(root)` walks the tree without ever following a link.
On Unix the walk is descriptor-relative: the root is opened component by
component from `/` with `openat` under `O_NOFOLLOW`, every entry is
stat'd with `fstatat(…, AT_SYMLINK_NOFOLLOW)`, directories are opened
with `openat` — plus `O_DIRECTORY` where the platform has it, and an
`fstat` kind check everywhere — before they are listed, and files are
opened the same way and stat'd again after they are hashed. A link
swapped into place mid-walk fails an open rather than redirecting one,
a directory's descriptor is held while it waits to be listed so the
listing reads the directory that was found rather than whatever the
path names later, and a file whose metadata moved while it was being
read is a fault rather than a digest of two halves. A link's entry is
its `readlinkat` target — the link itself — so a link pointing outside
the root pulls nothing in. On a platform where this cannot be promised,
every observation is refused rather than taken unsafely.

Each entry records what a write would change: files by length, SHA-256
of contents, modification time, and mode; directories by mode; other
kinds by name. Device and inode identity changes also count, including a
same-content replacement with the original modification time restored. The root itself is recorded at the empty path, so a
write that touches only the root's metadata is still seen. `compare` on
two snapshots of the same root reports `Clean`, `Changed` with a
deterministically ordered list, or `Unverifiable`. Renames pair removed
and created entries on device and inode, so a moved directory reads as
renames rather than a delete-and-create of everything in it.

The verdict's discipline is the important half: any fault — an
unreadable path, a mid-read mutation, the entry bound, the hashed-bytes
bound, an unresolved root — makes the snapshot incomplete, and an
incomplete snapshot cannot establish that nothing was written. Two
identical partial observations compare `Unverifiable`, never `Clean`.
Directory-name collection stops at the remaining entry budget before
allocating an unbounded list. Nonblocking opens and regular-file checks refuse
a file replaced by a FIFO without waiting for a writer. The bounds count
every listed entry, failed or not, and the fault list
itself is capped, so a hostile tree cannot exhaust the budget on
failures or turn detail into a memory problem.

## What this does not do

- **It confines writes unless the caller asks for more.** Reads stay
  open unless the caller confines them (see
  [Confined reads](#confined-reads)), and network stays open unless the
  caller asks for `Spec::offline`. A command that can read a secret and
  reach the network can send it.
- **It does not bound time, output, memory, or CPU.** That is
  a separate host concern. `crates/supervise` supplies the wall-clock and
  output limits; it does not supply a general memory or CPU quota.
- **`sandbox-exec` is a best-effort macOS facility.** Apple has called
  the profile interface unsupported for years while still shipping it;
  the parent's profile evidence stands, and the tests here exercise the
  built boundary end to end, but a future macOS could remove the tool,
  in which case `build` refuses rather than degrades.
- **The Seatbelt ordering is load-bearing and unverified by Apple
  documentation.** Denies are emitted before allows so that the
  checkout nested under a protected path is the exception. The
  writing-mode test proves that on this backend; a backend whose rule
  precedence differs would need a different profile.
- **Resolution is a point-in-time check.** Every configured path is
  canonicalized at `build`; a symlink planted between validation and
  the child's first write is not re-checked. On the enforced filesystem
  the check and the child's writes go through the same canonical path,
  so the exposure is the window, not a second path.
- **`git` inside a sandboxed worktree cannot write its metadata.** The
  worktree's index and administrative files live under the common Git
  directory, which is sealed and accepts no exception — a delegate can
  edit files but not commit. A caller that wants commits must arrange
  the worktree's metadata outside the sealed directory; granting a path
  beneath a sealed one is refused by design.
- **Snapshots see content, not intent.** A `chmod`, a `touch`, and a
  same-bytes rewrite all read as `Modified`, which is correct — they
  are writes — and an `mtime` left untouched by a clock-sensitive
  writer is still caught by the content digest. What a snapshot cannot
  see is a write that never landed on the filesystem it watched, and a
  mutation that lands mid-observation is a fault, not a clean digest.
- **The boundary holds the profile, not the child's honesty.** A
  delegate that only ever writes where it may still needs the snapshot
  comparison for a record of what it changed, and the trace still needs
  a verdict, not a claim.

## Verification

Run from a checkout on a machine with `/usr/bin/sandbox-exec`:

```sh
cargo +1.97.1 test --locked -p coder-boundary
cargo +1.97.1 clippy --locked -p coder-boundary --all-targets -- -D warnings
```

The enforced tests skip themselves on a machine without the backend.
They are implementation evidence — the wrapped command, the profile it
produced, and the supervisor around the child — not backend evidence;
the parent's proof that this profile shape denies outside writes and
permits the scratch stands separately. A cross-platform refusal check,
with a Linux target installed:

```sh
cargo +1.97.1 check --locked -p coder-boundary --target x86_64-unknown-linux-gnu
```

## Remaining gaps

- Coder still runs delegates under subprocess time and output bounds,
  without this filesystem boundary. Integration must hold `Boundary` through
  `Job::run_holding`. CoderBench now independently observes the repository's
  contents before and after execution, including already dirty and ignored
  files. Partial observations remain unverifiable, and self-reported writes
  cannot satisfy a positive required write count.
- The Devin adapter's writable state goes through `Spec::writable` —
  there is no hardcoded `HOME` grant, and what the adapter actually
  needs must come from an explicit host approval, not a manifest claim.
- No non-macOS backend exists. A Linux implementation (Landlock or
  `bubblewrap`) is the next boundary work; until one lands, delegation
  on Linux has no enforced filesystem isolation.
- The snapshot walk is Unix-only. Non-Unix platforms refuse observation
  rather than walk unsafely, which means snapshots there always read
  unverifiable — correct, and useless, until a no-follow traversal
  exists for them.
- `sandbox-exec` confines writes only within the process tree it wraps;
  a sandboxed process that convinces a privileged outside process to
  write for it acts outside this boundary. Process-group escape alone
  does not remove inherited filesystem restrictions.

Verified on macOS with Rust 1.97.1: 50 tests and one doctest pass. This
includes actual sandboxed writes, disallowed writes, child-process writes,
symlink paths, bounded listings, FIFO replacement, and same-content file
replacement. Linux execution has not been verified in this record.
