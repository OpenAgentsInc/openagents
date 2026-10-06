# Download

The [download page](/download) has every published release candidate and
its install commands.

## Coder + OpenAgents CLI

Coder 1.0.0-rc.3 is the new terminal UI. Its installer also installs the
OpenAgents CLI and the companion commands Coder uses.

On macOS and Linux:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

The installer selects your platform, verifies the published SHA-256
checksums, and installs under `~/.openagents/bin`
(`%USERPROFILE%\.openagents\bin` on Windows). It adds that directory to your
`PATH`. Open a new terminal, then run `coder` for the UI or
`openagents --help` for the CLI. Run the install command again to update.
The installer follows the `rc` channel by default.

Use `/plugins` to configure plugins, `/models` to select a model, and
`/demo` to switch between live conversations and the UI examples.

## OpenAgents for Mac

1. Download OpenAgents for Mac 1.0.0-rc.2 from the [download page](/download).
2. Open the `.dmg` and drag **OpenAgents** onto **Applications**.
3. Open OpenAgents. If macOS asks, allow it to run in the background: it
   keeps a small helper running so your phone can reach the Mac and Coder
   can work while the window is closed.

It needs macOS 13 or later and runs on Apple silicon and Intel. It is
signed by OpenAgents, Inc. and notarized by Apple. Coder comes inside it;
there is nothing else to install. See [OpenAgents for Mac](/docs/mac).

## OpenAgents Terminal

OpenAgents Terminal 1.0.0-rc.2 and the `openagents` command install with
one command.

On macOS and Linux:

```sh
curl -fsSL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh | sh
```

On Windows, in PowerShell:

```powershell
irm https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.ps1 | iex
```

The installer picks the build for your system, checks every file against
the release's published checksums, and puts `openagents` and the engine
Coder runs with in `~/.openagents/bin` (`%USERPROFILE%\.openagents\bin` on
Windows). If that folder isn't on your `PATH`, it prints the line to add.
Then it opens OpenAgents Terminal. Run the same command again to update.
See [OpenAgents Terminal](/docs/terminal).

## Everything else: build from source

OpenAgents for iPhone, the Android app, and OpenAgents for Linux and
Windows are not published for download yet. Build them from source at
[github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents).

## Then

- [Connect your phone to your Mac](/docs/connect-a-computer).
- Sign in to a coding agent on your computer so Coder can work there. See
  [Coding agents](/docs/coding-agents).

Next: [Chat on openagents.com](/docs/website).
