# The optional Android emulator, and the file that describes it.
#
# A host that leaves `coderos.desktop.android.enable` off gets nothing from
# this file: no SDK, no system image, no bind, and no Xwayland.
#
# With it on, the host installs the Android SDK that nixpkgs composes from
# Google's repository: the emulator, `adb`, `avdmanager`, one platform, and
# one system image, each pinned by the nixpkgs revision the flake pins. The
# launcher and every flag it passes are `os/bin/android-emulator`, and
# SUPER + A runs it on the desktop session. The guest runs on KVM; run
# `emulator -accel-check` to confirm a host's CPU and kernel provide it.
#
# The system image is the closure's largest piece, at 3.5 GB for API 35, and
# it is in the store rather than under a home directory because it is what
# the host runs, the same as a kernel. The virtual device the emulator boots
# from it is data: it holds what the guest writes, so it lives under
# `~/.openagents/android/avd` and is made on the first start.
#
# The emulator is an X11 program. Its Qt ships the xcb platform plugin and
# no Wayland one (`lib64/qt/plugins/platforms` in the emulator package holds
# xcb, offscreen, linuxfb, vnc, and minimal), so this file turns Xwayland on
# for the session, which the desktop leaves off for a session that has no
# X11 client. That is the one thing here that reaches past the emulator.
#
# The host writes what it granted to /etc/coderos/android.json: the SDK, the
# image, the device, the port, and the launcher. The launcher reads it so the
# script and this file agree on every name, and a tool that drives the guest
# over `adb` from Coder would read the same file to learn whether this host
# has one.
{ config, lib, options, pkgs, ... }:

let
  desktop = config.coderos.desktop;
  cfg = desktop.android;

  # The SDK, composed from Google's repository by nixpkgs. Only what the
  # emulator needs: the legacy `tools`, the build tools, and cmake are left
  # out, because nothing here compiles an app. `licenseAccepted` is the
  # androidenv's own switch for the SDK license, set here rather than through
  # `nixpkgs.config` so a host that leaves the option off accepts nothing.
  sdk = ((pkgs.androidenv.override { licenseAccepted = true; }).composeAndroidPackages {
    includeEmulator = true;
    includeSystemImages = true;
    platformVersions = [ cfg.platformVersion ];
    systemImageTypes = [ cfg.systemImageType ];
    abiVersions = [ cfg.abi ];
    buildToolsVersions = [ ];
    includeCmake = false;
    toolsVersion = null;
  }).androidsdk;

  sdkRoot = "${sdk}/libexec/android-sdk";

  # The package path `avdmanager` and the emulator know the image by.
  image = "system-images;android-${cfg.platformVersion};${cfg.systemImageType};${cfg.abi}";

  # The desk command, when the host has the module that declares it. The
  # launcher asks it whether the emulator's window is open; without it, the
  # launcher always starts the emulator.
  desk = lib.optional (options.coderos.desktop ? deskPackage) desktop.deskPackage;

  # The launcher is a real file under os/bin, wrapped so that shellcheck runs
  # at build time and its tools are on PATH by name.
  launcher = pkgs.writeShellApplication {
    name = "android-emulator";
    runtimeInputs = [ sdk pkgs.jq pkgs.coreutils ] ++ desk;
    text = builtins.readFile ../../bin/android-emulator;
  };

  grant = {
    grant = "coderos-android-v1";
    user = desktop.user;
    sdk = sdkRoot;
    inherit image;
    avd = cfg.avd;
    device = cfg.device;
    gpu = cfg.gpu;
    port = cfg.port;
    class = cfg.windowClass;
    # What SUPER + A runs.
    emulator = "${launcher}/bin/android-emulator";
  };
