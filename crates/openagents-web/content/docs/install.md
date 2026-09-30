# Install Coder Terminal

Coder Terminal is the command-line client for Coder.
One command installs it on macOS, Linux, and Windows.

## Install on macOS and Linux

Run:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | sh
```

The installer detects your operating system and processor, and on Linux
whether the machine uses glibc or musl. It downloads the build for that
platform together with the release's checksum file, and verifies the SHA-256
of the downloaded bytes against it. When a digest does not match, the
installer discards the download and installs nothing. It then puts the `coder`
command in `~/.openagents/bin`, and starts Coder when a terminal is attached.

If `~/.openagents/bin` is not on your `PATH`, the installer prints the line to
add. Add it to your shell profile:

```sh
export PATH="$HOME/.openagents/bin:$PATH"
```

Open a new terminal, and run `coder`.

## Install on Windows

Run in PowerShell:

```powershell
irm https://openagents.com/releases/install-terminal.ps1 | iex
```

The Windows installer verifies the same checksum file, puts `coder.exe` in
`%USERPROFILE%\.openagents\bin`, and adds that directory to your user `PATH`.
Open a new terminal to pick up the change.

## Install a release candidate

A version reaches the `rc` channel before it becomes `stable`. To follow `rc`,
set `CODER_TERMINAL_CHANNEL` in the environment the installer runs in.

On macOS and Linux:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | CODER_TERMINAL_CHANNEL=rc sh
```

In PowerShell:

```powershell
$env:CODER_TERMINAL_CHANNEL = 'rc'; irm https://openagents.com/releases/install-terminal.ps1 | iex
```

## Install a specific version

On macOS and Linux, pass the version as an argument:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | sh -s 0.5.0-rc.4
```

In PowerShell, set `CODER_TERMINAL_VERSION`:

```powershell
$env:CODER_TERMINAL_VERSION = '0.5.0-rc.4'; irm https://openagents.com/releases/install-terminal.ps1 | iex
```

A version name looks like `0.5.0` or `0.5.0-rc.4`. A channel name such as
`stable` is not a version: the shell installer answers `Not a version` when
you pass one. To follow a channel, set `CODER_TERMINAL_CHANNEL` instead.

## Current versions

| Surface | Version |
| --- | --- |
| Terminal, `stable` channel | `{{stable}}` |
| Terminal, `rc` channel | `{{rc}}` |
| Service | `0.5.0`, commit `30611a3ac6`, recorded 2026-09-12 |
| Phone, TestFlight build | `37`, recorded 2026-09-12 |

The two terminal rows read the channel pointers when you open this page:
`https://openagents.com/releases/coder-terminal.stable` and
`https://openagents.com/releases/coder-terminal.rc`. Each pointer holds one
version on one line, and the installer reads the same two files. A row that
reads `unknown` means this service could not read that pointer; open the URL
above to read the channel directly. The service and phone rows carry the
values recorded on the date they name.

## What changed in a release

The [changelog](changelog.md) lists every change in a release, grouped by
surface, with the issue or commit behind each one.
