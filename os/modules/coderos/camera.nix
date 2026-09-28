# The camera on a CoderOS desktop: one daemon that owns the node, the
# circle that shows it, and the key that toggles the circle.
#
# A UVC node streams to one process. Until 2026-09-17 the camera circle's
# `mpv` held `/dev/video0`, so nothing else could read it: no recording of
# the camera, no hand tracking while the circle was up, and no camera for a
# video call. `coderos-camera` opens the node once and serves every
# consumer: a `v4l2loopback` node for the programs that read a camera node,
# a recording to a file, and hand landmarks on a socket.
# `crates/coderos-camera/README.md` says how.
#
# This is a session capability like screen recording, so it hangs off
# `coderos.desktop` and does not exist without it. The daemon starts with
# the session, through `coderos.desktop.capabilityStart`, before the circle
# does, and the `camera` launcher row in `desktop.nix` gives the circle its
# window rule and SUPER + C.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.desktop;
  camera = cfg.camera;
  rendered = import ./camera-grant.nix { inherit camera; };

  # The circular camera view. mpv reads the loopback node the daemon
  # serves, or the camera itself on a host with no loopback, and masks the
  # picture to a disc on a transparent background, so the compositor shows
  # a circle whatever shape the window is, and `screen-record` catches it
  # because it is on the screen.
  cameraOverlay = pkgs.writeShellApplication {
    name = "camera-overlay";
    runtimeInputs = [ pkgs.mpv pkgs.procps cfg.deskPackage pkgs.jq pkgs.v4l-utils camera.package ];
    text = builtins.readFile ../../bin/camera-overlay;
  };

  # The one key that turns the camera view and its recording HUD on and off
  # together. It reads whether the `selfie` window is up and starts or stops
  # the pair, and it makes sure the daemon serves the node before the circle
  # opens it. recording-hud is on the system profile when the host has it,
  # reached through the inherited PATH behind a `command -v` guard, so a
  # host with the camera alone still toggles the camera.
  cameraToggle = pkgs.writeShellApplication {
    name = "camera-toggle";
    runtimeInputs = [ cfg.deskPackage pkgs.jq cameraOverlay camera.package ];
    text = builtins.readFile ../../bin/camera-toggle;
  };
in
{
  options.coderos.desktop.camera = {
    enable = lib.mkEnableOption "the camera daemon and a circular camera view from a webcam on this machine";

    device = lib.mkOption {
      type = lib.types.str;
      default = "/dev/video0";
      description = "The V4L2 node the daemon opens. The session sees it as `CODEROS_CAMERA_DEVICE`.";
    };

    width = lib.mkOption {
      type = lib.types.ints.positive;
      default = 1280;
      description = "The width the daemon asks the camera for, in pixels.";
    };

    height = lib.mkOption {
      type = lib.types.ints.positive;
      default = 720;
      description = ''
        The height the daemon asks the camera for, in pixels. The circle
        crops the picture to a square of this side. The session sees the
        size as `CODEROS_CAMERA_CAPTURE`.
      '';
    };

    framerate = lib.mkOption {
      type = lib.types.ints.positive;
      default = 30;
      description = ''
        The frame rate the daemon asks the camera for, and the rate a
        recording of the camera is held to.
      '';
    };

    loopback = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Whether the host loads `v4l2loopback` and the daemon serves a
          camera node on it for `mpv`, Zoom, and a sandboxed browser. Off,
          the circle reads the camera itself and nothing else can.
        '';
      };

      number = lib.mkOption {
        type = lib.types.ints.unsigned;
        default = 10;
        description = ''
          The loopback node's number: `/dev/video<number>`. Fixed, so the
          grant and the session name the same node across boots. The
          session sees the node as `CODEROS_CAMERA_LOOPBACK`.
        '';
      };

      label = lib.mkOption {
        type = lib.types.str;
        default = "CoderOS camera";
        description = "The name a program's camera picker shows for the node.";
      };
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ../../pkgs/coderos-camera.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ../../pkgs/coderos-camera.nix { }";
      description = ''
        The camera daemon `crates/coderos-camera` builds, on the session's
        PATH (`nix build ./os#coderos-camera`).
      '';
    };
  };

  config = lib.mkIf (cfg.enable && camera.enable) {
    environment.systemPackages = [ camera.package cameraOverlay cameraToggle ];

    # The `camera` launcher row: the circle's window rule and SUPER + C in
    # both compositors.
    coderos.desktop.launchers = [ "camera" ];

    # The daemon first, so it is up before the circle opens the loopback.
    # `recording-hud.nix` adds its strip after these.
    coderos.desktop.capabilityStart = [ "coderos-camera serve" "camera-overlay" ];

    # What the daemon reads at start. `CODEROS_CAMERA_GRANT` names another
    # file for a run by hand.
    environment.etc."coderos/camera.json".text = builtins.toJSON rendered.grant;

    environment.sessionVariables = rendered.sessionVariables;

    # logind puts an ACL on the device for whoever holds the active seat, so
    # a person at the machine can already read it. The group membership is
    # for everything that is not that: a service, an SSH login, a run.
    users.users.${cfg.user}.extraGroups = [ "video" ];

    # The loopback module, loaded at boot with a fixed node number so the
    # grant can name it. `camera-grant.nix` holds the options and why.
    boot.extraModulePackages = lib.mkIf camera.loopback.enable [ config.boot.kernelPackages.v4l2loopback ];
    boot.kernelModules = lib.mkIf camera.loopback.enable [ "v4l2loopback" ];
    boot.extraModprobeConfig = lib.mkIf camera.loopback.enable rendered.modprobe;

    # A module a switch adds is not in the booted system's module
    # directory, which is the one `modprobe` reads, so
    # `systemd-modules-load` cannot load it until a reboot. Load it
    # from the system just switched to — `/run/current-system` already
    # points there when activation runs — and trigger udev so the fresh
    # node takes its group and seat ACL. A module the running kernel
    # cannot take, such as one built for another release, fails this load
    # and waits for the reboot the kernel needs anyway.
    system.activationScripts.coderos-camera-loopback = lib.mkIf camera.loopback.enable ''
      if [ ! -d /sys/module/v4l2loopback ]; then
        if ${pkgs.kmod}/bin/modprobe -d /run/current-system/kernel-modules v4l2loopback; then
          ${pkgs.systemd}/bin/udevadm trigger --subsystem-match=video4linux --action=add || true
        fi
      fi
    '';
  };
}
