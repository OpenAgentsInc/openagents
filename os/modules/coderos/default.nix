# What every CoderOS host has, whatever else it does.
#
# A host file adds the hardware, the GPU, the accounts, and the state
# version. This file holds only what is true of all of them. Every optional
# capability lives in its own file below and is off until a host turns it on.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos;
in
{
  # One file per capability. A later module joins this list; none of them
  # contributes anything to a host that leaves its option off.
  imports = [
    ./desktop.nix
    ./compositor.nix
    ./desk.nix
    ./capture.nix
    ./presentation.nix
    ./browser.nix
    ./camera.nix
    ./hands.nix
    ./recording-hud.nix
    ./android.nix
    ./tailscale.nix
    ./cpu-limits.nix
    ./coder-host.nix
    ./coder-update.nix
  ];

  options.coderos = {
    authorizedKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        SSH public keys that may reach root. Password authentication is off,
        so this list, and the keys a host gives its own accounts, are the
        only way in over the network. A console login with an account
        password remains the fallback.
      '';
    };

    sudo.wheelNeedsPassword = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether a member of `wheel` has to give a password to sudo.

        Password authentication over SSH is off, so an account that reaches
        the host with a key may have no password to give. A host used by one
        person can set this to false so that the account, and an agent
        working as it, can run `nixos-rebuild switch` on the host itself.
        That makes anything running as a `wheel` account root on the host.
        Leave it on for any machine more than one person uses.
      '';
    };

    git.safeDirectories = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "/srv/checkouts/openagents" ];
      description = ''
        Checkouts that root's git trusts although another account owns them,
        written to the system git configuration as `safe.directory`.

        `nixos-rebuild` runs as root, and git refuses a repository another
        user owns unless it is named safe. A flake that builds from a
        checkout owned by a person, such as one fetched with
        `--override-input openagents git+file://...`, then falls back to a
        path-only evaluation under sudo and fails. Name the checkouts a host
        rebuilds from here. The list stays empty by default, because a
        wildcard would trust every repository on the machine.
      '';
    };
  };

  config = {
    # systemd-boot writes its own EFI variable, so the firmware always holds a
    # live entry. A bootloader that leaves the entry to something else is how a
    # machine ends up in its firmware setup screen with nothing to boot.
    boot.loader.systemd-boot.enable = lib.mkDefault true;
    boot.loader.efi.canTouchEfiVariables = lib.mkDefault true;
    boot.loader.systemd-boot.configurationLimit = lib.mkDefault 10;

    nixpkgs.config.allowUnfree = true;
    nix.settings.experimental-features = [ "nix-command" "flakes" ];

    programs.git.enable = true;
    programs.git.config = lib.mkIf (cfg.git.safeDirectories != [ ]) {
      safe.directory = cfg.git.safeDirectories;
    };

    security.sudo.wheelNeedsPassword = cfg.sudo.wheelNeedsPassword;

    # Runs dynamically linked foreign binaries: rustup toolchains, tools from
    # `cargo install`, and editor servers. The repository pins its Rust
    # version in `rust-toolchain.toml`, which rustup resolves and the nixpkgs
    # rustc does not, so a host without this cannot build the workspace as
    # pinned.
    programs.nix-ld.enable = true;

    # The console is a 16-color VGA text mode with no truecolor, so a
    # program's colours collapse to the nearest ANSI slot. The Linux console
    # lets those 16 entries be redefined, so every slot is set to a rung of
    # the OpenAgents white ladder (white, 75, 50, 25 on near-black; see
    # `desktop.nix`). The screen is then white whichever slot a program
    # reaches for.
    console.colors = [
      "0a0a0a" # near black
      "8a8a8a" # white 50
      "8a8a8a"
      "ffffff" # white
      "4a4a4a" # white 25
      "8a8a8a"
      "c8c8c8" # white 75
      "c8c8c8"
      "1a1a1a" # near black, raised
      "c8c8c8"
      "c8c8c8"
      "ffffff"
      "8a8a8a"
      "c8c8c8"
      "ffffff"
      "ffffff"
    ];

    # `~/.local/bin` holds binaries built from a checkout, such as the
    # `coder` that `scripts/install-coder.sh` links there. A login shell must
    # find them without each account editing its own profile.
    environment.localBinInPath = true;

    # A machine check logs at pr_emerg, which no console loglevel above zero
    # suppresses, so it prints straight over whatever is drawing on the tty.
    # The console a person reads is a display, not a log sink. The journal
    # keeps every message either way, and `dmesg --console-on` restores it.
    systemd.services.quiet-console = {
      description = "Stop kernel messages printing over the console";
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.util-linux}/bin/dmesg --console-off";
      };
    };

    # Kernel messages print over whatever is on tty1, so a login prompt ends
    # up interleaved with hex. They are still in the journal; this only stops
    # them from being painted on the console a person is reading.
    boot.consoleLogLevel = 3;
    boot.kernel.sysctl."kernel.printk" = "3 4 1 3";

    networking.networkmanager.enable = lib.mkDefault true;
    time.timeZone = lib.mkDefault "UTC";
    i18n.defaultLocale = lib.mkDefault "en_US.UTF-8";

    # Coder's durable tasks can run each command in a container, which this
    # daemon provides.
    virtualisation.docker.enable = lib.mkDefault true;

    services.openssh = {
      enable = true;
      settings.PasswordAuthentication = false;
      settings.PermitRootLogin = "prohibit-password";
    };

    users.users.root.openssh.authorizedKeys.keys = cfg.authorizedKeys;

    environment.systemPackages = with pkgs; [
      git gh curl wget vim tmux htop jq ripgrep fd
      # rustup, not the nixpkgs rustc: a checkout builds at the version
      # `rust-toolchain.toml` pins, and rustup is what reads that file. It
      # installs the pinned toolchain on first use in a checkout, and nix-ld
      # above runs it. Only one of the two packages can be here, because each
      # supplies bin/cargo and the system path refuses the collision.
      rustup
      gcc pkg-config openssl
      # The retained Python tooling and the `#!/usr/bin/env python3`
      # stand-ins some tests spawn. A child with a sanitized environment
      # finds an interpreter on the system path alone, and a person's
      # profile is not on it.
      python3
      # The namespace tool `crates/coder-boundary` uses to enforce its
      # filesystem write boundary on Linux. Without it, the boundary refuses
      # to run a command rather than run it unbounded.
      bubblewrap
      pciutils usbutils
    ];
  };
}
