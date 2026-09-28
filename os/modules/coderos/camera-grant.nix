# What `coderos.desktop.camera` renders, as a function of the option
# values alone, so `os/tests/coderos-camera.sh` evaluates it with
# `nix-instantiate` and no nixpkgs: the grant the daemon reads at
# `/etc/coderos/camera.json`, the variables the session sees, and the
# `modprobe` options the loopback module loads with.
{ camera }:
let
  loopbackNode = "/dev/video${toString camera.loopback.number}";
  loopback = if camera.loopback.enable then loopbackNode else null;
in
{
  # The daemon's grant. `crates/coderos-camera/src/config.rs` reads it.
  grant = {
    device = camera.device;
    width = camera.width;
    height = camera.height;
    framerate = camera.framerate;
    inherit loopback;
  };

  # What the session sees. `camera-overlay` reads the loopback node and
  # the capture size, and everything reads the camera itself from
  # `CODEROS_CAMERA_DEVICE`.
  sessionVariables = {
    CODEROS_CAMERA_DEVICE = camera.device;
    CODEROS_CAMERA_CAPTURE = "${toString camera.width}x${toString camera.height}";
  } // (if camera.loopback.enable then { CODEROS_CAMERA_LOOPBACK = loopbackNode; } else { });

  # The module's options. `exclusive_caps=1` makes the node a capture
  # device alone once the daemon writes it, which is what Zoom and a
  # browser look for; a fixed `video_nr` keeps the node's name the same
  # across boots and camera replugs, so the grant can name it.
  modprobe =
    if camera.loopback.enable then
      "options v4l2loopback exclusive_caps=1 video_nr=${toString camera.loopback.number} card_label=\"${camera.loopback.label}\""
    else
      "";

  inherit loopbackNode;
}
