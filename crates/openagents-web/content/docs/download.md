# Download

The [download page](/download) has Coder's install commands and manual
downloads for seven platforms.

## Coder + OpenAgents CLI

Coder 1.0.0-rc.3 is the new terminal UI. Its installer also installs the
OpenAgents CLI and Microcoder, the companion commands Coder uses.

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

## Manual downloads

The download page includes Coder, OpenAgents CLI, and Microcoder for
macOS, Linux, and Windows. Download every file in your platform's row and
follow the manual install instructions on that page. Windows also needs
the Coder launcher in the same row.

Windows RC: local task services and background automation require macOS
or Linux.

Next: [Chat on openagents.com](/docs/website).
