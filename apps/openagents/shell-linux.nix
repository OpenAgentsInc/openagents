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
}
