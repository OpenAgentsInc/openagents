# What a host that runs the Coder compositor needs beyond the package.
#
# `desktop.nix` owns the session: which TTY starts the compositor, its grant,
# and the package and session script it runs. This file adds only what the
# compositor needs from the system, on a host that runs it on tty1
# (`coderos.desktop.compositor = "coder"`) or on the trial TTY
# (`coderos.desktop.trialTty`). A host that runs neither gets nothing here.
#
# The package, `os/pkgs/coder-compositor.nix`, links Smithay's libraries
# (`libxkbcommon`, `libudev`, `libinput`, `libseat`, and `libgbm`) and wraps
# the binary with the Wayland client library, EGL, and the Vulkan loader on
# `LD_LIBRARY_PATH`, so it needs no library from the host. Two things still
# come from the host:
#
# - `Xwayland` on `PATH`, which the compositor starts the first time it
#   starts a program. Without it X11 programs have no server.
# - For `coderos.desktop.compositorBinary`, a compositor built from a
#   checkout with rustup, the same libraries at link time and at run time.
#   The binary runs through `nix-ld`, which the base module turns on, so the
#   run-time set goes in its library path. `LIBRARY_PATH` and
#   `PKG_CONFIG_PATH` name a directory that holds these libraries alone, so
#   a build of anything else on the host links what it linked before.
{ config, lib, options, pkgs, ... }:

let
  cfg = config.coderos.desktop;
  runsCoderCompositor = cfg.compositor == "coder" || cfg.trialTty != null;
  fromCheckout = cfg.compositorBinary != null;

  # What the binary opens by name while it runs.
  runtime = with pkgs; [
    libxkbcommon
    wayland
    libglvnd
    vulkan-loader
    systemdLibs
    libinput
    seatd
    libgbm
  ];

  # What the linker and Smithay's build script read. `libinput` and `seatd`
  # have separate `dev` outputs, and naming a package alone takes its first
  # output: without `libinput.out` the join carried `libinput.pc` and never
  # `libinput.so`, so `pkg-config` answered while the link failed with
  # `unable to find library -linput`.
  linked = pkgs.symlinkJoin {
    name = "coderos-compositor-build-libraries";
    paths = with pkgs; [
      libxkbcommon
      libxkbcommon.dev
      systemdLibs
      systemdLibs.dev
      libinput.out
      libinput.dev
      seatd
      seatd.dev
      libgbm
    ];
  };
in
{
  config = lib.mkIf (cfg.enable && runsCoderCompositor) (lib.mkMerge [
    {
      programs.xwayland.enable = lib.mkDefault true;
    }

    (lib.mkIf fromCheckout {
      # A definition replaces the option's default list, so the default
      # set is named again here.
      programs.nix-ld.libraries = options.programs.nix-ld.libraries.default ++ runtime;

      environment.variables = {
        LIBRARY_PATH = "${linked}/lib";
        PKG_CONFIG_PATH = "${linked}/lib/pkgconfig";
      };
    })
  ]);
}
