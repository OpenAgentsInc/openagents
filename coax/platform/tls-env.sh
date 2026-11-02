# The TLS library for coax/platform/tls-native.c, sourced by build and test
# scripts: sets CXTLS_CFLAGS and CXTLS_LIBS, unless they are already set.
#   Linux: the first of
#          1. OPENSSL_DEV (headers) and OPENSSL_LIB (libssl/libcrypto), when set;
#          2. the system's OpenSSL 3 (not on NixOS): its headers
#             (include/openssl/ssl.h, version 3) and libssl.so/libcrypto.so,
#             from pkg-config or the standard multiarch library directories.
#             The binary then uses the system's own glibc and libssl, so it
#             starts on ordinary distributions (Ubuntu, Debian, Fedora);
#          3. OpenSSL 3 from nixpkgs, found the way coax/web/toolchain.sh finds
#             its Wasm toolchain (nix build --no-link --print-out-paths), so
#             nothing is installed system-wide. The binary gets an rpath to
#             that store path (and nix's glibc with it: use it on NixOS).
#          CXTLS_SYSROOT (default empty) is put before every system path, for
#          coax/tests/net/tls-env.coax; pkg-config is asked only without it.
#   macOS: Network.framework + Security, with dispatch blocks in the C host.
case "$(uname -s)" in
  Linux)
    if [[ -z "${CXTLS_CFLAGS:-}" || -z "${CXTLS_LIBS:-}" ]]; then
      cxtls_root="${CXTLS_SYSROOT:-}"
      if [[ -z "${OPENSSL_DEV:-}" && -z "${OPENSSL_LIB:-}" && ! -e "$cxtls_root/etc/NIXOS" ]]; then
        # The system's OpenSSL 3: headers and the link-time libssl.so/libcrypto.so.
        cxtls_inc=""; cxtls_libdir=""
        if [[ -z "$cxtls_root" ]] && command -v pkg-config >/dev/null 2>&1 && pkg-config --atleast-version=3 openssl 2>/dev/null; then
          cxtls_inc="$(pkg-config --variable=includedir openssl 2>/dev/null || true)"
          cxtls_libdir="$(pkg-config --variable=libdir openssl 2>/dev/null || true)"
        fi
        if [[ -z "$cxtls_inc" || ! -f "$cxtls_inc/openssl/ssl.h" ]]; then cxtls_inc="$cxtls_root/usr/include"; fi
        if [[ -z "$cxtls_libdir" || ! -e "$cxtls_libdir/libssl.so" ]]; then
          cxtls_libdir=""
          cxtls_arch="$( { "${CC:-gcc}" -print-multiarch 2>/dev/null || true; } | head -n 1)"
          for d in ${cxtls_arch:+"$cxtls_root/usr/lib/$cxtls_arch"} "$cxtls_root/usr/lib/$(uname -m)-linux-gnu" \
                   "$cxtls_root/usr/lib64" "$cxtls_root/usr/lib" "$cxtls_root/lib/$(uname -m)-linux-gnu" "$cxtls_root/lib64" "$cxtls_root/lib"; do
            if [[ -e "$d/libssl.so" && -e "$d/libcrypto.so" ]]; then cxtls_libdir="$d"; break; fi
          done
        fi
        if [[ -n "$cxtls_libdir" && -e "$cxtls_libdir/libcrypto.so" && -f "$cxtls_inc/openssl/ssl.h" ]] &&
           grep -Eqs '^#[[:space:]]*define[[:space:]]+OPENSSL_VERSION_MAJOR[[:space:]]+3([^0-9]|$)' "$cxtls_inc/openssl/opensslv.h"; then
          CXTLS_CFLAGS="${CXTLS_CFLAGS:--I$cxtls_inc}"
          CXTLS_LIBS="${CXTLS_LIBS:--L$cxtls_libdir -lssl -lcrypto}"
        fi
        unset cxtls_inc cxtls_libdir cxtls_arch d
      fi
      if [[ -z "${CXTLS_CFLAGS:-}" || -z "${CXTLS_LIBS:-}" ]]; then
        if [[ -z "${OPENSSL_DEV:-}" || -z "${OPENSSL_LIB:-}" ]]; then
          cxtls_paths="$(nix build --no-link --print-out-paths nixpkgs#openssl.dev nixpkgs#openssl.out)" || {
            echo "tls-env.sh: no system OpenSSL 3 (headers and libssl.so) and nix build nixpkgs#openssl failed; install libssl-dev, or set OPENSSL_DEV and OPENSSL_LIB" >&2; unset cxtls_root; return 1; }
          OPENSSL_DEV="${OPENSSL_DEV:-$(printf '%s\n' "$cxtls_paths" | grep -- '-dev$')}"
          OPENSSL_LIB="${OPENSSL_LIB:-$(printf '%s\n' "$cxtls_paths" | grep -v -- '-dev$')}"
          unset cxtls_paths
        fi
        [[ -f "$OPENSSL_DEV/include/openssl/ssl.h" && -e "$OPENSSL_LIB/lib/libssl.so" ]] || {
          echo "tls-env.sh: no OpenSSL headers in $OPENSSL_DEV or libssl in $OPENSSL_LIB" >&2; unset cxtls_root; return 1; }
        CXTLS_CFLAGS="${CXTLS_CFLAGS:--I$OPENSSL_DEV/include}"
        CXTLS_LIBS="${CXTLS_LIBS:--L$OPENSSL_LIB/lib -Wl,-rpath,$OPENSSL_LIB/lib -lssl -lcrypto}"
      fi
      unset cxtls_root
    fi
    ;;
  Darwin)
    CXTLS_CFLAGS="${CXTLS_CFLAGS:--fblocks}"
    CXTLS_LIBS="${CXTLS_LIBS:--framework Network -framework Security -framework CoreFoundation}"
    ;;
esac
export CXTLS_CFLAGS CXTLS_LIBS
