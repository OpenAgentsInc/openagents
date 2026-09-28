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
| `devShells.x86_64-linux.android` | The Android SDK, NDK, JDK 17, Gradle, and `cargo-ndk` that `scripts/build-coder-android.sh` and `scripts/build-openagents-android.sh` build with. |
| `packages.x86_64-linux` | Empty for now. Modules that run a program from this workspace add its build here. |
| `checks.x86_64-linux` | Evaluates a stub host with the base module alone, and with every capability turned on. |

`nixpkgs` is pinned to one revision in `flake.nix` and `flake.lock`. A host
flake that follows this input builds against the same packages the checks
evaluated.

## Use it from your own host flake

Keep your hardware file, accounts, keys, and personal modules in a private
flake of your own. Import the module set and add your host:

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
| `default.nix` | `coderos.authorizedKeys`, `coderos.sudo.wheelNeedsPassword`, `coderos.git.safeDirectories` | The base system: systemd-boot, flakes, `nix-ld` for rustup toolchains, SSH with keys only, NetworkManager, Docker, the amber console palette, quiet kernel messages on the console, and base developer packages. Sudo asks `wheel` for a password unless a host turns that off. Root's git trusts only the checkouts you name. |
| `desktop.nix` | `coderos.desktop.enable`, `coderos.desktop.user` | For now, only the two desktop options that other modules read. The Hyprland session replaces this file in [#9869](https://github.com/OpenAgentsInc/openagents/issues/9869). |
| `android.nix` | `coderos.desktop.android.*` | The Android emulator on KVM with one system image, launched by `bin/android-emulator`. It turns on Xwayland for the session and writes what it installed to `/etc/coderos/android.json`. Needs the desktop. |
| `tailscale.nix` | `coderos.tailscale.*` | Joins the host to a tailnet so other machines reach it by a stable name. The auth key stays in a file on the host, never in the Nix store. Tailscale SSH stays off unless you turn it on. |
| `cpu-limits.nix` | `coderos.cpuLimits.*` | Caps CPU package power, boost frequency, and build parallelism, and reapplies the caps every five minutes and after a resume. Every value is null until you set it. The file holds one measured example. |

## Checks

Run the checks from the repository root:

```sh
nix flake check ./os
```

A check evaluates a stub host's toplevel without building it. The stub,
`tests/stub-host.nix`, supplies only file systems, a host name, and a state
version. `tests/all-capabilities.nix` turns every capability on. When you add
a module, add its option to that file.

## Nix and shell in this repository

The Nix and shell under `os/` are infrastructure, in the same sense as the
repository's retained Python and shell tooling. Product behavior belongs in
Rust; a script here launches or configures a Rust program rather than
growing product logic of its own.
