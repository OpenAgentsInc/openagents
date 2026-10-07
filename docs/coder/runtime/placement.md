# Placement

Long, heavy, non-interactive jobs don't have to run on the Mac that ten
agents share. A job names its class, and the placement policy decides
whether it runs here under a lease or on another computer over SSH
([#10767](https://github.com/OpenAgentsInc/openagents/issues/10767)). The
reasons are Decision 6 and section 11, "Leave the machine", in
[Many agents on one machine](../design/many-agents-one-machine.md).

## Run a placed job

```sh
openagents lease run --class CLASS [--place PLACE] [--fetch PATH]... [--receipt PATH] -- CMD [ARGS...]
openagents lease quiet --class soak [--place PLACE] -- CMD [ARGS...]
```

For example:

```sh
openagents lease run --class release-gate --fetch target/release-gate.json -- ./scripts/verify-rust.sh --release
openagents lease run --class bench --fetch bench/terminal-bench/run.json -- scripts/terminal-bench.sh
openagents lease quiet --class soak -- scripts/grid-soak.sh
openagents lease run --class build --place remote -- cargo build -p coder
```

`lease run` also takes `--priority`, `--no-wait`, and `--keep-target-dir`,
which apply when the job runs here. `lease RESOURCE --class CLASS` is the
same job with its lease named: `quiet` for a release gate, a benchmark, or
a soak, and `build` for a build. `--place` on `lease build` implies
`--class build`.

## Classes and defaults

| Class | Default | Lease here | Why |
| --- | --- | --- | --- |
| `release-gate` | `auto` | `quiet` | Release gates (`./scripts/verify-rust.sh --release`, `scripts/release/acceptance.sh`) are long, heavy, and not interactive. |
| `bench` | `auto` | `quiet` | Terminal-Bench and other benchmark runs are the same, and a computer nobody builds on measures them cleanly. |
| `soak` | `local` | `quiet` | A soak such as the #10559 battle soak measures this Mac's own client, so it has to run here, on a machine the `quiet` lease keeps free of builds. |
| `build` | `local` | `build` | The warm target directories and the build cache are here. |

A place is one of these:

- `local`: here, under the class's lease.
- `remote:COMPUTER`: on COMPUTER, an SSH alias from your SSH configuration
  or `user@host`.
- `remote`: on the first configured computer that answers.
- `auto`: on the first configured computer that answers, else here, under
  the class's lease.

`--place` overrides the policy for one job. A job placed `remote` or
`remote:COMPUTER` whose computer doesn't answer is refused, never run here
quietly; run it with `--place local` instead. A soak that is headless and
doesn't measure this machine can opt in with `--place remote:COMPUTER`.

"Answers" means that `ssh COMPUTER true` succeeds in batch mode within 15
seconds. A `local` placement never asks.

## The setting

`coder.placement` holds the policy
([settings](../../cli/settings.md)): comma-separated `CLASS=PLACE` entries
and `computer=NAME` entries, the computers `auto` and `remote` try in order.
Setting it replaces the whole policy, and a class it doesn't name keeps its
default.

```sh
openagents settings set coder.placement computer=coderos-4080
openagents settings set coder.placement 'computer=coderos-4080,soak=remote:coderos-4080'
openagents settings unset coder.placement
```

No computer is configured by default, so `auto` runs here until you name
one.

## On another computer

A remote job runs the commit you pushed, not your working tree:

1. The checkout must be clean (untracked files are ignored), and `HEAD`
   must be on a remote-tracking branch. Otherwise the job is refused before
   anything connects.
2. One `ssh` invocation, with Coder's connection options
   (`crates/coder-ssh`), runs a fixed script on the computer. It keeps a
   bare clone of `origin`'s URL at
   `~/.openagents/remote-runs/NAME/repo.git`, fetches the commit when the
   clone lacks it, and adds a detached worktree at
   `~/.openagents/remote-runs/NAME/COMMIT`. `NAME` is the repository's
   name, such as `openagents`.
3. The command runs at the top of that checkout with `CARGO_TARGET_DIR`
   set to `~/.openagents/remote-runs/NAME/target` unless the computer sets
   its own, so every commit's checkout shares one warm target directory, and
   with `OPENAGENTS_PLACEMENT=remote`. Its output streams here, and
   `openagents lease` exits with its status. Exit code `125` means the
   checkout couldn't be prepared, and `255` means `ssh` failed.
4. Each `--fetch PATH`, relative to the checkout's top, is copied back to
   the same path under this checkout's top. A file that is missing there is
   reported and skipped.

No lease is held here for a remote job. The computer needs `git` and read
access to `origin`'s URL; this repository's `origin` is public over HTTPS.

## Receipts

Every placed job writes a placement receipt to `placements/` under the
lease root (schema `openagents.lease.placement-receipt.v1`). `--receipt
PATH` writes a copy, and under `--json` it's printed after the command's
output.

| Field | Meaning |
| --- | --- |
| `class`, `asked` | The class, and the place asked for: `--place`, else the policy. |
| `place`, `computer`, `reason` | `local` or `remote`, the computer, and why it ran there. |
| `commit`, `remote_dir` | The commit, and the checkout on the computer for a remote run. |
| `command`, `exit` | The command and its exit code. |
| `started_at_ms`, `ended_at_ms` | When it started and ended, in Unix milliseconds. |
| `leases` | The [lease receipts](leases.md#receipts) of the leases held here: one for a local run, none for a remote one. |
| `fetched` | The result files copied back, as local paths. |

## When a build waits

When `openagents lease build` has to wait and a computer is configured, it
prints one line naming that computer and `openagents lease run --class build
--place remote`. Builds stay local by default; the hint is for a build that
doesn't need this machine's warm cache.

## Limits and next steps

- Stopping `openagents lease` stops the local `ssh`, which closes the
  connection; a remote command that ignores the hangup can keep running.
- Placement reaches computers over SSH only. Boat sandboxes
  (`openagents boat run`) and the GCE pool (`openagents cloud up`, granted
  as the computer `gce`) have their own runners, with per-run credentials
  and teardown; wiring them in as places, such as `remote:boat`, is the next
  step. Until then, run a gate or a benchmark there with those commands.
- A remote job runs at the top of the checkout, whatever directory you run
  `openagents lease` from.

## Tests

`cargo test -p coder-lease placement` covers the defaults, parsing the
setting, and the decision for reachable, unreachable, and unconfigured
computers and for explicit overrides. `cargo test -p coder-ssh --test job`
runs a job through the fake `ssh` harness in `crates/coder-ssh/tests/support`:
the checkout at a pushed commit, streamed output, a fetched result, and an
unpushed commit that fails before the command runs. `cargo test -p
openagents-cli --test lease_place` runs `openagents lease run` end to end
with a stand-in `ssh` first on `PATH`, a temporary `HOME`, and a temporary
lease root.
