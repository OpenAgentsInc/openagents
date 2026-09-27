# Host service

`coder-service` runs one Coder host as a background service on macOS and
Linux. It installs a launchd agent or a systemd user unit, runs the host
under a launcher, and updates the host without losing state: the new version
runs as a *trial* against a snapshot of the host's state, commits when the
host reports ready before a deadline, and rolls back to the previous version
and snapshot otherwise. A host descriptor tells clients what they are
talking to before they assume anything.

[Issue #9710](https://github.com/OpenAgentsInc/openagents/issues/9710)
delivers this slice of M11 in the
[migration tracker](../migration-status.md). The
[verification record](../verification/2026-09-26-host-service.md) holds the
unit tests and the macOS launchd runtime check.

## Where it lives and why

The service manager, the launcher, and the descriptor are one crate,
[`crates/coder-service`](../../../crates/coder-service/), with one binary,
`coder-service`. It is a separate small binary rather than a `coder`
subcommand for these reasons:

- The launcher must outlive the version it runs. A trial replaces the `coder`
  bundle that is running; if the launcher were that bundle, a failed trial
  could take down the component that has to roll it back.
- The launcher's own binary changes rarely. Install copies it to
  `~/.openagents/host/bin/<sha256>/coder-service`, and the service definition
  points at that copy, so rebuilding a checkout never changes what the
  service runs.
- It has no dependency on the agent, the model doors, or Nostr. It depends
  on `supervise` for process-group ownership, and on `secp256k1` only to
  check the host key's shape.

`scripts/coder-host.py` keeps staging digest-named bundles, checking their
task interface, and rendering one-shot task services. The resident host
service, trial updates, rollback, and the descriptor belong to
`coder-service`. The [portable host guide](portable-host.md) describes the
split.

## Layout

Everything the service writes for you lives under `~/.openagents`:

| Path | Contents |
| --- | --- |
| `~/.openagents/host-bundle/` | Staged bundles, written by `scripts/coder-host.py`. |
| `~/.openagents/tasks/` | The default state directory a trial snapshots. |
| `~/.openagents/host/service.json` | The service configuration. |
| `~/.openagents/host/launcher.json` | The launcher's durable record: committed version, generation, update phase. |
| `~/.openagents/host/descriptor.json` | The host descriptor. |
| `~/.openagents/host/update-request.json` | A pending update request. |
| `~/.openagents/host/snapshots/` | The snapshot of a trial in progress. |
| `~/.openagents/host/run/` | Ready records, one per host generation. |
| `~/.openagents/host/logs/` | `launcher.log` (macOS) and `host.log`. |
| `~/.openagents/host/service/` | The rendered unit or plist. |
| `~/.openagents/host/bin/` | The installed launcher binary. |

The operating system reads service definitions from its own directory, so
install adds one symbolic link there, pointing at the rendered file:
`~/Library/LaunchAgents/<label>.plist` on macOS and
`~/.config/systemd/user/<label>.service` on Linux. That link is the only
thing outside `~/.openagents`. Uninstall removes it only when it still points
at this host's file, and install refuses to replace a file it did not
create.

## Install

1. Stage a bundle with the [portable host guide](portable-host.md). Staging
   selects it, and `coder-service` uses the selected bundle unless you pass
   `--version`.
2. Build `coder-service` with the pinned toolchain:

   ```sh
   cargo build --release -p coder-service
   ```

3. Install the service. `--host-key` is the host's Nostr public key: 64
   lowercase hexadecimal characters of an x-only secp256k1 key. Host
   enrollment work owns the matching secret key; this service only
   publishes the public half.

   ```sh
   target/release/coder-service service install --host-key <64-hex-public-key>
   ```

Install writes the configuration and the launcher record, copies the
launcher binary, renders and registers the definition, and starts the
service. It refuses when the launcher record already committed a different
version; change versions with an update.

| Option | Default | Meaning |
| --- | --- | --- |
| `--version` | The bundle `coder-host.py` selected | The bundle to commit first. |
| `--bundle-root` | `~/.openagents/host-bundle` | Where bundles are staged. |
| `--state` | `~/.openagents/tasks` | A state directory to snapshot. Repeat for more. |
| `--label` | `org.openagents.coder-host` | The launchd label or systemd unit name. |
| `--listen` | `127.0.0.1:47100` | The address the host binds. It must be loopback. |
| `--ready-timeout` | `60` | Seconds a trial has to report ready. |
| `--stop-grace` | `10` | Seconds a host has to exit after `SIGTERM`. |
| `--snapshot-max-bytes` | 1 GiB | The most state one snapshot copies. |
| `--linger` | Off | Linux only: run `loginctl enable-linger`. |
| `--no-start` | Off | Register and enable without starting. |
| `--platform` | This machine's | `macos` or `linux`. |
| `--registration-dir` | The platform's directory | Where the registration link goes. |
| `-- <args>` | `host serve` | The arguments after the host binary. |

Pass `--root <dir>` before the command to use a host root other than
`~/.openagents/host`.

The default host arguments, `host serve`, name the resident host command that
[issue #9712](https://github.com/OpenAgentsInc/openagents/issues/9712)
adds to `coder`. Until that lands, the current `coder` binary refuses those
arguments with a usage error, the launcher exits with that code, and the
service manager restarts it within its limits. Pass explicit host arguments
after `--` for any other host program.

The service binds loopback only. Remote reach comes from the host reach and
enrollment work, never from this service.

## What the service does

The rendered definition runs `coder-service --root <root> run`. The service
manager restarts the launcher only when it fails: a clean stop stays
stopped.

| Setting | launchd agent | systemd user unit |
| --- | --- | --- |
| Starts | At login (`RunAtLoad`) | At login; at boot with linger |
| Restart | `KeepAlive.SuccessfulExit=false`, `ThrottleInterval=10` | `Restart=on-failure`, `RestartSec=5`, at most 5 starts in 300 seconds |
| Stop | `ExitTimeOut` = stop grace + 10 seconds | `KillMode=control-group`, `TimeoutStopSec` = stop grace + 10 seconds |
| Environment | `PATH=/usr/bin:/bin`, `Umask` 077 | `PATH=/usr/bin:/bin`, `UMask=0077`, `NoNewPrivileges=yes` |
| Logs | `logs/launcher.log` | The user journal |

On `SIGTERM` the launcher stops its host (`SIGTERM` to the host's process
group, the stop grace, then `SIGKILL`), writes a `stopped` descriptor, and
exits zero.

## Status, restart, and uninstall

```sh
coder-service service status
coder-service service restart
coder-service service uninstall
coder-service service render
```

Status is JSON. It never changes anything.

| Field | Meaning |
| --- | --- |
| `loaded`, `running`, `pid` | What launchd or systemd reports for the launcher. |
| `enabled` | Whether the service starts on its own. |
| `starts_at` | `boot`, `login`, or `never`. |
| `survives_logout` | `true` only for a systemd unit with linger. A launchd agent in the `gui` domain stops at logout. |
| `linger` | The systemd linger setting, or `null` on macOS. |
| `pending_restart` and `pending_reasons` | A changed definition, a systemd unit that needs a daemon reload, a waiting update request, or a running host that is not the committed version. |
| `committed`, `descriptor` | The launcher record's committed version and the latest descriptor. |

Restart runs `systemctl --user restart` or `launchctl kickstart -k`, and it
loads a registered launchd agent that is not loaded. Uninstall stops and
unloads the service, waits for launchd to finish unloading, and removes the
registration link and the rendered file. It keeps the state directories,
bundles, the launcher record, the descriptor, logs, and the linger setting.
Install runs `launchctl enable` only for a label that launchd lists as
disabled, because that command writes a persistent override.

## Updates

Ask for an update with a staged bundle's identity:

```sh
coder-service update --to <sha256> --wait 120
```

The command checks that the bundle is staged and intact, writes a request,
and returns. The running launcher picks the request up within a fraction of
a second. With `--wait`, the command waits for the outcome and prints the
descriptor; it exits `0` for `committed`, `2` for `rolled-back`, and `3`
when the wait ended first.

An update stops the running host, so the host is unavailable from the stop
until the trial or the restored version is ready. Clients see `updating` in
the descriptor during that time.

### States

The launcher record's `phase` is one of four states, and every transition is
one atomic write followed by a directory sync.

| Phase | Meaning | After a launcher crash |
| --- | --- | --- |
| `idle` | The committed version runs, or runs next. | Remove unreferenced snapshots and ready records. |
| `prepared` | A complete snapshot exists; the trial has not started. | Roll back. |
| `trial` | The target runs as generation *n* with a ready deadline. | Commit when the ready record for *n* exists; otherwise roll back. |
| `rolling-back` | The snapshot is being restored. | Restore again. A restore is idempotent. |

A trial commits when the host writes a ready record for its generation and
version before `--ready-timeout` passes. It rolls back when the host exits
first, misses the deadline, or the launcher is asked to stop. Rollback stops
the trial host, restores every state directory from the snapshot, records
the reason, and starts the previous version. An update that fails before
the trial, such as state larger than `--snapshot-max-bytes`, is recorded as
rolled back with the refusal as its reason; nothing ran and nothing changed.

Before recovering, the launcher stops any host process group its record
names that still runs, so a host a crashed launcher left behind never runs
beside a restore or a second host. A request that the record already
answered is removed rather than retried.

A snapshot copies each state directory into `snapshots/<request>/` and is
renamed into place only when complete. It copies directories and ordinary
files with their permission bits, and it refuses symbolic links, devices,
FIFOs, and sockets instead of following or dropping them. A restore stages
the copy beside the target, moves the target aside, renames the copy into
place, and checks the restored tree's digest. State directories must not
contain the host root or sit inside it.

### The host contract

The launcher starts `<bundle>/coder <host args>` in a process group of its
own, with a cleared environment that holds `PATH=/usr/bin:/bin`, `HOME`, and
these variables:

| Variable | Meaning |
| --- | --- |
| `OPENAGENTS_HOST_READY_FILE` | Where the host writes its ready record. |
| `OPENAGENTS_HOST_GENERATION` | The generation the ready record repeats. |
| `OPENAGENTS_HOST_VERSION` | The bundle identity the ready record repeats. |
| `OPENAGENTS_HOST_LISTEN` | The loopback address to bind. |
| `OPENAGENTS_HOST_TRIAL` | `1` during a trial, else `0`. |

The host writes the ready record atomically, for example to a temporary
name followed by a rename, once it serves:

```json
{
  "schema": "openagents.coder.host-ready.v1",
  "generation": 7,
  "version": "<64-character bundle identity>",
  "protocol_version": 1,
  "capabilities": ["tasks"]
}
```

Standard output and standard error go to `logs/host.log`.

## Host descriptor

`coder-service descriptor` prints `descriptor.json`. A client reads it before
it assumes anything about the host.

| Field | Meaning |
| --- | --- |
| `schema` | `openagents.coder.host-descriptor.v1`. A client refuses any other value. |
| `host_key` | The host's x-only Nostr public key. |
| `protocol_version` | The host protocol the running host reported, or `null` before it is ready. |
| `host_generation` | Increases every time the launcher starts a host. A different generation means the host restarted. |
| `capabilities` | Sorted, unique flags: `host-rollback` and `host-trial-update` from the launcher, plus the flags the ready host reported. |
| `listen` | The loopback address. |
| `state` | `starting`, `ready`, `updating`, or `stopped`. |
| `version` | The running bundle, or the committed one when no host runs. |
| `update` | The latest update: `state` (`none`, `prepared`, `trial`, `committed`, or `rolled-back`), `from`, `target`, `request`, and a rollback `reason`. |

The encoding is canonical: keys in lexicographic order, no insignificant
whitespace, and a trailing newline, so equal descriptors are equal bytes.
Decoding refuses an unknown schema, an unknown field, an invalid host key, a
non-loopback address, malformed or unsorted capability flags, and a version
that is not a bundle identity.

A client that asked for an update reconnects and reads the descriptor: it
sees `update.state` `committed` with `update.target` equal to `version`, or
`rolled-back` with a reason and the previous `version`.

## Limits

- **Linux runtime.** Unit rendering, quoting, linger parsing, and the exact
  `systemctl` and `loginctl` commands are unit tested. No real systemd run
  is part of this slice's evidence.
- **Logout on macOS.** A launchd agent in the `gui` domain stops at logout
  and starts at the next login. Status reports this rather than hiding it.
- **Login-time loading.** The runtime check loads the agent with
  `launchctl bootstrap` from a symbolic link. Loading at an actual login
  from `~/Library/LaunchAgents` was not exercised.
- **Recorded process groups.** A crash between starting a host and recording
  its process group leaves that host unrecorded; systemd's control-group
  stop still ends it, and launchd does not. A recorded group identifier
  could in principle be reused after its host exited.
- **Descriptor transport.** The descriptor is a local file. Publishing it to
  remote clients belongs to the host reach work.
- **Downtime.** An update stops the running host before the snapshot. There
  is no side-by-side trial.
