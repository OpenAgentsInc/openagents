#!/usr/bin/env bash
# NIP-49's ncryptsec vector at log_n 16: the Coax scrypt with a 64 MiB
# workspace the C host allocates on the heap, then the AEAD open, checked
# against the published key. Checked and release builds, both under
# ASan/UBSan.   coax/tests/nostr/ncryptsec_vector.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
cc="${COAX_CC:-cc}"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-ncryptsec.XXXXXX")"
trap 'rm -rf "$work"' EXIT
leaks=1;if [[ $(uname -s) == Darwin ]];then leaks=0;fi # LSan is unavailable on Apple arm64.
export ASAN_OPTIONS=detect_leaks=$leaks:abort_on_error=1 UBSAN_OPTIONS=print_stacktrace=1:halt_on_error=1
for mode in checked release;do
  flags=();if [[ $mode == release ]];then flags=(--release);fi
  "$coax" c "$here/ncryptsec_vector.coax" --library --header "$work/ncryptsec_vector.h" "${flags[@]}" > "$work/lib.c"
  "$cc" -std=c11 -g -O1 -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-unused-variable \
    -fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer \
    -I"$work" "$work/lib.c" "$here/ncryptsec_vector.c" -o "$work/$mode" -lm
  start=$(date +%s)
  printf '%s: ' "$mode"; "$work/$mode"
  echo "$mode: $(( $(date +%s) - start )) s"
done
