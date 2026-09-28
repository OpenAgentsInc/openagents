# Presentation mode: a gesture that sets the screen up to be watched, and one
# that puts it back.
#
# This is a session capability like the two in `capture.nix`, so it hangs off
# `coderos.desktop` and does not exist without one. It lives in its own file
# because it is its own capability: `screen-record` can enter the mode when a
# caller asks, and does not enter it on its own. A recording that rearranged
# somebody's desktop without being asked would startle them.
#
# `os/bin/presentation-mode` holds the reasoning about what is restored and
# where the captured state is written. SUPER + P runs it.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.desktop;

  presentationMode = pkgs.writeShellApplication {
    name = "presentation-mode";
    runtimeInputs = [ cfg.deskPackage pkgs.jq pkgs.coreutils ];
    text = builtins.readFile ../../bin/presentation-mode;
  };
in
{
  options.coderos.desktop.presentation = {
    enable = lib.mkEnableOption "a mode that sets the screen up to be recorded and puts it back";

    scale = lib.mkOption {
      type = lib.types.str;
      default = "1.25";
      description = ''
        The monitor scale the mode asks for. Hyprland rounds a scale that
        would leave a fractional buffer to the nearest one that does not, so
        this is a request rather than a setting. 1.25 divides 2560x1440 and
        1920x1080 exactly.
      '';
    };
  };

  config = lib.mkIf (cfg.enable && cfg.presentation.enable) {
    environment.systemPackages = [ presentationMode ];
    environment.sessionVariables.CODEROS_PRESENTATION_SCALE = cfg.presentation.scale;
    coderos.desktop.launchers = [ "presentation" ];
  };
}
