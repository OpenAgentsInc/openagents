# Paper Mono, the one typeface CoderOS draws, as a font package.
#
# The files are the ones committed under `crates/paper-mono/fonts/` (SIL Open
# Font License 1.1), so the package fetches nothing. `os/` is the flake's
# root, and the font directory sits one level up, as the workspace does for
# `coder-desk.nix`. Only the four static weights are installed: fontconfig
# then lists one family with four styles, and no variable instance competes
# with them for a match.
{ lib, stdenvNoCC }:

stdenvNoCC.mkDerivation {
  pname = "paper-mono";
  version = "1.000";

  src = lib.fileset.toSource {
    root = ../../crates/paper-mono/fonts;
    fileset = lib.fileset.unions [
      ../../crates/paper-mono/fonts/OFL.txt
      ../../crates/paper-mono/fonts/PaperMono-Regular.ttf
      ../../crates/paper-mono/fonts/PaperMono-Medium.ttf
      ../../crates/paper-mono/fonts/PaperMono-SemiBold.ttf
      ../../crates/paper-mono/fonts/PaperMono-Bold.ttf
    ];
  };

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall
    install -Dm644 -t $out/share/fonts/truetype/paper-mono PaperMono-*.ttf
    install -Dm644 -t $out/share/doc/paper-mono OFL.txt
    runHook postInstall
  '';

  meta = {
    description = "Paper Mono, the typeface every OpenAgents surface uses";
    homepage = "https://github.com/paper-design/paper-mono";
    license = lib.licenses.ofl;
    platforms = lib.platforms.all;
  };
}
