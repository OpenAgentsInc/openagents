# Screen recording and dictation for a CoderOS desktop session.
#
# Both are Wayland session capabilities, so both hang off `coderos.desktop`
# and neither exists without it. Their tools reach the compositor through
# the session's own socket, so they work from a program running in that
# session and not from an SSH login. SUPER + V toggles dictation.
#
# Why this file exists at all: these capabilities first appeared on one host
# as `nix profile install` and two hand-written scripts in `~/.local/bin`.
# That works until the profile is garbage-collected, the account is
# recreated, or a second host wants the same thing, and it leaves the
# machine holding capability the repository cannot describe.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.desktop;

  # The scripts are real files under os/bin so they can be read, reviewed,
  # and linted as shell. writeShellApplication supplies the shebang, sets
  # `set -euo pipefail`, runs shellcheck at build time, and — the point —
  # puts the runtime tools on PATH by name, so the script does not depend on
  # whatever a person's profile happens to hold.
  # `beep-sound` is not named here: it is the dictation capability's chime,
  # reached through the inherited PATH behind a `command -v` guard, so a host
  # without dictation still records and warns on stderr alone.
  screenRecord = pkgs.writeShellApplication {
    name = "screen-record";
    runtimeInputs = [
      pkgs.wf-recorder
      pkgs.slurp
      pkgs.ffmpeg
      pkgs.procps
      pkgs.pulseaudio
      pkgs.pipewire
      pkgs.jq
      pkgs.coreutils
    ];
    text = builtins.readFile ../../bin/screen-record;
  };

  # The chime that says recording started and stopped. A person pressing a
  # key with no window to look at needs to hear that something happened.
  beep = pkgs.writeShellApplication {
    name = "beep-sound";
    runtimeInputs = [ pkgs.pipewire pkgs.ffmpeg ];
    text = builtins.readFile ../../bin/beep-sound;
  };

  # `libnotify` is what raises the banner that says the microphone is open:
  # the tone alone failed silently on 2026-09-11 and a capture ran six and a
  # half minutes. The notification daemon the desktop session starts draws
  # it. `gnused` is named rather than inherited, because the transcription
  # trims its text with it.
  dictate = pkgs.writeShellApplication {
    name = "dictate-toggle";
    runtimeInputs = [
      pkgs.whisper-cpp
      pkgs.wtype
      pkgs.wl-clipboard
      pkgs.pipewire
      pkgs.ffmpeg
      pkgs.curl
      pkgs.coreutils
      pkgs.gnused
      pkgs.libnotify
    ];
    text = builtins.readFile ../../bin/dictate-toggle;
  };
