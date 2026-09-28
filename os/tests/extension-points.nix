# What a private host flake sets through the desktop's extension points, so a
# check proves the public modules reproduce a host's own launchers and window
# rules without declaring the options its private modules own.
#
# The values are the ones a host with Zoom, a slide deck, and Battle.net sets
# today: three launcher chords, a floating Battle.net launcher, and World of
# Warcraft and StarCraft II clients that tile as a pane and ignore a
# fullscreen request. `flake.nix` checks that `/etc/coderos/hyprland.conf`
# holds each line exactly as the host's session reads it now, and that the
# Coder compositor's grant carries the same entries.
#
# The Coder compositor's packages are stubs here, so the check reads the
# grant without evaluating the compositor's build. The
# `coder-compositor-host` check evaluates the real ones.
{ pkgs, ... }:

let
  # A game client: a tile that a fullscreen or maximize request leaves in
  # its tile.
  game = {
    ignoreCase = true;
    float = false;
    suppressFullscreen = true;
  };
in
{
  imports = [ ./all-capabilities.nix ];

  coderos.desktop = {
    extraBinds = [
      { mods = "SUPER SHIFT"; key = "D"; command = "coder-deck-open"; }
      { mods = "SUPER"; key = "Z"; command = "coder-zoom"; }
      { mods = "SUPER"; key = "G"; command = "coder-battlenet"; }
    ];

    extraWindowRules = [
      {
        name = "Battle.net launcher";
        patterns = [
          { exact = "battle.net.exe"; }
          { exact = "Battle.net.exe"; }
          { exact = "steam_app_battlenet"; }
        ];
        float = true;
        center = true;
      }
      (game // {
        name = "World of Warcraft client, by class";
        patterns = [
          { prefix = "wow"; }
          { prefix = "world of warcraft"; }
          { prefix = "steam_app_"; holds = "wow"; }
        ];
      })
      (game // {
        name = "World of Warcraft client, by title";
        field = "title";
        patterns = [ { prefix = "World of Warcraft"; } ];
      })
      (game // {
        name = "StarCraft II client, by class";
        patterns = [
          { prefix = "sc2"; }
          { prefix = "starcraft"; }
          { prefix = "steam_app_"; holds = "sc2"; }
        ];
      })
      (game // {
        name = "StarCraft II client, by title";
        field = "title";
        patterns = [ { prefix = "StarCraft II"; } ];
      })
    ];

    # The Coder compositor on the trial TTY, so the grant is written.
    trialTty = 2;
    compositorPackage = pkgs.writeShellScriptBin "coder-compositor" "exit 0";
    compositorSessionPackage = pkgs.writeShellScriptBin "coder-compositor-session" "exit 0";
  };
}
