# Keep this machine's Coder on a branch of this repository.
#
# A timer runs `bin/coder-update`, which builds `coder` from a checkout of
# the branch and hands the binary to the Coder host service with
# `coder-service update --to <sha256>`. The host service runs it as a trial
# against a snapshot of the host's state, keeps it when the host reports
# ready, and rolls back otherwise. Trial and rollback are `coder-service`'s,
# in Rust; this module only builds, checks, and stages. With no host service
# installed, it installs the `coder` command and nothing more.
#
# This was reimplemented from the private CoderOS update module, which
# built another program and swapped a symbolic link itself.
#
# The bound that matters most is `TasksMax`. On 2026-09-11 a test run on a
# CoderOS host spawned 7,472 processes, took all 125 GB of memory, and the
# out-of-memory killer took the compositor with it. A build is the load most
# likely to do that again, so the unit may not exceed a fixed number of
# tasks, and the kernel refuses the fork rather than the machine falling
# over. `MemoryMax` is the same reasoning for memory: the build fails, and
# the desktop keeps running.
#
# Build parallelism is left alone by default. `cpu-limits.nix` explains why
# narrowing a build under a fixed power ceiling makes the thermal behavior
# worse: the same watts over fewer cores means each one boosts higher. Set
# `jobs` only when you have measured a reason.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.coderUpdate;
  host = config.coderos.coderHost;

  # writeShellApplication supplies the shebang, sets the shell options, runs
  # shellcheck at build time, and puts the runtime tools on PATH by name. The
  # Rust toolchain is not one of them: the workspace pins Rust in
  # `rust-toolchain.toml`, which rustup resolves and the nixpkgs rustc does
  # not, so the script finds the account's own rustup.
  coderUpdate = pkgs.writeShellApplication {
    name = "coder-update";
    runtimeInputs = [
      pkgs.git
      pkgs.coreutils
      pkgs.findutils
      pkgs.util-linux
      pkgs.gnugrep
      pkgs.gnused
      pkgs.jq
      pkgs.python3
      pkgs.systemd
    ];
    text = builtins.readFile ../../bin/coder-update;
  };
in
{
  options.coderos.coderUpdate = {
    enable = lib.mkEnableOption "building `coder` from this repository and handing it to the Coder host service";

    user = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = if host.enable then host.user else null;
      defaultText = lib.literalExpression "config.coderos.coderHost.user when the Coder host is on";
      description = "The account whose Coder this maintains. It must be the account that runs the Coder host.";
    };

    repository = lib.mkOption {
      type = lib.types.str;
      default = "https://github.com/OpenAgentsInc/openagents.git";
      description = "The repository to build from.";
    };

    branch = lib.mkOption {
      type = lib.types.str;
      default = "main";
      description = "The branch to follow. Build any other branch by hand.";
    };

    interval = lib.mkOption {
      type = lib.types.str;
      default = "hourly";
      description = ''
        How often to look for a new commit, as a systemd calendar
        expression. A run that finds the commit it already installed exits
        without building, so a short interval costs one fetch.
      '';
    };

    waitSecs = lib.mkOption {
      type = lib.types.ints.positive;
      default = 300;
      description = ''
        How long a run waits for the host service to commit or roll back a
        trial. A run that stops waiting leaves the decision to the launcher
        and reads the outcome next time.
      '';
    };

    keepBundles = lib.mkOption {
      type = lib.types.ints.positive;
      default = 5;
      description = ''
        How many staged bundles to keep, newest first. A bundle that the
        host's launcher record or the bundle selection still names is always
        kept. Each bundle is one release binary, about 60 MB.
      '';
    };

    tasksMax = lib.mkOption {
      type = lib.types.ints.positive;
      default = 2048;
      description = ''
        The most processes the build may have at once. This bounds a
        runaway rather than tuning the build: a `cargo build` of this
        workspace needs a few hundred.
      '';
    };

    memoryMax = lib.mkOption {
      type = lib.types.str;
      default = "48G";
      description = "The memory ceiling for the build, so a runaway fails before the desktop does.";
    };

    freeSpaceFloorGb = lib.mkOption {
      type = lib.types.ints.positive;
      default = 40;
      description = "Refuse to build when less than this many gigabytes are free.";
    };

    jobs = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      description = "Passed to `cargo --jobs`. With null, cargo uses its default.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [{
      assertion = cfg.user != null;
      message = "coderos.coderUpdate.user must name an account when coderos.coderHost is off.";
    }];

    environment.systemPackages = [ coderUpdate ];

    # `~/.openagents/bin` holds the `coder` and `coder-service` this
    # maintains, and it comes before `~/.local/bin`, where
    # `scripts/install-coder.sh` links a checkout's build.
    environment.sessionVariables.PATH = [ "$HOME/.openagents/bin" ];

    systemd.services.coder-update = {
      description = "Build coder from ${cfg.branch} and hand it to the Coder host service";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      # What a release build needs beyond the script's own tools. The
      # toolchain's `gcc-ld` wrapper is a script that starts with
      # `#!/usr/bin/env bash`, so a unit without bash fails at the link step
      # with `env: 'bash': No such file or directory`.
      path = with pkgs; [ bash gcc binutils pkg-config ];

      environment = {
        CODER_UPDATE_REPOSITORY = cfg.repository;
        CODER_UPDATE_BRANCH = cfg.branch;
        CODER_UPDATE_FLOOR_GB = toString cfg.freeSpaceFloorGb;
        CODER_UPDATE_WAIT_SECS = toString cfg.waitSecs;
        CODER_UPDATE_KEEP_BUNDLES = toString cfg.keepBundles;
      } // lib.optionalAttrs host.enable {
        # With no host service installed yet, the run starts the Coder host
        # module's installer once the command is in place.
        CODER_UPDATE_HOST_INSTALL_UNIT = "coderos-coder-host-install.service";
      } // lib.optionalAttrs config.programs.nix-ld.enable {
        # The toolchain's linker, `rust-lld`, is a dynamically linked
        # foreign binary, and nix-ld runs it through these two variables. A
        # login shell gets them from `/etc/set-environment`; a systemd unit
        # does not, and the link step then fails with
        # `collect2: error: ld returned 127 exit status`.
        NIX_LD = "/run/current-system/sw/share/nix-ld/lib/ld.so";
        NIX_LD_LIBRARY_PATH = "/run/current-system/sw/share/nix-ld/lib";
      } // lib.optionalAttrs (cfg.jobs != null) {
        CODER_UPDATE_JOBS = toString cfg.jobs;
      };

      serviceConfig = {
        Type = "oneshot";
        User = cfg.user;
        ExecStart = "${coderUpdate}/bin/coder-update";

        # The bounds. A build that exceeds either fails alone.
        TasksMax = cfg.tasksMax;
        MemoryMax = cfg.memoryMax;

        # A background build yields to whatever the person is doing.
        Nice = 10;
        IOSchedulingClass = "idle";
        CPUSchedulingPolicy = "batch";

        # A failed build is a normal outcome, not a reason to retry in a
        # loop: the next run tries again.
        Restart = "no";
      };
    };

    systemd.timers.coder-update = {
      description = "Look for a new commit of coder to build";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = cfg.interval;
        # A machine that was asleep at the scheduled time still builds.
        Persistent = true;
        # Two hosts on one schedule should not fetch in the same second.
        RandomizedDelaySec = "5m";
      };
    };
  };
}
