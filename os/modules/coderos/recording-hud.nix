# The recording HUD: the strip that docks under the camera circle and shows
# what the microphone is capturing while it starts and stops a recording.
#
# This is a session capability like the ones in `capture.nix`, so it hangs off
# `coderos.desktop` and does not exist without one. It needs two of that file's
# capabilities as well: the camera view it docks under and the screen recording
# its button drives. A host with either off gets nothing from this file, so the
# strip is present exactly when there is a circle to sit under and a recorder to
# start.
#
# The strip is a foot terminal drawing a meter, the microphone's name, a record
# button, and a resize control, and `os/bin/recording-hud` holds the reasoning
# for why it is a terminal and not a layer-shell surface or a browser window.
# The short version is that it has to follow the camera by title with
# `coder-desk shape`, the way camera-overlay places itself, and foot is already the
# session's white-themed terminal. The level comes from a second `pw-record` on
# the node `screen-record microphone` reports, which PipeWire shares, so the
# meter does not hold the device against the recorder's own capture, and the
# strip turns red when that node is the wrong one or delivers nothing.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.desktop;

  # The script is a real file under os/bin so it can be read and linted as
  # shell. writeShellApplication supplies the shebang, sets the shell options,
  # runs shellcheck at build time, and puts the runtime tools on PATH by name.
  # `screen-record` is not named here: it is the sibling capability's own
  # command on the system profile, reached through the inherited PATH behind a
  # `command -v` guard, the way presentation-mode reaches camera-overlay.
  recordingHud = pkgs.writeShellApplication {
    name = "recording-hud";
    runtimeInputs = [
      pkgs.foot
      pkgs.pipewire
      pkgs.coreutils
      pkgs.gawk
      cfg.deskPackage
      pkgs.jq
      pkgs.procps
    ];
    text = builtins.readFile ../../bin/recording-hud;
  };
in
{
  config = lib.mkIf (cfg.enable && cfg.camera.enable && cfg.screenRecording.enable) {
    environment.systemPackages = [ recordingHud ];

    # The strip opens after the camera's own rows, which it docks under;
    # it waits for the circle, so the order is for tidiness, not safety.
    coderos.desktop.capabilityStart = lib.mkAfter [ "recording-hud" ];
  };
}
