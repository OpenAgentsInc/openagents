# Verse OS's boot session (docs/os/2026-11-02-verse-os.md, milestone V0).
#
# A VT whose login shows the Verse instead of a shell: the login runs
# verse-session (os/verse-session.coax), which waits until that VT is on the
# screen, runs the first realm on the bare display (verse/gpu/vk-realm.c), and
# follows travel between realms. Escape held a second hands the screen to
# `leaveTo`; switching back to the VT starts the Verse again where you were.
#
# The realms are not built by Nix yet. `verse/gpu/realm.sh local install DIR`
# builds them from an openagents checkout into `dir`, the update agent's job
# from V1 on; until that directory holds a verse-session, the VT is an
# ordinary login.
{ config, lib, pkgs, ... }:

let
  cfg = config.verseos.session;
  tty = toString cfg.tty;
  stateDir = if cfg.stateDir == null then "" else cfg.stateDir;
  # the realms: a package (an image) or a directory realm.sh install keeps (a machine you develop on)
  dir = if cfg.package != null then "${cfg.package}/share/verse-os" else cfg.dir;
in
{
  options.verseos.session = {
    enable = lib.mkEnableOption "the Verse on a VT of its own";

    user = lib.mkOption {
      type = lib.types.str;
      example = "alice";
      description = ''
        The account whose login on `tty` runs the Verse. The VT must log
        this account in on its own (`services.getty.autologinUser`), so
        switching to it is all it takes.
      '';
    };

    tty = lib.mkOption {
      type = lib.types.ints.between 1 12;
      default = 3;
      description = ''
        The VT the Verse runs on. 1 makes the machine boot into it; any
        other is a key chord away (Ctrl+Alt+F<tty>) from whatever tty1 runs.
      '';
    };

    leaveTo = lib.mkOption {
      type = lib.types.ints.between 1 12;
      default = 1;
      description = "The VT shown when you leave the Verse (Escape held a second).";
    };

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = ''
        The realms built by Nix (os/nix/pkgs/verse-realms.nix). When set, `dir` is
        its share/verse-os, read only, and what the session writes goes to
        $XDG_RUNTIME_DIR/verse-os.
      '';
    };

    dir = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "/home/alice/.local/share/verse-os/current";
      description = ''
        What `verse/gpu/realm.sh local install DIR` keeps: a directory per
        realm, the travel file, and verse-session.
      '';
    };

    first = lib.mkOption {
      type = lib.types.str;
      default = "newland";
      description = "The realm the Verse opens on: newland is the Grid.";
    };

    gpuLimits = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Set the GPU's power ceiling with os/gpu-limits.coax (os/gpu-limits-policy.coax decides):
        the card's default while you play, 70% of it while the session serves the mesh as a
        Pylon. The session says which by writing 1 or 0 to /run/verse-os/serving; a path unit
        runs the limit again as root when it changes. NVIDIA cards only (nvidia-smi).
      '';
    };

    stateDir = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/home/alice/.local/state/verse";
      description = ''
        Where the player, place and bar are kept online (vk-realm --net).
        Null keeps the Verse offline.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = config.services.getty.autologinUser == cfg.user;
        message = ''
          verseos.session.user is "${cfg.user}", but services.getty.autologinUser
          is ${if config.services.getty.autologinUser == null then "unset" else "\"${config.services.getty.autologinUser}\""}.
          The VT's own login is what starts the Verse, so the two name the same account.
        '';
      }
      {
        assertion = cfg.package != null || cfg.dir != "";
        message = "verseos.session needs the realms: set package (an image) or dir (realm.sh local install DIR).";
      }
      {
        assertion = cfg.tty != cfg.leaveTo;
        message = "verseos.session.leaveTo is the Verse's own VT (${tty}); name the one to leave to.";
      }
    ];

    # The realm reads the keyboard and mice itself (evdev) and drives the
    # display through DRM; video is the card, input the devices.
    users.users.${cfg.user}.extraGroups = [ "input" "video" ];

    # Vulkan: the loader finds the driver's ICD under /run/opengl-driver.
    hardware.graphics.enable = true;
    environment.systemPackages = lib.optional (cfg.package != null) cfg.package;

    # the GPU's ceiling (V5): set at boot, and again when the session starts or stops serving
    systemd.tmpfiles.rules = lib.mkIf cfg.gpuLimits [ "d /run/verse-os 0755 ${cfg.user} users -" "f /run/verse-os/serving 0644 ${cfg.user} users - 0" ];
    systemd.services.verse-gpu-limits = lib.mkIf cfg.gpuLimits {
      description = "Verse OS: the GPU's power ceiling";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-tmpfiles-setup.service" ];
      path = [ "/run/current-system/sw" ];
      serviceConfig.Type = "oneshot";
      script = ''
        if [ "$(cat /run/verse-os/serving 2>/dev/null)" = 1 ]; then exec "${dir}/gpu-limits" --serving; else exec "${dir}/gpu-limits"; fi
      '';
    };
    systemd.paths.verse-gpu-limits = lib.mkIf cfg.gpuLimits {
      wantedBy = [ "multi-user.target" ];
      pathConfig.PathChanged = "/run/verse-os/serving";
    };

    programs.bash.loginShellInit = lib.mkAfter ''
      if [ "$(tty)" = "/dev/tty${tty}" ] && [ -z "$WAYLAND_DISPLAY" ] && [ "$USER" = "${cfg.user}" ] \
        && [ -x "${dir}/verse-session" ]; then
        run="''${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/verse-os"
        export VK_DRIVER_FILES="''${VK_DRIVER_FILES:-$(ls /run/opengl-driver/share/vulkan/icd.d/*.json 2>/dev/null | head -1)}"
        "${dir}/verse-session" --dir "${dir}" --run "$run" --first "${cfg.first}" --tty ${tty} \
          --leave-to ${toString cfg.leaveTo} ${lib.optionalString (stateDir != "") "--state \"${stateDir}\""} \
          </dev/null 2>&1 | ${pkgs.systemd}/bin/systemd-cat -t verse-session   # journalctl -t verse-session
      fi
    '';
  };
}
