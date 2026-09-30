# Run the resident Coder host, `coder host serve`, as a service of one
# account that starts at boot.
#
# `coder-service` owns the host service: it renders the systemd user unit,
# keeps the launcher record, runs trial updates against a state snapshot,
# and rolls back. Read `docs/coder/runtime/host-service.md`.
#
# Why this module runs `coder-service service install` rather than declaring
# the unit in Nix:
#
#   - The unit's shape is `coder-service`'s. Its `ExecStart` names a copy of
#     the launcher under a digest directory in the host root, and install
#     writes the configuration and launcher record that `run` reads. A unit
#     rendered by Nix would be a second copy of that shape, and the next
#     change to it in Rust would leave the two disagreeing.
#   - Which version the host runs changes at run time, through
#     `coder-service update`, and a NixOS rebuild must not reset it. The
#     launcher record holds that state, and only the launcher writes it.
#   - `coder-service service status` compares the registered unit with what
#     it would render now, so a unit it did not write reads as drift.
#
# So the module declares only what belongs to the system: linger for the
# account, so the account's service manager runs from boot without a login,
# and a user unit that runs the installer once. The installer exits at once
# when the host service is already installed, so a rebuild or a login never
# touches a running host. Once the OpenAgents desktop app runs the host
# (its `com.openagents.desktop.host.service`), the installer's unit does not
# run at all.
#
# Install needs three things this module cannot supply:
#
#   - A staged bundle and a `coder-service` binary. `coderos.coderUpdate`
#     builds both from this repository; without it, stage one by hand as
#     `docs/coder/runtime/portable-host.md` describes.
#   - An owner and relays, from `coder host init`. The installer refuses
#     until `~/.openagents/host/serve.json` exists.
#   - The host key, which the installer reads with `coder host public-key`.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.coderHost;

  installUnit = "coderos-coder-host-install";

  # The systemd user unit the OpenAgents desktop app registers to run the
  # host (`crates/openagents-desktop/src/platform/linux.rs`).
  appHostUnit = "com.openagents.desktop.host.service";

  # No `runtimeInputs`: install records the PATH it runs with as the host's
  # path, and a Nix store path there would go stale after a garbage
  # collection. The unit's PATH below names the system profile, which holds
  # `jq` and the core utilities through `default.nix`.
  coderHostInstall = pkgs.writeShellApplication {
    name = "coder-host-install";
    text = builtins.readFile ../../bin/coder-host-install;
  };
in
{
  options.coderos.coderHost = {
    enable = lib.mkEnableOption "the resident Coder host as a systemd user service that starts at boot";

    user = lib.mkOption {
      type = lib.types.str;
      description = "The account that runs the Coder host. Its home holds `~/.openagents/host`.";
    };

    label = lib.mkOption {
      type = lib.types.str;
      default = "org.openagents.coder-host";
      description = "The systemd user unit name, without `.service`, that `coder-service` installs.";
    };

    listen = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:47100";
      description = ''
        The loopback address the host binds. `coder-service` refuses any
        address that is not loopback; remote devices reach the host through
        its relays and reach hints instead.
      '';
    };

    readyTimeoutSecs = lib.mkOption {
      type = lib.types.ints.positive;
      default = 60;
      description = "Seconds a trial update has to report ready before `coder-service` rolls it back.";
    };

    extraPath = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "/opt/tools/bin" ];
      description = ''
        Directories added to the search path `coder-service` records at
        install. The host and every terminal it opens inherit that path, so
        name here any directory whose tools a task needs. The default
        already holds the account's `~/.openagents/bin`, `~/.local/bin`, and
        `~/.cargo/bin`, and the NixOS system and per-user profiles.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # The account's service manager starts at boot and keeps running after
    # the last logout, so the host does too. NixOS enforces this setting on
    # every activation, so `coder-service` is never asked to run
    # `loginctl enable-linger`.
    users.users.${cfg.user}.linger = true;

    environment.systemPackages = [ coderHostInstall ];

    systemd.user.services.${installUnit} = {
      description = "Install the Coder host service with coder-service";
      wantedBy = [ "default.target" ];
      unitConfig = {
        ConditionUser = cfg.user;
        # The OpenAgents desktop app runs the host on this account once it
        # has registered its own unit, or adopted the old service (renaming
        # `service.json` to `service.adopted.json`). The installer then
        # stands down: systemd skips it, at login and when `coder-update`
        # starts it, so no rebuild installs a second host on the same
        # state and nobody has to mask it by hand. `%E` is the account's
        # configuration directory (`$XDG_CONFIG_HOME`, else `~/.config`),
        # where the app writes its unit. The installer script checks the
        # same two files.
        ConditionPathExists = [
          "!%E/systemd/user/${appHostUnit}"
          "!%h/.openagents/host/service.adopted.json"
        ];
      };

      environment = {
        CODER_HOST_LABEL = cfg.label;
        CODER_HOST_LISTEN = cfg.listen;
        CODER_HOST_READY_TIMEOUT = toString cfg.readyTimeoutSecs;
        # Install records this path, after `/usr/bin:/bin`, as the path of
        # the host and its terminals. On NixOS those two directories hold
        # only `env` and `sh`, so the system profile has to be on it.
        # systemd expands `%h` and `%u` to the account's home and name. This
        # replaces the Nix store paths NixOS gives every unit, which would
        # go stale in the recorded path after a garbage collection; the
        # system profile holds the same tools.
        PATH = lib.mkForce (lib.concatStringsSep ":" ([
          "%h/.openagents/bin"
          "%h/.local/bin"
          "%h/.cargo/bin"
          "/run/wrappers/bin"
          "/etc/profiles/per-user/%u/bin"
          "/run/current-system/sw/bin"
        ] ++ cfg.extraPath));
      };

      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${coderHostInstall}/bin/coder-host-install";
        # An install that refused, for example before `coder host init`,
        # waits for the next login, boot, or update rather than retrying.
        Restart = "no";
      };
    };
  };
}
