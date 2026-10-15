# Download and install

Coder runs on macOS, Linux, and Windows. One command installs it, and
the same command updates it. The [download page](/download) has the same
commands and a manual download for each platform.

## Install Coder

On macOS or Linux, in a terminal:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

Then open a new terminal and check it:

```sh
coder --version
```

It prints `coder` and the version you have. The installer picks the
latest release for your computer: Apple silicon or Intel on macOS,
x86_64 or ARM on Linux (including musl systems such as Alpine), and
x86_64 on Windows (Windows on ARM runs that build too).

Next, log in and send a first message: [Coder quick start](/docs/coder).

## What gets installed

| Command | What it is |
| --- | --- |
| `coder` | The Coder terminal app. Run it in a project folder. |
| `openagents` | The OpenAgents command: Coder from scripts, chat, settings, and more. See [The openagents command](/docs/cli). |
| The engine | The engine Coder's own coding loop runs on. You never run it yourself. |

On Windows there is a fourth file, `coder-boundary.exe`, which keeps the
commands Coder runs from writing outside the folders they're given.

They go in `~/.openagents/bin` (on Windows,
`%USERPROFILE%\.openagents\bin`). The installer adds that folder to your
`PATH`: on macOS and Linux it adds one line to your shell's startup file
(`.zshrc`, `.bashrc`, `.bash_profile`, `config.fish`, or `.profile`),
and on Windows to your user `PATH`. Open a new terminal to pick it up.

Before anything is replaced, the installer checks each download against
the release's published SHA-256 checksums and checks that each command
runs and reports the right version. If any step fails, your existing
install is left as it was.

### Install options

```sh
# One exact version
curl -fsSL https://openagents.com/cli/install.sh | bash -s -- 1.0.0
# Follow release candidates
curl -fsSL https://openagents.com/cli/install.sh | bash -s -- rc
# Another folder, and no change to your shell's startup file
curl -fsSL https://openagents.com/cli/install.sh | CODER_BIN_DIR=~/bin CODER_NO_PATH_UPDATE=1 bash
```

On Windows, set the same choices first:

```powershell
$env:CODER_VERSION = '1.0.0'   # or: $env:CODER_CHANNEL = 'rc'
irm https://openagents.com/cli/install.ps1 | iex
```

With no version named, the installer follows stable releases, and the
newest release candidate while no stable release is out yet.

## Update

Coder keeps itself up to date. Once a day, while it's open, it checks for
a newer release, downloads and verifies it in the background, and
installs it when you quit. A release candidate follows release
candidates; a stable release follows stable releases.

| To | Run |
| --- | --- |
| Update now | `coder update` |
| Just check | `coder update --check` |
| Go back to the version the last update replaced | `coder update --rollback` |
| Only be told when an update is ready | `coder update --mode notify` |
| Stop checking | `coder update --mode off` |
| Switch to release candidates | `coder update --channel rc` |

Running the install command again also updates Coder.

## Uninstall

1. Sign this computer out of your account: `coder logout`.
2. Remove the install folder, where the installer put Coder's commands
   and the copies kept for `coder update --rollback` (keep anything else
   you put there yourself). On macOS or Linux:

   ```sh
   rm -rf ~/.openagents/bin
   ```

   On Windows, in PowerShell:

   ```powershell
   Remove-Item -Recurse "$env:USERPROFILE\.openagents\bin"
   ```

3. Remove the `PATH` line. On macOS and Linux, delete the two lines that
   start with `# Coder installer PATH` from your shell's startup file. On
   Windows, remove `.openagents\bin` from your user `PATH` in **Settings →
   System → About → Advanced system settings → Environment Variables**.
4. To also delete your saved chats and Coder's settings on this computer:
   `rm -rf ~/.openagents/coder` (on Windows,
   `Remove-Item -Recurse "$env:USERPROFILE\.openagents\coder"`). Chats you
   synced to your account stay there until you delete them on
   [openagents.com](/).

## Windows

Coder, its chats, and the `openagents` command work on Windows. Local
task services and background automation need macOS or Linux for now.

## Web and phone

Nothing to install for the web: chat at [openagents.com](/) in any
browser. The iPhone app is in beta; see
[OpenAgents for iPhone](/docs/iphone).

Next: [Chat on openagents.com](/docs/website).
