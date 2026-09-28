# The desk command: the one way a script on this desktop asks what is on the
# screens and changes a window.
#
# A host that leaves `coderos.desktop.enable` off gets nothing from this
# file. With the desktop on, the session holds `coder-desk`, the command
# `crates/coder-desk-cli` builds over `crates/coder-desk`, packaged by
# `os/pkgs/coder-desk.nix`. Every script under `os/bin/` runs it, and each
# names it through `runtimeInputs` rather than through the session's PATH,
# so a script resolves the command the host granted.
#
# The command replaces `hyprctl` in those scripts. Each script used to hold
# Hyprland's verb spellings, JSON field names, and selector grammar, and
# each would have changed again when the compositor did. `desktop.nix`
# still installs Hyprland, which is the compositor the session runs; what
# changed is that a script no longer names it.
#
# The package carries no compositor of its own. Its Hyprland backend speaks
# the session's control socket directly, reading
# `HYPRLAND_INSTANCE_SIGNATURE` and `XDG_RUNTIME_DIR` from the environment
# the session sets.
{ config, lib, pkgs, ... }:

let
  desktop = config.coderos.desktop;
in
{
  options.coderos.desktop.deskPackage = lib.mkOption {
    type = lib.types.package;
    default = pkgs.callPackage ../../pkgs/coder-desk.nix { };
    defaultText = lib.literalExpression "pkgs.callPackage ../../pkgs/coder-desk.nix { }";
    description = ''
      The `coder-desk` command the session's scripts ask the desk through,
      built from this repository (`nix build ./os#coder-desk`). A host that
      runs a build from a checkout names it here instead.
    '';
  };

  config = lib.mkIf desktop.enable {
    # In the session's packages as well as in each script's runtime inputs,
    # so a person on the seat can ask the same questions a script asks.
    environment.systemPackages = [ desktop.deskPackage ];
  };
}
