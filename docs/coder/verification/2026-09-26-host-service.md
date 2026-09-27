# Host service verification

Date: September 26, 2026. Scope:
[#9710](https://github.com/OpenAgentsInc/openagents/issues/9710), the host
service slice of M11 in the [migration tracker](../migration-status.md), part
of [#9704](https://github.com/OpenAgentsInc/openagents/issues/9704).

**The host service passes its acceptance checks.** Unit tests cover systemd
unit and launchd plist rendering, the service manager commands for install,
status, restart, and uninstall, the trial, commit, and rollback state machine
with a crash in every state, snapshot restore, and descriptor encoding. A
real macOS launchd run, recorded separately below, installed a throwaway
agent, committed an update, rolled back two failed updates (one by a trial
host exit, one by killing the launcher during a trial), restarted, and
uninstalled with no residue. No Linux systemd run was performed.

## Implementation

The implementation is new public Rust written for this repository. The
[host service guide](../runtime/host-service.md) documents the design and the
decision to ship a separate `coder-service` binary.

- [Crate overview](../../../crates/coder-service/src/lib.rs).
- [Service manager](../../../crates/coder-service/src/service.rs) and its
  [tests](../../../crates/coder-service/src/service/tests.rs).
- [Launcher state machine](../../../crates/coder-service/src/launcher.rs) and
  its [tests](../../../crates/coder-service/src/launcher/tests.rs).
- [Snapshots](../../../crates/coder-service/src/snapshot.rs).
- [Host descriptor](../../../crates/coder-service/src/descriptor.rs).
- [Bundle verification](../../../crates/coder-service/src/bundle.rs).
- [Command-line interface](../../../crates/coder-service/src/main.rs).

The base revision is `cfc08f81b8`. Checks ran on macOS 26.4 (build 25E246)
with the pinned Rust 1.97.1 toolchain and a separate Cargo target directory
for the worktree.

## Unit tests

```sh
cargo test -p coder-service
cargo clippy -p coder-service --all-targets -- -D warnings
cargo fmt -p coder-service --check
```

All three pass: 27 tests, no Clippy findings, and no formatting changes. The
[test log](2026-09-26-host-service/tests.log) is retained. Six consecutive
test runs passed after the last fix below.

| Area | Tests |
| --- | --- |
| Unit and plist rendering | The exact systemd unit; systemd quoting of `%`, `$`, quotes, and backslashes, and refusal of newlines; the plist's label, `RunAtLoad`, `KeepAlive.SuccessfulExit=false`, `AbandonProcessGroup=false`, umask, exit timeout, arguments, and log paths; XML escaping; `plutil -lint` on the rendered plist; label validation. |
| Service manager | The exact `systemctl` and `loginctl` commands for install with linger, restart, and uninstall; status from `systemctl show` and `loginctl show-user`, including `starts_at` `boot` with linger and `login` without, `survives_logout`, and a pending daemon reload; the exact `launchctl` commands for install, reinstall of a loaded and disabled agent, kickstart, and bootout followed by waiting for the unload; status from `launchctl print` and `print-disabled` in both spellings; refusal to replace or remove a registration file that belongs to something else; private log directories created before launchd can create them. |
| Crash in each state | Crash while staging the snapshot (no record written; the partial snapshot is removed and the request stays pending); crash in `prepared` (rolled back); crash in `trial` before ready (the orphaned trial host's process group is stopped and the snapshot is restored); crash in `trial` after the ready record (committed); crash in `rolling-back` at three points, including after the state directory was moved aside (the restore completes); crash after the commit record (the commit stands and the snapshot is removed). |
| Snapshot restore | Changed, added, and removed files; a state directory absent at snapshot time is removed again; permission bits; an interrupted restore at each step completes; an interrupted snapshot leaves only staging; symbolic links and state over the byte limit refuse. |
| Descriptor encoding | Exact canonical bytes and a round trip; refusal of another schema, an invalid or uppercase key, a non-loopback address, malformed or unsorted flags, an unknown field, an unknown update state, and a version that is not a bundle identity; IPv6 loopback. |
| End to end in process | A client requests an update and reads `committed` with the target version, a new generation, and the new host's capability flags; a trial host that exits and one that never reports ready both roll back to the previous version with the snapshot restored; a refused update is answered instead of retried; a second launcher on the same root is refused. |

## macOS launchd runtime check

This is a real launchd run, separate from the unit tests. The retained
[script](2026-09-26-host-service/macos-launchd-check.sh) and its
[log](2026-09-26-host-service/macos-launchd.log) record it.

- **Label.** `org.openagents.test.coder-service-1790470027`, in the
  `gui/501` domain.
- **Files.** A temporary directory under the session's scratch directory held
  the host root, the bundle root, the state directory, and the registration
  directory. The agent was loaded with `launchctl bootstrap` from a symbolic
  link in that temporary registration directory; nothing was written to
  `~/Library/LaunchAgents` or `~/.openagents`.
- **Hosts.** Four shell-script fixture hosts, staged through
  `scripts/coder-host.py install`, so the check also exercises the helper's
  bundle staging and health checks. Two report ready, one exits with code 3,
  and one never reports ready. They are fixtures, not Coder releases.

| Step | Observation |
| --- | --- |
| Install | Loaded and running; `starts_at` `login`; `survives_logout` `false`; descriptor `ready` at generation 1 with protocol version 1. |
| Update to a good bundle | `update --wait` exited 0 with `committed`, the target as `version`, and generation 2; the new host wrote its marker. |
| Update to a bundle that exits | Exited 2 with `rolled-back` and the reason "the trial host exited with code 3 before it reported ready"; the previous version runs; the trial's write was restored away and `tasks.json` kept its original bytes. |
| Restart | `launchctl kickstart -k`; the descriptor returned to `ready` at a new generation. |
| `SIGKILL` of the launcher during a trial | The launcher was killed by its process identifier while a never-ready trial ran. launchd restarted it; recovery stopped the orphaned trial host (no process remained in its group), restored the snapshot, and recorded `rolled-back` with the reason "the launcher stopped during the trial, before the host reported ready". The previous version ran again under a new launcher process. |
| Uninstall | `stopped` `true`; registration link and definition removed; `launchctl print` then failed for the label; no process referenced the temporary directory. |

After the run, `launchctl print-disabled`, `launchctl list`, and
`~/Library/LaunchAgents` held no entry for any throwaway label.

The first runtime attempt found a defect that unit tests had missed: launchd
created the log directory for `StandardOutPath` with mode 0744 before the
launcher ran, and the launcher refused it and restarted in a loop. Install
now creates the log, run, and snapshot directories privately first, and a
unit test covers it. That attempt's agent and an earlier one were removed.
The same session also found that `launchctl bootout` can return before the
agent is unloaded; uninstall now waits for it. A concurrent reader of the
descriptor could also see a link count of zero on a record that an atomic
write had just replaced; reads now accept that and still refuse hard links.

## Not covered

- No real systemd run. Linux rendering and commands are unit tested only.
- Loading at an actual login from `~/Library/LaunchAgents`, and survival
  across a reboot, were not exercised.
- The fixture hosts stand in for `coder host serve`, which
  [#9712](https://github.com/OpenAgentsInc/openagents/issues/9712) adds.
  No Coder release was trialed, and no state migration between real releases
  was tested.
- No paid or model call was made.
