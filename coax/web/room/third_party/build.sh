#!/usr/bin/env bash
# Firm reason: 4 (vendored C: ngtcp2, nghttp3 and picotls, the stated exception to "tools in Coax"
# for transport and crypto maturity, #13175; everything above aethyr/net's transport stays Coax)
# Builds the room's QUIC stack from the vendored sources in this directory into OUT/libquic.a, with
# the host's C compiler and nothing installed: ngtcp2 (QUIC, RFC 9000/9001/9002/9221), nghttp3
# (HTTP/3 and QPACK) and picotls with its minicrypto binding (TLS 1.3, P-256 and X25519, AES-GCM and
# ChaCha20-Poly1305), so the Mac builds it as the Linux hosts and the room's image do.
#   coax/web/room/third_party/build.sh OUT
# The archive is rebuilt only when a source here is newer than it. VERSIONS names each upstream.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
out="${1:?usage: build.sh OUT}"
mkdir -p "$out"
lib="$out/libquic.a"
if [[ -f "$lib" ]] && [[ -z "$(find "$here" -newer "$lib" \( -name '*.c' -o -name '*.h' -o -name build.sh \) -print -quit)" ]]; then exit 0; fi
cc="${CC:-cc}"
flags=(-std=c11 ${QUIC_CFLAGS:--O2} -w -fPIC -D_DEFAULT_SOURCE -DNGTCP2_STATICLIB -DNGHTTP3_STATICLIB
  -DHAVE_ARPA_INET_H=1 -DHAVE_NETINET_IN_H=1 -DHAVE_UNISTD_H=1 -DNGHTTP3_COAX_WEBTRANSPORT=1
  -I"$here/ngtcp2/lib/includes" -I"$here/ngtcp2/lib" -I"$here/ngtcp2/crypto/includes" -I"$here/ngtcp2/crypto"
  -I"$here/nghttp3/lib/includes" -I"$here/nghttp3/lib"
  -I"$here/picotls/include" -I"$here/picotls/deps/cifra/src" -I"$here/picotls/deps/cifra/src/ext" -I"$here/picotls/deps/micro-ecc")
[[ "$(uname)" != Darwin ]] || flags+=(-D_DARWIN_C_SOURCE)
srcs=("$here"/ngtcp2/lib/*.c "$here"/ngtcp2/crypto/*.c "$here"/nghttp3/lib/*.c "$here"/nghttp3/lib/sfparse/*.c
  "$here"/picotls/lib/*.c "$here"/picotls/lib/cifra/*.c "$here"/picotls/deps/micro-ecc/uECC.c)
for f in aes blockwise chacha20 chash curve25519 drbg hmac gcm gf128 modes poly1305 sha256 sha512; do srcs+=("$here/picotls/deps/cifra/src/$f.c"); done
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-quic.XXXXXX")"
trap 'rm -rf "$work"' EXIT
objs=(); k=0; running=0
jobs_max="${QUIC_JOBS:-4}"
for f in "${srcs[@]}"; do
  rel="${f#"$here/"}"; o="$work/$(printf '%s' "$rel" | tr '/' '_').o"
  "$cc" "${flags[@]}" -c "$f" -o "$o" &
  objs+=("$o"); running=$((running + 1)); k=$((k + 1))
  if (( running >= jobs_max )); then wait -n; running=$((running - 1)); fi
done
wait
for o in "${objs[@]}"; do [[ -s "$o" ]] || { echo "third_party/build.sh: $o was not built" >&2; exit 1; }; done
rm -f "$lib.new"
ar rcs "$lib.new" "${objs[@]}"
mv "$lib.new" "$lib"
echo "built $lib ($k sources)"
