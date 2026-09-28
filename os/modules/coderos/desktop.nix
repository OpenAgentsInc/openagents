# The desktop session's shared options, and nothing else yet.
#
# The Hyprland session moves here in its own change (#9869), which replaces
# this file with the full module. Until then, this declares the two options
# the optional capabilities read, so that `android.nix` can name the account
# it runs for and stay off unless a desktop is on. It configures nothing.
{ lib, ... }:

{
  options.coderos.desktop = {
    enable = lib.mkEnableOption "the CoderOS desktop session";

    user = lib.mkOption {
      type = lib.types.str;
      description = ''
        The account the desktop session runs as. Optional capabilities such
        as the Android emulator grant their devices and paths to this
        account.
      '';
    };
  };
}
