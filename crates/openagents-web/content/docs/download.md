# Download

The [download page](/download) has Coder's install commands for seven
platforms, the iPhone beta, and a link to the web app.

## Coder

Coder 1.0.0-rc.6 is the new terminal UI. Installing Coder also adds the
`openagents` command.

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

Follow the [Coder quick start](/docs/coder) for an ordered playtest: send a
message, change a small file, configure plugins, and export and resume a chat.
Use `/plugins` to configure plugins and `/models` to select an OpenRouter
model when that plugin is enabled.

## Manual downloads

For a manual install, the download page offers one Coder download per
platform when the release has them: one archive to extract into `~/.openagents/bin`
(`%USERPROFILE%\.openagents\bin` on Windows). The install commands above
need no manual download.

Windows RC: local task services and background automation require macOS
or Linux.

## Phone

On iPhone, install
[TestFlight](https://apps.apple.com/app/testflight/id899247664) from the
App Store, then open
[testflight.apple.com/join/dvQdns5B](https://testflight.apple.com/join/dvQdns5B)
on your phone to install the app.

## Web

Nothing to install: chat at [openagents.com](/) in any browser.

Next: [Chat on openagents.com](/docs/website).
