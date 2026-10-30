# The TLS library for coax/platform/tls-native.c, sourced by build and test
# scripts: sets CXTLS_CFLAGS and CXTLS_LIBS, unless they are already set.
#   Linux: OpenSSL 3 from nixpkgs, found the way coax/web/toolchain.sh finds
#          its Wasm toolchain (nix build --no-link --print-out-paths), so
#          nothing is installed system-wide. Set OPENSSL_DEV (the .dev output,
#          headers) and OPENSSL_LIB (the out output, libssl/libcrypto) to
#          choose another build. The binary gets an rpath to that store path.
#   macOS: tls-native.c's Apple branch is a stub (CXTLS_UNSUPPORTED) and needs
#          no library yet.
case "$(uname -s)" in
  Linux)
    if [[ -z "${CXTLS_CFLAGS:-}" || -z "${CXTLS_LIBS:-}" ]]; then
      if [[ -z "${OPENSSL_DEV:-}" || -z "${OPENSSL_LIB:-}" ]]; then
        cxtls_paths="$(nix build --no-link --print-out-paths nixpkgs#openssl.dev nixpkgs#openssl.out)" || {
          echo "tls-env.sh: nix build nixpkgs#openssl failed; set OPENSSL_DEV and OPENSSL_LIB" >&2; return 1; }
        OPENSSL_DEV="${OPENSSL_DEV:-$(printf '%s\n' "$cxtls_paths" | grep -- '-dev$')}"
        OPENSSL_LIB="${OPENSSL_LIB:-$(printf '%s\n' "$cxtls_paths" | grep -v -- '-dev$')}"
        unset cxtls_paths
      fi
      [[ -f "$OPENSSL_DEV/include/openssl/ssl.h" && -e "$OPENSSL_LIB/lib/libssl.so" ]] || {
        echo "tls-env.sh: no OpenSSL headers in $OPENSSL_DEV or libssl in $OPENSSL_LIB" >&2; return 1; }
      CXTLS_CFLAGS="${CXTLS_CFLAGS:--I$OPENSSL_DEV/include}"
      CXTLS_LIBS="${CXTLS_LIBS:--L$OPENSSL_LIB/lib -Wl,-rpath,$OPENSSL_LIB/lib -lssl -lcrypto}"
    fi
    ;;
  Darwin)
    CXTLS_CFLAGS="${CXTLS_CFLAGS:-}"
    CXTLS_LIBS="${CXTLS_LIBS:-}"
    ;;
esac
export CXTLS_CFLAGS CXTLS_LIBS
