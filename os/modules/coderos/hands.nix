# Hand tracking on a CoderOS desktop: the option the compositor's gesture
# module reads, and the Jev seam beside it.
#
# The camera daemon owns the camera and publishes hand landmarks on its
# socket (os/modules/coderos/camera.nix, `crates/coderos-camera`). On, the
# Coder compositor reads that socket and turns the landmarks into desk
# input: an extended index finger moves the pointer, a pinch presses and
# drags, a flat palm swiped sideways switches desks, and a fist held for
# two seconds sends Escape. `crates/coder-hands/src/gestures.rs` holds the
# rules and `docs/os/camera-and-hands.md` says how to use them.
#
# Beside those deterministic rules sits the Jev seam that judges the
# windows they cannot settle (`docs/os/hands-judge.md`). The seam is
# `coder_hands::judge` and `coder_hands::seam`, the compositor calls it
# through `coder_hands::watch`, it is off unless
# `coderos.desktop.hands.judge` is on, and on it records rather than
# acts.
{ config, lib, ... }:

let
  cfg = config.coderos.desktop;
in
{
  options.coderos.desktop.hands = {
    enable = lib.mkEnableOption ''
      hand tracking as desk input in the Coder compositor. On, the
      compositor's grant names `hands` among its launchers, so the session
      starts with hands driving the desk and Super+H turns them off and on
      again. Off, Super+H reaches the client. Hyprland gets no bind for it:
      the gestures land in the Coder compositor alone.

      The session names the sockets both sides use, in
      `os/bin/coder-compositor-session`: `CODEROS_CAMERA_DIR` and
      `CODEROS_HANDS_SOCKET`. It starts the compositor before the daemon,
      so the compositor asks the daemon to track until the daemon answers.
    '';

    judge = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether the compositor asks Jev to judge hand-gesture windows the
        deterministic rules cannot settle. Off, the rules run alone.

        On, the session sees `CODEROS_HANDS_JUDGE=shadow`, which is the
        first rung of the rollout: the compositor asks about a window
        whose transition the rules decided on a thin margin, or whose
        labels keep flipping, and records the answers beside what the
        rules did. It changes nothing the desk does. The seam acts only
        at the `act` rung, which waits on a measurement showing its
        answers match what the person did more often than the rules do.

        It reads the key from `TYPESAFE_API_KEY` or `~/.openagents/jev.json`,
        and a session with no key runs the rules alone and says so once.
        What leaves the machine is a one-second window of palm-relative hand
        features and pose labels, never an image and never a raw
        coordinate; `docs/os/hands-judge.md` says what a state carries.
        Turning this on is the host's decision that the provider may keep
        those windows.
      '';
    };
  };

  config = {
    # The `hands` launcher row: the compositor's grant names it, so the
    # session starts with hands driving the desk, and Super+H toggles them.
    coderos.desktop.launchers = lib.mkIf (cfg.enable && cfg.hands.enable) [ "hands" ];

    # The compositor reads the landmarks off the camera daemon's socket, so
    # hands without the daemon would read nothing. The seam reads the same
    # frames, so it needs the hands it judges.
    assertions = [
      {
        assertion = !cfg.hands.enable || (cfg.enable && cfg.camera.enable);
        message = "coderos.desktop.hands.enable reads the camera daemon's landmarks, so it needs coderos.desktop.camera.enable";
      }
      {
        assertion = !cfg.hands.judge || cfg.hands.enable;
        message = "coderos.desktop.hands.judge judges the windows the gesture rules cannot settle, so it needs coderos.desktop.hands.enable";
      }
    ];

    # The rung the session runs. `crates/coder-hands/src/watch.rs` reads
    # it, and an unset variable is the seam off.
    environment.sessionVariables = lib.mkIf (cfg.hands.enable && cfg.hands.judge) {
      CODEROS_HANDS_JUDGE = "shadow";
    };
  };
}
