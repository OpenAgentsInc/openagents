# Bounds on how hard this machine's CPU may be driven.
#
# Off by default, and every bound is null until a host names a value, so
# turning the module on changes nothing a host does not ask for. The values
# belong to one machine's CPU and cooler; measure your own before you set
# them.
#
# Two levers, because they cover different loads:
#
#   watts         bounds a many-threaded load, where total package power is
#                 what reaches the junction temperature limit (Tjmax).
#   maxPerfPct    bounds a lightly-threaded load, where the package is
#                 nowhere near its power ceiling but one or two cores boost
#                 to maximum frequency and so to maximum voltage. That is the
#                 condition a degraded core faults at, so the frequency cap
#                 matters even though the power cap looks like it covers it.
#
# An example. On 2026-09-09 an Intel i7-14700K that logged Bank 0 machine
# checks under sustained load was measured with 28 threads, after its
# heatsink was cleaned out:
#
#   280 W, max_perf 100%    peak 100 C   throttle events 32312
#   125 W, max_perf  88%    peak  78 C   throttle events     0
#
# 125 W is Intel's base power for that part; the board ran it at more than
# twice that. That host set:
#
#   coderos.cpuLimits = {
#     enable = true;
#     watts = 125;
#     maxPerfPct = 88;
#     nixJobs = 6;
#     nixCores = 4;
#     cargoJobs = 20;
#   };
#
# Build parallelism is not a thermal lever. Under a fixed power ceiling,
# fewer threads is worse rather than better, because the same watts spread
# over fewer cores means each one boosts higher and sits at a higher voltage.
# The job counts leave headroom for the desktop during a build; they are not
# a safety measure.
#
# Every setting here is a mitigation, not a repair. A CPU that needs them
# still needs replacing, and a cooler that lets the package reach Tjmax in
# seconds needs reseating.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.cpuLimits;
  set = value: value != null;
in
{
  options.coderos.cpuLimits = {
    enable = lib.mkEnableOption "bounds on CPU power, boost frequency, and build parallelism";

    watts = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      example = 125;
      description = ''
        The package power ceiling in watts, applied to both the long-term and
        the short-term RAPL constraint. Setting both to one value also fixes
        a board that sets the sustained limit above the burst one. With null,
        the power limits stay as the firmware set them.
      '';
    };

    maxPerfPct = lib.mkOption {
      type = lib.types.nullOr (lib.types.ints.between 10 100);
      default = null;
      example = 88;
      description = ''
        Ceiling on the intel_pstate performance scale, which bounds peak
        boost frequency and so peak core voltage. This is the lever that
        applies to a lightly-threaded load, where the power ceiling does not
        bind. With null, the scale stays as it is.
      '';
    };

    nixJobs = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      example = 6;
      description = "Concurrent Nix derivations. With null, Nix's own setting applies.";
    };

    nixCores = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      example = 4;
      description = "Cores offered to each Nix derivation. With null, Nix's own setting applies.";
    };

    cargoJobs = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      example = 20;
      description = ''
        Default parallelism for a cargo build with no `-j` flag, set through
        `CARGO_BUILD_JOBS`. Set it below the thread count so a build leaves
        the desktop and any running agents responsive. With null, cargo uses
        every thread.
      '';
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    (lib.mkIf (set cfg.nixJobs) { nix.settings.max-jobs = cfg.nixJobs; })
    (lib.mkIf (set cfg.nixCores) { nix.settings.cores = cfg.nixCores; })

    # A plain `cargo build` in any checkout picks this up, so the bound holds
    # without every caller remembering a flag.
    (lib.mkIf (set cfg.cargoJobs) {
      environment.variables.CARGO_BUILD_JOBS = toString cfg.cargoJobs;
    })

    {
      # Reading the temperature should not require knowing the hwmon layout.
      environment.systemPackages = [ pkgs.lm_sensors ];
    }

    (lib.mkIf (set cfg.watts || set cfg.maxPerfPct) {
      # Re-assert on a schedule as well as at boot. More than this unit can
      # write the RAPL limits, because firmware and the management engine can
      # put them back, and a limit that quietly reverted looks exactly like a
      # limit that was never applied, until the machine faults. Every five
      # minutes costs nothing and makes the bound self-healing.
      systemd.timers.coderos-cpu-limits = {
        description = "Re-assert the CPU bounds periodically";
        wantedBy = [ "timers.target" ];
        # OnCalendar rather than OnUnitActiveSec, because the service below is
        # a oneshot: "active since" is not a useful clock to count from, and a
        # relative timer against a unit that finishes immediately schedules
        # nothing. An absolute schedule fires regardless of unit state.
        timerConfig = {
          OnCalendar = "*:0/5";
          Persistent = true;
          AccuracySec = "30s";
        };
      };

      systemd.services.coderos-cpu-limits = {
        description = "Bound CPU power and boost frequency";

        # post-resume.target does not exist on NixOS, so naming it would make
        # a symlink into a target that never runs and a bound that silently
        # disappears after the first suspend. The sleep targets below are the
        # documented way to order a unit after a resume: systemd starts a
        # unit that is both After= and WantedBy= a sleep target when that
        # target is left, which is on the way back up.
        wantedBy = [ "multi-user.target" "suspend.target" "hibernate.target" "hybrid-sleep.target" ];
        after = [ "sysinit.target" "suspend.target" "hibernate.target" "hybrid-sleep.target" ];

        # Not RemainAfterExit: a unit that stays active cannot be started
        # again, so the timer above would fire into a no-op and the periodic
        # re-assert would do nothing. Letting the oneshot finish and go
        # inactive is what makes re-running it possible.
        serviceConfig.Type = "oneshot";

        # The limits live in sysfs, and a reboot or a resume resets them,
        # which is why this is a unit rather than a note in a runbook. A
        # constraint the firmware has locked is reported and skipped rather
        # than failing the unit, because a locked limit is a reason to change
        # the firmware settings, not a reason to leave the machine without a
        # boot.
        script = ''
          set -u
        '' + lib.optionalString (set cfg.watts) ''
          applied=0

          for pkgdir in /sys/class/powercap/intel-rapl:[0-9]*; do
            case "$(basename "$pkgdir")" in *:*:*) continue ;; esac
            [ -r "$pkgdir/name" ] || continue

            for c in 0 1; do
              limit="$pkgdir/constraint_''${c}_power_limit_uw"
              [ -w "$limit" ] || continue
              if echo ${toString (cfg.watts * 1000000)} > "$limit" 2>/dev/null; then
                applied=$((applied + 1))
              else
                echo "constraint $c on $(basename "$pkgdir") is locked; leaving it at $(cat "$limit" 2>/dev/null)"
              fi
            done
          done

          echo "power ceiling ${toString cfg.watts} W applied to $applied constraint(s)"
        '' + lib.optionalString (set cfg.maxPerfPct) ''
          pstate=/sys/devices/system/cpu/intel_pstate/max_perf_pct
          if [ -w "$pstate" ]; then
            echo ${toString cfg.maxPerfPct} > "$pstate"
            echo "max_perf_pct set to $(cat "$pstate")"
          else
            echo "intel_pstate is not in use; no frequency ceiling applied"
          fi
        '';
      };
    })
  ]);
}
