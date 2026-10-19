# Native development libraries for the Linux desktop and browser helper.
{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = with pkgs; [
    openssl.dev fontconfig.dev freetype.dev libxkbcommon.dev wayland.dev
    (pkgs.libxcb or pkgs.xorg.libxcb).dev
    (pkgs.libx11 or pkgs.xorg.libX11).dev
    alsa-lib.dev dbus.dev libglvnd.dev
    webkitgtk_4_1.dev json-glib.dev
  ];
  # GPUI opens the Wayland, xkbcommon, EGL, and Vulkan libraries at run time,
  # and NixOS has no global library path, so a launch from this shell finds
  # them here. /run/opengl-driver/lib holds the GPU driver.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
    libxkbcommon wayland libglvnd vulkan-loader
    (pkgs.libxcb or pkgs.xorg.libxcb) (pkgs.libx11 or pkgs.xorg.libX11)
  ]) + ":/run/opengl-driver/lib";
}
