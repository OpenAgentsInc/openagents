# Linux runs of the host service, SSH launcher, and terminals

Date: September 27, 2026. Scope: the Linux platform-acceptance items of
[#9719](https://github.com/OpenAgentsInc/openagents/issues/9719), which
follow the [remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704):

- A Linux systemd run of `coder-service` with `coder host serve`.
- A loopback `sshd` run and a Linux remote run of `coder-ssh`, including the
  `setsid` detach path.
- Linux PTY runs of `coder-pty`, including the glibc `libutil` link.

**All three pass on Linux after four fixes.** The runs found two defects that
only a Linux machine shows, one test that assumed a macOS file layout, and
one command-line defect on every platform. Each fix has a test. Reboot
recovery was not exercised; it waits for an owner-approved reboot.

## Machines

| Fact | Linux machine | Client machine |
| --- | --- | --- |
| Name | `coderos-4080`, shared with other work | The owner's Mac |
| Operating system | NixOS 26.05.20260906.c257840, x86_64 | macOS 26.4 (build 25E246), arm64 |
| Kernel | Linux 6.18.49 | Darwin 25.4.0 |
| C library | GNU libc 2.42 | Apple libSystem |
| Service manager | systemd 260 (260.2), user manager running, linger `no` | launchd |
| OpenSSH | 10.5p1 (`sshd` and `ssh`) | 10.2p1 (`ssh`) |
| Rust | 1.97.1, the pinned toolchain, through `rustup` | 1.97.1 |

The base revision is `dc248dff01`. The source reached the Linux machine as a
`git archive` of that revision, followed by `rsync` of the changed crates,
under a new directory, `/tmp/oa-9719-linux-d941997a`. Builds ran there with
`CARGO_TARGET_DIR` inside that directory, `CARGO_BUILD_JOBS=4`, and
`CARGO_PROFILE_DEV_DEBUG=0`. Nothing was installed, and no file outside that
directory was edited; the two systemd links below were added and removed.

## Defects found and fixed

| Defect | Where | Fix and test |
| --- | --- | --- |
| The host service ran the launcher and the host with `PATH=/usr/bin:/bin`. On NixOS, `/usr/bin` holds only `env` and `/bin` only `sh`, so a host could not run `mv`, `sleep`, `git`, or a terminal's tools. Four launcher tests failed on Linux for this reason ([log](2026-09-27-linux-runs/before-fix-service.log)). | `crates/coder-service` | Install records a search path: `/usr/bin:/bin`, then each absolute directory of the installing shell's `PATH`. `--path` overrides it, and an older configuration keeps `/usr/bin:/bin`. The unit, the plist, and the host environment use it. Three new tests cover the default, the quoting in the unit and the plist, refusal of a relative entry, and an older configuration. |
| Uninstall reported `registration_removed: false` on Linux although the link was gone: `systemctl --user disable` removes a linked unit's registration link itself, before uninstall checked it. | `crates/coder-service` | Uninstall reads the link before it stops the service. A new test uses a runner that removes the link on `disable`, as systemd does. |
| `host_shutdown_kills_the_process_group` failed on Linux every time: the group still answered signal zero after shutdown returned ([log](2026-09-27-linux-runs/before-fix-pty.log)). A killed background child stays in the group as a zombie until init reaps it. | `crates/coder-pty` | Ending a terminal now waits, within the 250-millisecond grace period, until the group is empty, as `supervise` already does for jobs. The existing test covers it. |
| The PTY tests launched `/bin/cat`, which NixOS does not have, so ten real-PTY tests failed to spawn. | `crates/coder-pty` tests | The tests find `cat` through `PATH`. |
| `coder host serve --no-telemetry` failed with "`--no-telemetry` needs a value". The flag was documented but missing from the parser's flag list, on every platform. | `crates/coder-host` | Added to the flag list, with a parser test. |

## Terminals: `coder-pty`

```sh
cargo test -p coder-pty
cargo test -p coder-pty --no-default-features
```

| Run | Result |
| --- | --- |
| Default features, before the fixes | 8 of 20 real-PTY tests passed. |
| Default features, after the fixes | 19 unit, 20 real-PTY, and 3 wire tests passed, in four consecutive runs. |
| `--no-default-features` | 19 unit and 3 wire tests passed; the real-PTY suite needs `host` and is not built. |

The [final log](2026-09-27-linux-runs/final-pty.log) holds the last run.
The real-PTY suite covers echo, a terminal the process can see, resize, two
readers, detach and replay, a wrapped ring, rights and revocation, exit
codes and signals, close, shutdown and drop killing a group that ignores
`SIGHUP` and `SIGTERM`, idle expiry, retries, admission, rates, and a
terminal inside a write boundary.

**The `libutil` link.** The crate links `libutil` on glibc. The test binary
linked without error against glibc 2.42. `readelf -d` lists only
`libgcc_s.so.1`, `libc.so.6`, and `ld-linux-x86-64.so.2` as needed, and
`nm -D` shows `openpty@GLIBC_2.34` as an undefined symbol that `libc`
satisfies. glibc 2.34 and later keep `openpty` in `libc` and an empty
`libutil.so` for compatibility, so the linker drops `libutil` as unneeded. A
glibc before 2.34 was not tested.

## SSH launcher: `coder-ssh`

```sh
cargo test -p coder-ssh
```

On Linux, 9 unit tests and 18 integration tests passed, with the helper
process test ignored by design ([log](2026-09-27-linux-runs/final-ssh.log)).

### A real `sshd`

The new `real_sshd_lifecycle` test runs a real `ssh` against a real `sshd`
and installs the real `coder` binary built on the Linux machine. The host
runs `coder host serve --loopback --loopback-test --no-telemetry --owner
<key> --relay ws://127.0.0.1:9/`; nothing serves that relay, so the host
serves direct channels after its relay wait. The test checks each step:

1. Remove before start reports `absent`.
2. The first `up` uploads and verifies the archive, and starts a managed
   host. On Linux the script starts it with `setsid`: `ps` shows the host's
   process, group, and session identifiers equal and its parent as process 1,
   after the SSH session that started it has ended.
3. A second `up` reuses both the installation and the host.
4. A tunnel forwards a local port to the host's direct listener and accepts
   a connection. `invite` returns one `coder-host:` line.
5. The tunnel's `ssh` process is killed with `SIGKILL`. The host keeps
   running, and the next `up` reuses it.
6. Remove stops the managed host.
7. A host started over SSH outside the launcher is adopted as `external`.
   Remove detaches from it and leaves it running; the test then stops it by
   the process identifier it recorded.

**The user-level `sshd`.** The system `sshd` configuration was not used. The
run started its own daemon from the NixOS `sshd` binary, as the same user,
with a new host key and a new client key in the temporary directory:

```text
Port 48219
ListenAddress 127.0.0.1
ListenAddress 100.74.238.61
HostKey /tmp/oa-9719-linux-d941997a/sshd/host_key
PidFile /tmp/oa-9719-linux-d941997a/sshd/sshd.pid
AuthorizedKeysFile /tmp/oa-9719-linux-d941997a/sshd/authorized_keys
AllowUsers christopherdavid
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM no
StrictModes no
PermitUserEnvironment yes
AllowTcpForwarding local
AllowAgentForwarding no
X11Forwarding no
PermitTTY no
```

The only authorized key carries `environment="HOME=/tmp/oa-9719-linux-d941997a/rhome"`,
so every session had a throwaway home and the launcher wrote only under it.
The client used a wrapper, `ssh -F <config>`, whose configuration names the
client key, a known-hosts file holding the new host key with strict
checking, and batch mode.

| Run | Command | Result |
| --- | --- | --- |
| Loopback on Linux | `CODER_SSH_REAL_DESTINATION=ssh://christopherdavid@127.0.0.1:48219 CODER_SSH_REAL_SSH=<wrapper> CODER_SSH_REAL_CODER=<linux coder> cargo test -p coder-ssh --test remote real_sshd_lifecycle -- --nocapture` | Passed in 29 seconds ([log](2026-09-27-linux-runs/ssh-loopback-real.log)). |
| Mac to Linux | The same test from the Mac, to `ssh://christopherdavid@100.74.238.61:48219` over the tailnet, with `CODER_SSH_REAL_PLATFORM=linux/x86_64` and the Linux `coder` binary copied to the Mac | Passed in 33 seconds ([log](2026-09-27-linux-runs/ssh-mac-to-linux.log)). |

The first Mac run also passed, but its archive held an AppleDouble
`._coder` entry that macOS `tar` adds for a file with extended attributes,
and it landed in the remote version directory. The test now sets
`COPYFILE_DISABLE=1`. A release pipeline that builds archives on macOS
needs the same setting.

Each run left this under the throwaway home, all removed afterward:
`.openagents/ssh-host/` (the verified version, its manifest, an empty
uploads directory, and `host.log`), `.openagents/host/generation`, and the
host's access store in `.openagents/coder-access/`. The runtime record was
gone because the host removes it on `SIGTERM`.

## Host service: `coder-service` with `coder host serve`

```sh
cargo test -p coder-service
cargo test -p coder-host --lib cli::
```

On Linux, 31 `coder-service` tests and the new parser test passed
([log](2026-09-27-linux-runs/final-service.log)).

The runtime check is a real systemd user unit that runs the real
`coder host serve`. The retained
[script](2026-09-27-linux-runs/linux-systemd-check.sh) and its
[log](2026-09-27-linux-runs/linux-systemd.log) record it:

```sh
WT=$B/src BIN=$B/target/debug/coder-service CODER=$B/target/debug/coder \
  BASE=$B LABEL=openagents-test-9719-d941997a PORT=47919 \
  bash linux-systemd-check.sh
```

- **Unit.** `openagents-test-9719-d941997a.service`, rendered to the host
  root in the temporary directory and linked into
  `~/.config/systemd/user`; `systemctl --user enable` added a link in
  `default.target.wants`. Both links were gone after uninstall.
- **State.** The host root, bundle root, task directory, access store
  (`--state`), and host settings (`--root`) all lived under
  `/tmp/oa-9719-linux-d941997a/svc`, and the host ran with `--no-runtime`.
  No file under `~/.openagents/host`, `tasks`, `coder-access`, or
  `host-bundle` changed during the run.
- **Builds.** Two real builds with different identities: the Linux `coder`
  binary, and the same binary with one trailing comment line, which the
  ELF loader ignores. Two shell fixtures stand in for failed releases: one
  exits with code 3, one never reports ready. All four were staged with
  `scripts/coder-host.py install`.

| Step | Observation |
| --- | --- |
| Install | Loaded, running, and enabled; `starts_at` `login`; `survives_logout` `false`; linger `false`. The descriptor reported `ready` at generation 1 with protocol version 1 and the host's capabilities. The unit's control group held the launcher and the host, and the host listened on `127.0.0.1:47919`. |
| Update to the second build | `update --wait` exited 0 with `committed`, the target as `version`, and generation 2. |
| Update to the build that exits | Exited 2 with `rolled-back` and the reason "the trial host exited with code 3 before it reported ready". The fixture's write to the task directory was restored away, and the evidence file kept its original bytes. The descriptor showed `stopped` when the command returned; the previous build was starting again. |
| Restart | `systemctl --user restart`; the descriptor returned to `ready` at a new generation with the committed build. |
| `SIGKILL` of the launcher during a trial | systemd ended the rest of the control group and restarted the launcher (`NRestarts=1`). Recovery found no process in the trial's group, restored the snapshot, and recorded `rolled-back` with the reason "the launcher stopped during the trial, before the host reported ready". The committed build ran again. |
| Uninstall | `stopped` and `registration_removed` `true`; `LoadState=not-found`; both links gone; no process referenced the temporary directory. |

The first runtime run found the `registration_removed` defect above. The
second run, after the fix, is the retained log.

## Cleanup on the Linux machine

Everything the runs created was removed:

- The throwaway unit, its two links, and the definition, by uninstall,
  followed by `systemctl --user daemon-reload`. `systemctl --user
  list-units --all 'openagents-test-9719*'` then listed nothing.
- The user-level `sshd`, stopped by the process identifier this run
  started, and its keys and configuration.
- Every host the runs started, stopped by the launcher or by recorded
  process identifier.
- `/tmp/oa-9719-linux-d941997a`, including the 3.7 GB target directory.

Linger stayed `no`, the machine was not rebooted, no package was installed,
and no existing file was edited.

## Not covered

- **Reboot recovery.** Starting at boot needs linger, and recovery needs a
  reboot of a shared machine. Both wait for an owner-approved reboot.
- **The process-group fallback.** NixOS has `setsid`, so the Linux runs took
  the `setsid` path only. The earlier macOS runs cover the fallback.
- **Older glibc and other distributions.** Only glibc 2.42 on NixOS ran.
  Musl and Android builds were not run.
- **A reachable relay.** Every host ran against a relay that nothing
  serves, so relay enrollment and presence were not exercised here.
- **Real releases.** The two builds differ by trailing bytes, not by
  source; no state migration between real releases was tested.
