# coder-ssh

`coder-ssh` installs, starts or adopts, enrolls, and reaches a Coder host on
any machine you can reach with `ssh`. Your code and provider credentials stay
on the remote machine; the client only forwards a loopback port to it. It
implements the launcher side of the
[SSH-launched hosts profile](../../nips/openagents/NIP-ENV.md#ssh-launched-hosts)
in NIP-ENV, from [issue #9709](https://github.com/OpenAgentsInc/openagents/issues/9709).

## Use it

```rust,ignore
use coder_ssh::{Arch, Artifact, Launcher, Os, Release, Runner};

let release = Release::new(vec![Artifact {
    os: Os::Linux,
    arch: Arch::X86_64,
    sha256: pinned_sha256,
    archive: "/path/to/coder-linux-x86_64.tar.gz".into(),
}])?;
let runner = Runner::new(
    vec!["host".into(), "serve".into(), "--loopback".into()],
    vec!["host".into(), "invite".into()],
)?;
let launcher = Launcher::new("devbox", release, runner)?;
let host = launcher.up()?;             // install or reuse, then adopt or start
let mut tunnel = launcher.connect(&host)?;
tunnel.ready(std::time::Duration::from_secs(10))?;
let invitation = launcher.invite(&host)?; // redeem through the host's access contract
```

Every call blocks; run it on a blocking thread from an asynchronous host.

## What happens on the remote machine

Each operation is one `ssh` invocation that sends the fixed script in
[`src/remote.sh`](src/remote.sh) to `sh -s` on standard input. The script
never evaluates text from the client. It uses these paths under the remote
account's home:

| Path | Contents |
| --- | --- |
| `~/.openagents/ssh-host/versions/<archive-sha256>/` | The extracted `coder` binary and a `manifest` with the archive and binary digests. |
| `~/.openagents/ssh-host/uploads/` | Archives in transit, each under a fresh random name. |
| `~/.openagents/ssh-host/lock/` | The installation lock and its `owner` record (`<pid> <node>`). |
| `~/.openagents/ssh-host/managed` | The ownership record for a host a launcher started. |
| `~/.openagents/ssh-host/host.log` | Standard output and standard error of a managed host. |
| `~/.openagents/host/runtime` | The runtime record every resident host writes: schema, process identifier, and loopback port. |

The version directories follow the
[portable host bundle](../../docs/coder/runtime/portable-host.md) layout:
digest-named, immutable, staged in a private directory, verified, and then
renamed into place. They live in a separate root because the portable host
helper's lock and journal are not available to a POSIX shell.

`up` detects the platform, picks the pinned archive, and asks for the upload
only when no verified copy is installed. The archive travels in a second
`ssh` invocation and is verified against its SHA-256 before extraction. The
extracted binary must answer `--version` within ten seconds before it is
moved into place. A live host found through the runtime record is adopted
as `external` unless the managed record names the same process and that
process still runs the recorded version's binary; then it is `managed` and
reused, or relaunched when the version or serve arguments changed.

## Ownership rule

Only two explicit operations stop a host, and only a `managed` one:
`Launcher::remove`, and `Launcher::up` when the release or serve arguments
changed and the host must be replaced. Remove detaches from an `external`
host. Either operation signals only the process that both remote records
name and that still runs the recorded version's binary, so a reused process
identifier never reaches an unrelated program. A dropped `Launcher`, `Host`,
or `Tunnel`, a dead tunnel, and a client that exits without cleanup leave the
host running.
The host is started in its own session (`setsid` when available, otherwise a
separate process group) with its standard streams redirected, so the end of
the SSH session that started it does not reach it.

## Transport rules

- The crate runs the system `ssh` binary directly, with no local shell and no
  SSH library.
- `ssh -G` resolves a destination for display and records.
- Every connection sets `ControlMaster=no`, `ControlPath=none`,
  `ForwardAgent=no`, and `ForwardX11=no`. Script runs clear all forwardings;
  a tunnel forwards exactly `127.0.0.1:<local>` to `127.0.0.1:<remote>` with
  `ExitOnForwardFailure=yes`.
- Without a `Prompter`, `ssh` runs with `BatchMode=yes`. With one, prompts
  go through a one-shot askpass helper in a private temporary directory: the
  helper writes the prompt to one named pipe and copies the answer from
  another. The answer is never in an environment variable, an argument, or a
  file, and the directory is removed when the invocation ends. This needs
  OpenSSH 8.4 or later for `SSH_ASKPASS_REQUIRE=force`.

## The host contract this crate relies on

The serve command must bind loopback only, stay in the foreground, and write
`~/.openagents/host/runtime` with its own process identifier once it
listens. The invite command must print one invitation line and exit. The
resident host in [issue #9712](https://github.com/OpenAgentsInc/openagents/issues/9712)
provides both; until then, the tests use a shell stand-in.

## Tests

```sh
cargo test -p coder-ssh
```

The integration tests in `tests/remote.rs` use a fake `ssh` program and a
temporary remote home. They never read or write the real `~/.ssh` or
`~/.openagents`. A test against a real loopback `sshd` is skipped unless you
set `CODER_SSH_LOOPBACK_DESTINATION` to a disposable account.

## Limits

- Password prompts repeat for each invocation, because connection sharing is
  disabled. Use key-based authentication for a single prompt-free launch.
- The local port is reserved and released just before `ssh` binds it; another
  local program can take it in that window, and `Tunnel::ready` reports it.
- A client that crashes leaves its `ssh -N` tunnel process running until the
  stopped or its connection fails; the host is unaffected either way.
- On systems without `ps`, a managed host is identified by its process
  identifier alone.
- The launcher returns typed local results. It does not yet sign NIP-ENV
  request, lease, or cleanup artifacts, and it does not redeem invitations.
