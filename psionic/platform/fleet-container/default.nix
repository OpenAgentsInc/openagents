# Pinned build/runtime inputs for the Coax-emitted fleet. Product protocol and
# policy remain in the unchanged emitted C; Nix supplies compiler/TLS/closure.
let
  pin = builtins.fromJSON (builtins.readFile ./nixpkgs-pin.json);
  nixpkgs = builtins.fetchTarball { inherit (pin) url sha256; };
  pkgs = import nixpkgs { system = "x86_64-linux"; };
  fleet = pkgs.stdenv.mkDerivation {
    pname = "psionic-fleet";
    version = "m0-nix-20261031";
    src = ./.;
    dontConfigure = true;
    # Keep explicit -g compilation but strip installed debug sections before
    # closure scanning, so development headers/compiler are not runtime roots.
    nativeBuildInputs = [ pkgs.binutils pkgs.patchelf ];
    buildInputs = [ pkgs.openssl ];
    buildPhase = ''
      runHook preBuild
      sha256sum -c SOURCES.sha256
      cc -std=c11 -O1 -g -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -Dmain=coax_fleet_main -c fleet.c -o fleet.o
      cc -std=c11 -O1 -g -I. -Iplatform -I${pkgs.openssl.dev}/include -L${pkgs.openssl.out}/lib -Wl,-rpath,${pkgs.openssl.out}/lib fleet-host.c platform/socket-native.c platform/tls-native.c platform/entropy-native.c platform/entropy-foreign.c platform/process-native.c fleet.o -lssl -lcrypto -lm -lpthread -o psionic-fleet
      runHook postBuild
    '';
    installPhase = ''
      mkdir -p $out/bin $out/share/psionic-fleet
      cp psionic-fleet $out/bin/
      cp SOURCE_COMMIT COAX_SHA256 SOURCES.sha256 nixpkgs-pin.json $out/share/psionic-fleet/
      printf 'nixpkgs_revision=%s\nopenssl_version=%s\nopenssl_runtime=%s\ncompiler_version=%s\n' '${pin.rev}' '${pkgs.openssl.version}' '${pkgs.openssl.out}' '${pkgs.stdenv.cc.version}' > $out/share/psionic-fleet/NIX_TLS_PROVENANCE
    '';
    postFixup = ''
      ${pkgs.patchelf}/bin/patchelf --print-interpreter $out/bin/psionic-fleet > $out/share/psionic-fleet/ELF_INTERPRETER
      ${pkgs.binutils}/bin/readelf -d $out/bin/psionic-fleet > $out/share/psionic-fleet/ELF_DYNAMIC
      ${pkgs.glibc.bin}/bin/ldd $out/bin/psionic-fleet | sed -E 's/ \(0x[0-9a-f]+\)//g' > $out/share/psionic-fleet/ELF_LDD
      grep -F '${pkgs.openssl.out}/lib/libssl.so' $out/share/psionic-fleet/ELF_LDD
      grep -F '${pkgs.openssl.out}/lib/libcrypto.so' $out/share/psionic-fleet/ELF_LDD
    '';
  };
  runtimeRoot = pkgs.runCommand "psionic-fleet-runtime" {} ''
    mkdir -p $out/bin $out/usr/local/bin $out/etc/ssl/certs $out/usr/share/psionic-fleet $out/tmp
    ln -s ${fleet}/bin/psionic-fleet $out/usr/local/bin/psionic-fleet
    cp ${./start.sh} $out/usr/local/bin/start-fleet
    ln -s ${pkgs.dash}/bin/dash $out/bin/sh
    ln -s ${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt $out/etc/ssl/certs/ca-certificates.crt
    cp ${fleet}/share/psionic-fleet/* $out/usr/share/psionic-fleet/
    printf 'root:x:0:0:root:/root:/bin/sh\nfleet:x:10001:10001:fleet:/nonexistent:/bin/sh\n' > $out/etc/passwd
    printf 'root:x:0:\nfleet:x:10001:\n' > $out/etc/group
    printf 'hosts: files dns\n' > $out/etc/nsswitch.conf
    printf '127.0.0.1 localhost\n::1 localhost\n' > $out/etc/hosts
    touch $out/etc/resolv.conf
  '';
  closure = pkgs.closureInfo { rootPaths = [ runtimeRoot ]; };
  imageRoot = pkgs.runCommand "psionic-fleet-image-root" {} ''
    mkdir -p $out/usr/share/psionic-fleet
    cp ${closure}/store-paths $out/usr/share/psionic-fleet/RUNTIME_CLOSURE
    cp -a ${runtimeRoot}/. $out/
    chmod u+w $out
    tar -cpf runtime-store.tar --files-from ${closure}/store-paths
    tar -xpf runtime-store.tar --directory $out --no-same-owner --delay-directory-restore
  '';
in { inherit fleet runtimeRoot imageRoot; nativeTools = pkgs.coreutils; }
