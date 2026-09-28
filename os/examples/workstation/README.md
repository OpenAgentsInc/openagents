# Example workstation

This directory is a complete CoderOS host: a desktop machine that runs Coder
in a tiling Hyprland session, keeps the resident Coder host running, and
rebuilds `coder` from this repository on a timer. Copy it into a private
repository of your own and make it your machine.

| File | What it is |
| --- | --- |
| `flake.nix` | A host flake that imports `nixosModules.coderos` from this repository and follows its pinned nixpkgs. |
| `configuration.nix` | The host: your account, keys, and host name, the desktop, the Coder host service, and `coder-update`. Hardware that only some machines have, such as an NVIDIA card, is in comments that say when to turn it on. |

The flake also lists `hardware-configuration.nix`, which you generate on the
machine. `nix flake check ./os` evaluates `configuration.nix` against a stub
hardware file, so the example stays in step with the modules.

## What the example turns on

- The base system from `nixosModules.coderos`: systemd-boot, SSH with keys
  only, NetworkManager, Docker, and the developer packages.
- Tailscale, so your other machines reach this one by name.
- The Hyprland desktop on tty1, with Coder as its first window, dictation,
  screen recording, presentation mode, and the Coder Browser.
- The resident Coder host as a systemd user service, and the `coder-update`
  timer that builds `coder` from `main` and trials each new build.

It leaves off the Coder compositor, the camera, hand tracking, and the
Android emulator. The comments in `configuration.nix` say how to turn on an
NVIDIA card, CPU limits, `rasdaemon`, and compressed swap, and when a machine
wants each one.

## Make it your machine

1. Create a private repository, for example `~/coderos-hosts`, and copy
   this directory into it:

   ```sh
   mkdir ~/coderos-hosts
   cp ~/openagents/os/examples/workstation/{flake.nix,configuration.nix} ~/coderos-hosts/
   cd ~/coderos-hosts
   git init
   ```

1. Generate the hardware file on the machine that runs the host:

   ```sh
   nixos-generate-config --show-hardware-config > hardware-configuration.nix
   ```

   If you are installing from the NixOS installer, run
   `nixos-generate-config --root /mnt` instead and copy
   `/mnt/etc/nixos/hardware-configuration.nix`.

1. Edit `configuration.nix`. Replace the account `you`, the host name
   `coderos`, and the checkout path `/home/you/openagents` with your own,
   and add your SSH public key to `coderos.authorizedKeys`. With that list
   empty, only the console can log in.

1. Add every file to git and lock the inputs. A git flake ignores untracked
   files, so a file you have not added doesn't exist for Nix:

   ```sh
   git add flake.nix configuration.nix hardware-configuration.nix
   nix flake lock
   git add flake.lock
   ```

1. Build and switch:

   ```sh
   sudo nixos-rebuild switch --flake ~/coderos-hosts#coderos
   ```

   The name after `#` is the attribute in `flake.nix`. Rename
   `nixosConfigurations.coderos` when you rename the host.

1. Start the Coder host once, as the steps in
   [Run the Coder host](../../README.md#run-the-coder-host) describe.

## Edit the modules and your host together

When you change a CoderOS module in your checkout of this repository, point
your host at the checkout for that rebuild instead of the pinned revision:

```sh
sudo nixos-rebuild switch \
  --flake ~/coderos-hosts#coderos \
  --override-input openagents "git+file://$HOME/openagents?dir=os"
```

Nix reads tracked files from the working tree, including uncommitted edits,
so a change to `os/modules/coderos/desktop.nix` takes effect without a
commit. A new file needs `git add` first, in either repository, because a
git flake ignores untracked files. Root runs the rebuild and your account
owns the checkout, so the checkout has to be in
`coderos.git.safeDirectories`, which the example sets.

When the change is right, commit and push it to this repository, then pin
the new revision in your host flake:

```sh
cd ~/coderos-hosts
nix flake update openagents
git commit -am "Update CoderOS"
```

A change that is only about your machine, such as a new launcher or a
microphone gain, stays in your host flake and never touches this
repository. A private module adds its chords and window rules through
`coderos.desktop.extraBinds` and `extraWindowRules`; see
[Add your own launchers and window rules](../../README.md#add-your-own-launchers-and-window-rules).
