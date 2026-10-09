# CoderOS

CoderOS is a set of NixOS modules for machines that run OpenAgents Coder.
This directory is a flake. It exports modules, packages, and development
shells, and it defines no host: your machine lives in a private flake that
imports this one.

The plan for moving CoderOS here, and the list of what moves next, is
[`docs/os/2026-09-28-coderos-audit.md`](../docs/os/2026-09-28-coderos-audit.md).

## What the flake exports

| Output | What it is |
| --- | --- |
| `nixosModules.coderos` | The base system and every optional capability, each off by default. `nixosModules.default` is the same module. |
| `devShells.x86_64-linux.compositor` | `pkg-config` and the system libraries Smithay links, for `cargo test -p coder-wm -p coder-compositor` with the pinned toolchain. |
| `devShells.x86_64-linux.android` | The Android SDK, NDK, JDK 17, Gradle, and `cargo-ndk` that `scripts/build-coder-android.sh` and `scripts/build-openagents-android.sh` build with. |
| `packages.x86_64-linux` | `coder-desk`, the desk command the CoderOS scripts run instead of `hyprctl`, built from `crates/coder-desk-cli` by `pkgs/coder-desk.nix`, `coder-compositor`, the optional Coder Wayland compositor, built from `crates/coder-compositor` by `pkgs/coder-compositor.nix`, and `coderos-camera`, the camera daemon, built from `crates/coderos-camera` by `pkgs/coderos-camera.nix` with its ONNX Runtime fetched by digest. `paper-mono` is the Paper Mono font package, built by `pkgs/paper-mono.nix` from `crates/paper-mono/fonts/`. Modules that run a program from this workspace add its build here. |
| `checks.x86_64-linux` | Evaluates a stub host with the base module alone, and with every capability turned on, and the example workstation against a stub hardware file. |

`nixpkgs` is pinned to one revision in `flake.nix` and `flake.lock`. A host
flake that follows this input builds against the same packages the checks
evaluated.

## Use it from your own host flake

Keep your hardware file, accounts, keys, and personal modules in a private
flake of your own. [`examples/workstation/`](examples/workstation/README.md)
is a complete host to start from: a host flake, a workstation configuration
with every personal value replaced by a placeholder, and the steps to copy
it into a private repository. A host flake imports the module set and adds
your host:

```nix
{
  inputs.openagents.url = "github:OpenAgentsInc/openagents?dir=os";
  inputs.nixpkgs.follows = "openagents/nixpkgs";

  outputs = { nixpkgs, openagents, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        openagents.nixosModules.coderos
        ./hosts/my-host.nix
        ./hosts/my-host-hardware.nix
      ];
    };
  };
}
```

A host file sets what is yours, for example:

```nix
{
  networking.hostName = "my-host";
  users.users.you = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    openssh.authorizedKeys.keys = [ "ssh-ed25519 AAAA... you@laptop" ];
  };
  coderos.authorizedKeys = [ "ssh-ed25519 AAAA... you@laptop" ];
  coderos.tailscale.enable = true;
  system.stateVersion = "26.05";
}
```

### Edit the modules and your host together

To test a change to these modules before you commit it, point the input at
your checkout for one rebuild:

```sh
sudo nixos-rebuild switch \
  --flake ~/my-hosts#my-host \
  --override-input openagents "git+file://$HOME/openagents?dir=os"
```

Nix reads tracked files from the working tree, including uncommitted edits.
A new file needs `git add` first, because a git flake ignores untracked
files. Root runs the rebuild and your account owns the checkout, so name the
checkout in `coderos.git.safeDirectories`. When the change is right, commit
and push it here, then run `nix flake update openagents` in your host flake
to pin the new revision.

## Modules