in
{
  options.coderos.desktop.screenRecording = {
    enable = lib.mkEnableOption "screen recording in the desktop session";

    framerate = lib.mkOption {
      type = lib.types.ints.positive;
      default = 30;
      description = ''
        The constant frame rate `screen-record` holds the picture to. Pick
        one the compositor delivers under the heaviest thing you record;
        wf-recorder writes the display's rate otherwise, as a variable-rate
        stream that editors read as constant and play fast. The session sees
        it as `CODEROS_RECORD_FPS`.
      '';
    };
  };

  # Which microphone recordings and dictation use. Dictation reads PipeWire's
  # default source, and with nothing said here PipeWire picks among the inputs
  # it finds by a priority every USB device shares, so a webcam's built-in
  # microphone can win over the microphone on the desk. It did on
  # 2026-09-07: a recording made "with the mic" was the webcam's, faint and
  # pumping. Naming the microphone here outranks
  # every other input while it is plugged in, and `screen-record` reads it by
  # name rather than trusting the default, and says out loud when it has to
  # read something else or when the named microphone delivers no audio.
  # `exclude` goes further for an input that is never wanted: its capture node
  # is not created, so it can neither become the default nor be recorded.
  options.coderos.desktop.microphone = {
    node = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "alsa_input.usb-Example_Microphone.*";
      description = ''
        A regular expression over PipeWire node names (`wpctl inspect`, the
        `node.name` line). The first input it matches is the default source,
        and the one `screen-record --audio` records. The session sees it as
        `CODEROS_MICROPHONE_NODE`.
      '';
    };
    name = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "Desk mic";
      description = ''
        What to call the named microphone in a warning and on the recording
        HUD, which reads `NOT DESK MIC` when a recording is reading another
        input.
        The session sees it as `CODEROS_MICROPHONE_NAME`.
      '';
    };
    exclude = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "alsa_input.usb-Example_Webcam.*" ];
      description = ''
        Regular expressions over PipeWire node names whose capture nodes are
        never created. An input listed here cannot become the default source
        and cannot be recorded, so a host with the named microphone absent
        has no microphone rather than the wrong one.
      '';
    };
    usbVendor = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "1234";
      description = "The microphone's USB idVendor, to pin its port's autosuspend off. Empty disables the udev rule.";
    };
    usbProduct = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "5678";
      description = "The microphone's USB idProduct, paired with usbVendor.";
    };
    gain = lib.mkOption {
      type = lib.types.nullOr (lib.types.submodule {
        options = {
          card = lib.mkOption {
            type = lib.types.str;
            example = "Microphone";
            description = "The ALSA card id (`/proc/asound/cards`, the bracketed name).";
          };
          control = lib.mkOption {
            type = lib.types.str;
            default = "Mic Capture Volume";
            description = "The mixer control that sets the hardware capture gain.";
          };
          value = lib.mkOption {
            type = lib.types.int;
            example = 10;
            description = ''
              The control's raw value. A USB microphone remembers nothing
              across a power cycle and the kernel exposes its gain at
              whatever it woke with; on 2026-09-07 one woke at its maximum
              (+21 dB) and clipped. This is set at boot and every
              time the card appears.
            '';
          };
        };
      });
      default = null;
      description = "Hardware capture gain to set whenever the microphone's card appears.";
    };
  };
  options.coderos.desktop.dictation = {
    enable = lib.mkEnableOption "push-to-talk dictation in the desktop session";

    model = lib.mkOption {
      type = lib.types.str;
      default = "$HOME/.cache/whisper.cpp/ggml-base.en.bin";
      description = ''
        Where the speech model rests. Weights are data rather than
        configuration: they stay out of the repository and out of the system
        closure.
        The script fetches them once to this path when they are absent.
      '';
    };

    limitSeconds = lib.mkOption {
      type = lib.types.ints.positive;
      default = 180;
      description = ''
        How long one capture may run. The recorder is given this as its own
        duration, so the microphone closes when it runs out whether or not
        anyone presses the key again, and the capture is kept rather than
        discarded. A recording past a few minutes is more likely a toggle
        nobody saw than a person still talking: on 2026-09-11 one ran for six
        minutes and thirty seconds after two presses that showed nothing. The session sees it as
        `CODEROS_DICTATION_LIMIT`.
      '';
    };
  };

  config = lib.mkMerge [
    (lib.mkIf (cfg.enable && cfg.screenRecording.enable) {
      environment.systemPackages = [ screenRecord ];
      environment.sessionVariables.CODEROS_RECORD_FPS = toString cfg.screenRecording.framerate;
    })

    (lib.mkIf (cfg.enable && cfg.microphone.node != null) {
      # The scripts read the pattern and the name from the session, so the
      # recorder targets the same node this rule ranks first.
      environment.sessionVariables.CODEROS_MICROPHONE_NODE = cfg.microphone.node;
    })
    (lib.mkIf (cfg.enable && cfg.microphone.name != null) {
      environment.sessionVariables.CODEROS_MICROPHONE_NAME = cfg.microphone.name;
    })
    (lib.mkIf (cfg.enable && cfg.microphone.exclude != [ ]) {
      # An excluded input has no capture node at all. WirePlumber's default
      # selection remembers every input a person ever chose and ranks a
      # remembered one above any priority, so demoting the webcam's
      # microphone would still hand it the default the moment the named
      # microphone left. On one host the state file listed the webcam as a
      # past default. A node that is never created cannot be remembered,
      # selected, or recorded.
      services.pipewire.wireplumber.extraConfig."52-coderos-microphone-exclude" = {
        "monitor.alsa.rules" = map (pattern: {
          matches = [ { "node.name" = "~${pattern}"; } ];
          actions.update-props = {
            "node.disabled" = true;
          };
        }) cfg.microphone.exclude;
      };
    })
    (lib.mkIf (cfg.enable && cfg.microphone.node != null) {
      # WirePlumber ranks inputs by priority.session; every USB input arrives
      # at the same 2109, and the tie goes to whichever appeared first. A
      # named microphone gets a priority nothing else has.
      #
      # It also never suspends. By default WirePlumber closes an idle capture
      # node after a few seconds; this microphone's firmware comes back from
      # that ALSA suspend/resume wedged, delivering silent frames until it is
      # physically unplugged. `session.suspend-timeout-seconds = 0` keeps its
      # stream open for the life of the session so the resume never happens,
      # and `node.pause-on-idle = false` stops the graph pausing it between
      # recordings. The device stays live; no replug is ever needed.
      services.pipewire.wireplumber.extraConfig."51-coderos-microphone" = {
        "monitor.alsa.rules" = [
          {
            matches = [ { "node.name" = "~${cfg.microphone.node}"; } ];
            actions.update-props = {
              "priority.session" = 3000;
              "session.suspend-timeout-seconds" = 0;
              "node.pause-on-idle" = false;
            };
          }
        ];
      };
    })
    (lib.mkIf (cfg.enable && cfg.microphone.gain != null) {
      # udev tags the card's appearance; the service sets the gain once the
      # control exists. `amixer` addresses the card by id, so the number the
      # kernel hands out does not matter.
      services.udev.extraRules = ''
        SUBSYSTEM=="sound", ACTION=="add", KERNEL=="card*", ATTR{id}=="${cfg.microphone.gain.card}", TAG+="systemd", ENV{SYSTEMD_WANTS}+="coderos-microphone-gain.service"
      ${lib.optionalString (cfg.microphone.usbVendor != "") ''
        # Keep the microphone's USB port from autosuspending. A replug or a
        # reboot brings it back at the kernel default of "auto"; a suspended
        # USB audio device is one of the ways this microphone wedges. Pin it on.
        SUBSYSTEM=="usb", ATTR{idVendor}=="${cfg.microphone.usbVendor}", ATTR{idProduct}=="${cfg.microphone.usbProduct}", TEST=="power/control", ATTR{power/control}="on"
      ''}'';
      systemd.services.coderos-microphone-gain = {
        description = "Set the microphone's hardware capture gain";
        serviceConfig = {
          Type = "oneshot";
          ExecStart = ''
            ${pkgs.alsa-utils}/bin/amixer -c ${cfg.microphone.gain.card} cset name='${cfg.microphone.gain.control}' ${toString cfg.microphone.gain.value}
          '';
        };
        # Also at boot, for a card that was present before udev had a rule.
        wantedBy = [ "sound.target" ];
        after = [ "sound.target" ];
      };
    })
    (lib.mkIf (cfg.enable && cfg.dictation.enable) {
      environment.systemPackages = [ dictate beep ];
      coderos.desktop.launchers = [ "dictation" ];

      # The microphone reaches the script through PipeWire, which the desktop
      # already runs for audio. `pw-record` reads the default source, so a
      # USB microphone works by being the default rather than by being named
      # here.
      services.pipewire = {
        enable = lib.mkDefault true;
        pulse.enable = lib.mkDefault true;
      };

      environment.sessionVariables.CODEROS_WHISPER_MODEL = cfg.dictation.model;
      environment.sessionVariables.CODEROS_DICTATION_LIMIT = toString cfg.dictation.limitSeconds;
    })
  ];
}