in
{
  options.coderos.desktop.android = {
    enable = lib.mkEnableOption ''
      the Android emulator: the SDK's emulator on KVM, on SUPER + A, with one
      system image in the store. It needs the desktop session, and it turns
      Xwayland on for it
    '';

    platformVersion = lib.mkOption {
      type = lib.types.str;
      default = "35";
      description = ''
        The Android API level of the platform and the system image. 35 is
        Android 15, which is what `bins/coder-android` and
        `bins/openagents-android` compile against.
      '';
    };

    systemImageType = lib.mkOption {
      type = lib.types.str;
      default = "google_apis";
      description = ''
        The image's tag. `google_apis` carries Google Play services without
        the Play Store, and its `adb` runs as root, which is what a developer
        wants on a desk. `google_apis_playstore` carries the store and locks
        `adb` out of root; `default` carries neither.
      '';
    };

    abi = lib.mkOption {
      type = lib.types.str;
      default = "x86_64";
      description = ''
        The guest's architecture. On an x86_64 host, only an x86_64 image
        runs on KVM; an `arm64-v8a` image would be emulated one instruction
        at a time. Google's x86_64 images from API 30 on run an app's
        `arm64-v8a` libraries through their own translation layer, so the
        app's ABI does not have to match this.
      '';
    };

    device = lib.mkOption {
      type = lib.types.str;
      default = "pixel_7";
      description = ''
        The hardware profile `avdmanager create avd --device` takes, from
        `avdmanager list device`. It sets the screen, its density, and the
        memory. This is read when the virtual device is made, on the first
        start, and a change after that reaches a device that is deleted and
        made again.
      '';
    };

    avd = lib.mkOption {
      type = lib.types.str;
      default = "coder";
      description = ''
        The virtual device's name. Its files live at
        `~/.openagents/android/avd/<name>.avd`.
      '';
    };

    gpu = lib.mkOption {
      type = lib.types.enum [ "host" "swiftshader_indirect" "angle_indirect" "guest" "off" ];
      default = "host";
      description = ''
        What draws the guest's screen, the value the emulator's `-gpu` flag
        takes. `host` sends the guest's OpenGL to this machine's GPU through
        Xwayland; `swiftshader_indirect` rasterizes on the CPU, which is what
        to fall back to when the host driver misbehaves.
      '';
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 5554;
      description = ''
        The emulator's console port. `adb` uses the next one up, and names
        the guest `emulator-<port>`. The emulator accepts an even port from
        5554 to 5584. Both listen on loopback only.
      '';
    };

    windowClass = lib.mkOption {
      type = lib.types.str;
      default = "Emulator";
      description = ''
        The class Hyprland reports for the emulator's windows. The QEMU
        process that draws them sets `WM_CLASS` to the pair
        `qemu-system-x86_64`, `Emulator`, and Hyprland 0.55 takes the second
        string of the pair as the class (`handleWMClass` in
        `src/xwayland/XWM.cpp`). The launcher matches this to focus a window
        that is open rather than open a second, and the desktop matches it
        to float the windows.
      '';
    };
  };

  config = lib.mkIf (desktop.enable && cfg.enable) {
    # The description. The launcher reads it, and a tool that drives the
    # guest would read it to learn whether this host has one.
    environment.etc."coderos/android.json".text = builtins.toJSON grant;

    # The launcher, and the SDK on the PATH as well, so a person at the
    # machine can reach the guest with the same `adb` the launcher stops it
    # with, and `scripts/build-coder-android.sh` finds the SDK by
    # `ANDROID_HOME`.
    environment.systemPackages = [ launcher sdk ];
    environment.sessionVariables = {
      ANDROID_HOME = sdkRoot;
      ANDROID_SDK_ROOT = sdkRoot;
      ANDROID_AVD_HOME = "${config.users.users.${desktop.user}.home}/.openagents/android/avd";
    };

    # The emulator opens /dev/kvm. logind grants the active seat an ACL on
    # it, so a person at the machine already can; the group is for a start
    # from anywhere else.
    users.users.${desktop.user}.extraGroups = [ "kvm" ];

    # The window is an X11 one, so the session needs Xwayland. The desktop
    # sets this to false with `mkDefault`, and this plain assignment wins.
    programs.hyprland.xwayland.enable = true;

    # A compositor other than Hyprland starts the `Xwayland` binary it finds
    # on `PATH`, which `programs.xwayland` installs. Hyprland's option turns
    # that on as well; this names it for any other session.
    programs.xwayland.enable = true;

    assertions = [
      {
        assertion = lib.mod cfg.port 2 == 0 && cfg.port >= 5554 && cfg.port <= 5584;
        message = ''
          coderos.desktop.android.port is ${toString cfg.port}. The emulator
          takes an even console port from 5554 to 5584 and uses the next one
          for adb.
        '';
      }
    ];
  };
}