| File | Option | What it does |
| --- | --- | --- |
| `default.nix` | `coderos.authorizedKeys`, `coderos.sudo.wheelNeedsPassword`, `coderos.git.safeDirectories` | The base system: systemd-boot, flakes, `nix-ld` for rustup toolchains, SSH with keys only, NetworkManager, Docker, the white console palette, quiet kernel messages on the console, and base developer packages. Sudo asks `wheel` for a password unless a host turns that off. Root's git trusts only the checkouts you name. |
| `desktop.nix` | `coderos.desktop.*` | The Hyprland session on tty1, started under `uwsm` by the console autologin, with Coder in a foot pane as its first window, the tiling binds, the white palette, and a notification daemon. Holds the hooks for the Coder compositor and the extension points for a host's own launchers and window rules. See [The desktop](#the-desktop). |
| `compositor.nix` | none; follows `coderos.desktop.compositor`, `trialTty`, and `compositorBinary` | On a host that runs the Coder compositor, puts `Xwayland` on `PATH`. For a compositor built from a checkout, it also adds the libraries Smithay links to `nix-ld` and to `LIBRARY_PATH` and `PKG_CONFIG_PATH`. The package needs neither, because it links and wraps them itself. |
| `desk.nix` | `coderos.desktop.deskPackage` | Puts `coder-desk`, the command every script asks the session through, in the session. |
| `capture.nix` | `coderos.desktop.screenRecording.*`, `coderos.desktop.dictation.*`, `coderos.desktop.microphone.*` | Screen recording with `bin/screen-record`, push-to-talk dictation on SUPER + V with `bin/dictate-toggle`, and the rules that pick, keep, and set the gain of the microphone you name. Every microphone value is empty until you set it. |
| `presentation.nix` | `coderos.desktop.presentation.*` | SUPER + P sets the screen up to be watched and puts it back, with `bin/presentation-mode`. |
| `browser.nix` | `coderos.desktop.browser.*` | The Coder Browser on SUPER + B: ungoogled-chromium, dark, with the DevTools Protocol on a loopback port, launched by `bin/coder-browser`. Every browser name on the host opens it, and `coder-open-url` opens a URL in it from a sandboxed application. |
| `camera.nix` | `coderos.desktop.camera.*` | The camera daemon `coderos-camera`, which owns the camera node and serves a `v4l2loopback` node, a recording, and hand landmarks at once, and the circular camera view on SUPER + C with `bin/camera-overlay` and `bin/camera-toggle`. Writes its grant to `/etc/coderos/camera.json`. See [Camera and hands](../docs/os/camera-and-hands.md). |
| `hands.nix` | `coderos.desktop.hands.*` | Hand tracking as desk input in the Coder compositor, from the camera daemon's landmarks, with SUPER + H to turn it off and on. `judge` turns on the Jev seam beside the gesture rules in shadow. Needs the camera. |
| `recording-hud.nix` | none | The strip under the camera circle that shows the microphone's level and starts and stops a recording, with `bin/recording-hud`. Present when the camera and screen recording are both on. |
| `android.nix` | `coderos.desktop.android.*` | The Android emulator on KVM with one system image, launched by `bin/android-emulator`. It turns on Xwayland for the session and writes what it installed to `/etc/coderos/android.json`. Needs the desktop. |
| `tailscale.nix` | `coderos.tailscale.*` | Joins the host to a tailnet so other machines reach it by a stable name. The auth key stays in a file on the host, never in the Nix store. Tailscale SSH stays off unless you turn it on. |
| `cpu-limits.nix` | `coderos.cpuLimits.*` | Caps CPU package power, boost frequency, and build parallelism, and reapplies the caps every five minutes and after a resume. Every value is null until you set it. The file holds one measured example. |
| `coder-host.nix` | `coderos.coderHost.*` | Runs the resident Coder host, `coder host serve`, as the systemd user unit that `coder-service` installs, and turns on linger so it starts at boot. See [Run the Coder host](#run-the-coder-host). |
| `coder-update.nix` | `coderos.coderUpdate.*` | A timer that builds `coder` from a branch of this repository and hands it to `coder-service update`, which trials it and rolls back on failure. The build runs under `TasksMax`, `MemoryMax`, and a free-space floor. Runs `bin/coder-update`. |

## The desktop

Turn the desktop on for the account the console logs in as:

```nix
services.getty.autologinUser = "you";
coderos.desktop = {
  enable = true;
  user = "you";
  directory = "/home/you/openagents";
  presentation.enable = true;
  browser.enable = true;
  screenRecording.enable = true;
  dictation.enable = true;
};
```

The tty1 login starts Hyprland with `/etc/coderos/hyprland.conf`, which the
module writes, and asks a running session to reload it after every switch.
The first window is `bin/coder-pane`: foot, in the Coder palette from
`/etc/coderos/foot.ini`, running `coderos.desktop.command`, which is this
repository's `coder-new` found on the session's PATH. Point `directory` at a
checkout so that writing delegations each get a worktree.

The keys follow Omarchy's tiling set:

| Keys | What they do |
| --- | --- |
| SUPER + RETURN, SUPER + T | Open a Coder pane, or a pane on a bare shell. |
| SUPER + W, SUPER + Q | Close the focused window with `bin/coder-close`. A window whose Coder session has a turn streaming or a delegation that has not reported shows a notice first and closes on the second press. |
| SUPER + arrows, SUPER + SHIFT + arrows, SUPER + CTRL + arrows | Move the focus, move the tile, and resize it. |
| SUPER + 1 to 9, SUPER + SHIFT + 1 to 9 | Go to a workspace, and send the window to one. |
| SUPER + J, SUPER + SPACE or D, SUPER + F, SUPER + CTRL + F | Turn the split, float, fullscreen, and fill the tiling area. |
| CTRL + ALT + TAB, SUPER + ALT + arrows, SUPER + SHIFT + ALT + arrows, SUPER + CTRL + ALT + arrows | Focus another monitor, send the window to one, and send the whole workspace to one. |
| SUPER + V, P, B, A, C | Dictation, presentation mode, the browser, the Android emulator, and the camera view, when each is on. |
| SUPER + H | Hand tracking in the Coder compositor, when it is on. |
| SUPER + mouse | Move a float with the left button and resize it with the right. |
| SUPER + SHIFT + E | End the session. |

Every bind and window rule is a row of
[`crates/coder-binds`](../crates/coder-binds/README.md), and a test there
fails when `desktop.nix` and the table disagree.

### Add your own launchers and window rules

A module that stays in your private host flake, such as a video client or
a game launcher, adds its chord and its window rules through two options.
It never needs an option that this flake does not declare:

```nix
coderos.desktop.extraBinds = [
  { mods = "SUPER"; key = "G"; command = "coder-battlenet"; }
];
coderos.desktop.extraWindowRules = [
  {
    name = "StarCraft II client, by title";
    field = "title";
    patterns = [ { prefix = "StarCraft II"; } ];
    ignoreCase = true;
    float = false;
    suppressFullscreen = true;
  }
];
```

Hyprland gets a `bind` or `windowrule` line for each entry, and the Coder
compositor reads the same entries from its grant. A pattern sets `exact`,
or `prefix` with an optional `holds`, and the module escapes it for
Hyprland's regular expressions. A chord that is already bound fails the
build, because Hyprland runs every bind a press matches.
`tests/extension-points.nix` sets the launchers and rules of a host with
Zoom, a slide deck, and Battle.net, and the `extension-points` check holds
the exact lines they render.

### The Coder compositor

`coderos.desktop.compositor = "coder"` starts the Coder compositor on tty1
instead, and `coderos.desktop.trialTty` runs the other compositor on a
second TTY as a trial or a way back. Either one writes the compositor's
grant to `/etc/coderos/compositor.json`: the pane, the terminal, the
command, the start list, the launchers that are on, and the host's extra
binds and rules. The compositor is `crates/coder-compositor`, built by
`pkgs/coder-compositor.nix`, and its session is
`bin/coder-compositor-session`; `compositorPackage` and
`compositorSessionPackage` name them by default, and a host that runs
neither compositor option builds neither. `compositorBinary` runs a build
from a checkout instead.

### The product workspace

`os/bin/oa-workspace` puts the whole product on one desk (desk 5) in four
panes: the Android emulator with the OpenAgents preview app on its Verse
tab, the website from `scripts/dev/full-local.sh` in a Chromium app window
with a demo chat, the Verse client in Everglade at the owner's workshop
(`verse --owners-house`), and two Coder panes running `coder-new`. Run it
from a checkout on the host, over SSH or from a pane:

```sh
os/bin/oa-workspace build   # origin/main (when the checkout is clean), then every build, through `openagents lease build`
os/bin/oa-workspace         # start what is not running, lay it out, show desk 5
os/bin/oa-workspace shot /tmp/workspace.png
os/bin/oa-workspace stop
```

Builds go to `~/work/openagents-target-workspace`, never to the host
service's `target/release`. The script's header lists its settings.

## Run the Coder host

Turn on both modules for the account that runs the host:

```nix
coderos.coderHost = {
  enable = true;
  user = "you";
};
coderos.coderUpdate.enable = true;
```

`coderUpdate.user` defaults to the host's account.

`coder-service` owns the host service: the unit, the launcher record, trial
updates, and rollback. The [host service guide](../docs/coder/runtime/host-service.md)
describes them. Nix does not render the unit. The host module declares
linger for the account and a user unit, `coderos-coder-host-install`, that
runs `coder-service service install` once and exits at once after that. A
rebuild never restarts or reconfigures a running host.

What happens on a new host:

1. The first `coder-update` run builds `coder` and `coder-service` from
   `main`, stages the binary as a bundle with `scripts/coder-host.py`, and
   links `~/.openagents/bin/coder` and `~/.openagents/bin/coder-service`.
2. The installer refuses until the host has an owner and relays. Run
   `coder host init --owner KEY --relay URL`, then
   `systemctl --user start coderos-coder-host-install`.
3. The installer reads the host key with `coder host public-key` and runs
   `coder-service service install`, which registers and starts the unit.
4. Each later run that finds a new commit builds it and runs
   `coder-service update --to <sha256> --wait 300`. On `committed`, the
   `coder` command moves to the new build. On `rolled-back`, the host keeps
   its previous version, and the run skips that commit until the branch
   moves.

Each run takes a lock, refuses to build with less than
`freeSpaceFloorGb` (40) gigabytes free, and checks that the new binary's
`--version` names its commit before it stages anything. The build runs at
`Nice` 10 with idle I/O, under `TasksMax` 2048 and `MemoryMax` 48G, so a
runaway build fails without taking the desktop down. `jobs` passes
`--jobs` to cargo and is unset by default. The last `keepBundles` (5)
bundles stay, along with any bundle the launcher record or the bundle
selection names.

The update does not run the test suite. A commit that compiles and fails a
test can be staged; the trial rolls it back only if the host fails to
report ready.

Read what happened:

```sh
journalctl -u coder-update -n 50
systemctl --user status org.openagents.coder-host
coder-service service status
coder-service descriptor
```

Limits:

- The launcher binary that the unit runs is the copy `service install`
  made. An update changes the `coder` the host runs, not the launcher. To
  move the launcher to a newer `coder-service`, run
  `coder-service service install` again with the committed version.
- The installer passes the label, listen address, and ready timeout. Other
  install options, such as extra state directories or host arguments, need
  a manual `coder-service service install`.

## Checks

Run the checks from the repository root:

```sh
nix flake check ./os
```

A check evaluates a stub host's toplevel without building it. The stub,
`tests/stub-host.nix`, supplies only file systems, a host name, and a state
version. `tests/all-capabilities.nix` turns every capability on. When you add
a module, add its option to that file. `tests/extension-points.nix` adds a
host's own launchers and window rules, and the `extension-points` check
compares what they render with the lines such a host reads today. The
`coder-compositor-host` check evaluates a host that runs the Coder
compositor on tty1 and Hyprland on a trial TTY.
The `example-workstation` check evaluates
`examples/workstation/configuration.nix` with `tests/stub-hardware.nix` in
place of the hardware file a real machine generates. When you change an
option the example sets, update the example in the same change.

The scripts under `bin/` have shell tests under `tests/` that stub every
program they call, so they need no desktop. Run them by hand:

```sh
os/tests/coder-pane.sh
os/tests/coder-close.sh
os/tests/coder-compositor-session.sh
os/tests/coder-browser.sh
os/tests/dictate-toggle.sh
os/tests/presentation-mode.sh
os/tests/screen-record.sh
os/tests/coderos-camera.sh
os/tests/recording-hud.sh
```

## Nix and shell in this repository

The Nix and shell under `os/` are infrastructure, in the same sense as the
repository's retained Python and shell tooling. Product behavior belongs in
Rust; a script here launches or configures a Rust program rather than
growing product logic of its own.
