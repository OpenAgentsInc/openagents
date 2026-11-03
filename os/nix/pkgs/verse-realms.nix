# The Verse's realms and the boot session, built for an image (docs/os/2026-11-02-verse-os.md, V7).
#
# `src` is what `verse/gpu/realm.sh local ship DIR` writes: each realm's Coax as C, its specs,
# packs and render assets, the network helper's and verse-session's C, and the shared Vulkan host.
# This builds it exactly as realm.sh builds it on a machine (no PIE, no FP contraction, musl's
# transcendental math ahead of glibc's so a realm computes what its wasm computes) and installs the
# tree to $out/share/verse-os, the directory verse-session runs (`--dir`). Nothing here is Coax: the
# Coax compiler already ran, on the machine that shipped `src`.
{ lib, stdenv, pkg-config, libdrm, vulkan-headers, vulkan-loader, wayland, wayland-scanner, wayland-protocols, openssl, musl, foot, src }:

stdenv.mkDerivation {
  pname = "verse-realms";
  version = "0";
  inherit src;

  nativeBuildInputs = [ pkg-config wayland-scanner ];
  buildInputs = [ libdrm vulkan-headers vulkan-loader wayland wayland-protocols openssl ];

  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    wp=$(pkg-config --variable=pkgdatadir wayland-protocols); mkdir -p wl
    for x in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1; do
      n=$(basename $x)
      wayland-scanner client-header $wp/$x.xml wl/$n-client-protocol.h
      wayland-scanner private-code $wp/$x.xml wl/$n-protocol.c
    done
    (cd net-src
     gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. net.c tool-native.c process-native.c store-file-native.c socket-native.c \
       tls-native.c entropy-native.c entropy-foreign.c entropy.c $(pkg-config --cflags --libs openssl) -lpthread -lm -o ../net
     gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-session.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
       entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-session
     gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-transcript.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
       entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-transcript
     # the desk's headless compositor (os/compositor.coax), which vk-realm --desk runs
     gcc -std=c11 -O2 -D_DEFAULT_SOURCE -w -I. $(pkg-config --cflags libdrm) ../desk/compositor.c tool-native.c process-native.c store-file-native.c read-file-native.c \
       socket-native.c entropy-native.c entropy-foreign.c entropy.c wayland-native.c screen-native.c $(pkg-config --libs libdrm) -lpthread -lm -o ../desk/compositor)
    # musl's math (verse/native/musl-math.sh), taken from its libc.a
    mkdir musl-math
    names=""
    for f in sin cos sincos tan asin acos atan atan2 sinh cosh tanh exp exp2 expm1 log log2 log10 log1p pow cbrt hypot; do names="$names $f.lo ''${f}f.lo"; done
    members=$(ar t ${musl}/lib/libc.a | grep -E '^(__sin|__cos|__tan|__sindf|__cosdf|__tandf|__rem_pio2|__rem_pio2f|__rem_pio2_large|__expo2|__expo2f|__math_[a-z]+|[a-z0-9]+_data|__log1pf?)\.lo$' || true)
    (cd musl-math && ar x ${musl}/lib/libc.a $(printf '%s\n' $names $members | sort -u | grep -Fx -f <(ar t ${musl}/lib/libc.a)))
    math=$(ls $PWD/musl-math/*.lo | tr '\n' ' ')
    for r in $(cat realms); do
      (cd $r
       flags="-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -w -I."
       read -r -a sources < sources
       units=(); link=()
       if [ -f RenderIdentity.h ]; then
         units=(render-host.c render-realm.c render-media.c render-textures.c RenderTexture.c)
         flags="$flags -ffunction-sections -fdata-sections"; link=(-Wl,--gc-sections)
       fi
       for s in "''${sources[@]}" "''${units[@]}" realm-host.c verse-panel.c verse-hands.c; do gcc $flags -c $s -o ''${s%.c}.o; done
       gcc $flags -c ../wl/xdg-shell-protocol.c -o wl-xdg-shell.o
       gcc $flags -c ../wl/xdg-decoration-unstable-v1-protocol.c -o wl-xdg-decoration.o
       gcc $flags "''${link[@]}" -Ikms -I../wl $(pkg-config --cflags libdrm wayland-client) vk-realm.c vk-spec.c vks.c vkc.c vkp.c vkd.c *.o $math \
         $(pkg-config --libs libdrm wayland-client) -lvulkan -lm -o vk-realm
       cp ../net .)
    done
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out/share/verse-os $out/bin
    for r in $(cat realms); do cp -a $r $out/share/verse-os/; rm -f $out/share/verse-os/$r/*.o; done
    cp net verse-session verse-transcript realms travel $out/share/verse-os/
    mkdir -p $out/share/verse-os/desk/launch
    cp desk/compositor desk/compositor-us.xkb $out/share/verse-os/desk/
    for n in start coder pane; do printf '#!/bin/sh\nexec ${foot}/bin/foot\n' > $out/share/verse-os/desk/launch/$n; chmod +x $out/share/verse-os/desk/launch/$n; done
    ln -s $out/share/verse-os/verse-session $out/bin/verse-session
    runHook postInstall
  '';

  meta = {
    description = "The Verse's realms on Vulkan and Verse OS's boot session";
    platforms = lib.platforms.linux;
  };
}
