#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds the room's fuzzers with clang's libFuzzer, ASan and UBSan and
# runs them over the corpus here; the room's property is wire-fuzz.coax's, the harnesses are C)
# The room's wire and framing under coverage-guided fuzzing (#13283), on a Linux build host (Apple's
# clang has no libFuzzer): three fuzzers, each with its corpus in corpus/NAME/ (the seeds, and every
# input that ever crashed one, as regress-*):
#   room    wire-fuzz.coax's actions into the room library (fuzz-room.c): no trap, and no frame of one
#           seat changes another seat or socket
#   wt      the WebTransport layer, wt.c (fuzz-wt.c): packets, stream bytes, datagrams, the host's calls
#   frames  the socket framing: ws_parse and the upgrade, and the WebTransport stream's (fuzz-frames.c)
#   coax/remote/remote.sh run -- coax/web/room/fuzz/fuzz.sh slice 60      each fuzzer 60 s (after landing)
#   coax/remote/remote.sh run -- coax/web/room/fuzz/fuzz.sh long 24 24   each 24 hours on 24 workers (nightly)
#   coax/web/room/fuzz/fuzz.sh replay        every corpus input once: the regressions
#   coax/web/room/fuzz/fuzz.sh build [DIR]   the fuzzers into DIR (default coax/web/room/build/fuzz)
#   coax/web/room/fuzz/fuzz.sh run NAME SECONDS [WORKERS]   one fuzzer
# A crash, a hang (over 10 s), a leak or a property failure stops the run with status 1 and keeps the
# input in coax/web/room/build/fuzz/artifacts/; make it corpus/NAME/regress-WHAT and fix it (each is also a #test in
# wire-fuzz.coax when the room's). `long` and `run` merge the inputs that reach new code into the
# corpus here (commit them); `slice` and `replay` leave the corpus as it is.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"; root="$(cd "$here/../../../.." && pwd -P)"
room="$here/.."
out="$room/build/fuzz"
cc="${FUZZ_CC:-clang}"
coax="${COAX:-$root/coax/bin/coax}"
names=(room wt frames)
san=(-fsanitize=fuzzer-no-link,address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer -g -O1)
die() { echo "fuzz.sh: $*" >&2; exit 2; }

build() {   # dir
  local b="$1" T="$room/third_party"
  mkdir -p "$b"
  echo 'int LLVMFuzzerTestOneInput(const char *d, long n) { (void)d; (void)n; return 0; }' > "$b/probe.c"
  "$cc" -fsanitize=fuzzer "$b/probe.c" -o "$b/probe" 2> /dev/null || die "$cc has no libFuzzer (-fsanitize=fuzzer): run it on a Linux build host"
  # the room library, checked (every slice and overflow traps), with the room's own math
  "$coax" c "$here/wire-fuzz.coax" --library --header "$b/wire-fuzz.h" > "$b/wire-fuzz.c" || die "coax c wire-fuzz.coax failed"
  "$cc" -std=c11 -w "${san[@]}" -ffp-contract=off -c "$b/wire-fuzz.c" -o "$b/wire-fuzz.o" &
  "$cc" -std=c11 -w "${san[@]}" -ffp-contract=off -c "$room/room-math.c" -o "$b/room-math.o" &
  # the QUIC stack, instrumented for coverage and ASan (its own UB is upstream's)
  CC="$cc" QUIC_CFLAGS="-O1 -g -fsanitize=fuzzer-no-link,address" QUIC_JOBS=8 "$T/build.sh" "$b/quic" > /dev/null &
  wait
  [[ -s "$b/wire-fuzz.o" && -s "$b/room-math.o" && -s "$b/quic/libquic.a" ]] || die "a part did not build"
  local quic=(-D_DEFAULT_SOURCE -DNGTCP2_STATICLIB -DNGHTTP3_STATICLIB -I"$T/ngtcp2/lib/includes" -I"$T/ngtcp2/crypto/includes" -I"$T/nghttp3/lib/includes" -I"$T/picotls/include")
  "$cc" -std=c11 "${san[@]}" -fsanitize=fuzzer -I"$b" "$here/fuzz-room.c" "$b/wire-fuzz.o" "$b/room-math.o" -lm -o "$b/fuzz-room" &
  "$cc" -std=c11 "${san[@]}" -fsanitize=fuzzer "${quic[@]}" "$here/fuzz-wt.c" "$b/quic/libquic.a" -o "$b/fuzz-wt" &
  "$cc" -std=c11 "${san[@]}" -fsanitize=fuzzer "${quic[@]}" "$here/fuzz-frames.c" "$root/verse/voice-host/ws.c" "$room/wt.c" "$b/quic/libquic.a" -o "$b/fuzz-frames" &
  wait
  for n in "${names[@]}"; do [[ -x "$b/fuzz-$n" ]] || die "fuzz-$n did not build"; done
  [[ -s "$b/cert.pem" ]] || "$room/wt-cert.sh" "$b/cert.pem" "$b/key.pem" > /dev/null || die "no certificate"
  echo "fuzz.sh: built ${names[*]} in $b"
}
opts() {   # name: the run's flags
  local max=8192; [[ "$1" != wt ]] || max=16384
  echo "-timeout=10 -rss_limit_mb=4096 -max_len=$max -print_final_stats=1 -artifact_prefix=$out/artifacts/$1-"
}
fuzz_env() { WT_FUZZ_CERT="$out/cert.pem" WT_FUZZ_KEY="$out/key.pem" ASAN_OPTIONS="detect_leaks=1:allocator_may_return_null=1" UBSAN_OPTIONS="print_stacktrace=1:halt_on_error=1" "$@"; }
# One fuzzer for secs on workers processes, from a scratch copy of its corpus; merge: fold what is new
# into corpus/NAME.
run_one() {   # name secs workers merge
  local n="$1" secs="$2" w="${3:-1}" merge="${4:-0}" scratch="$out/new/$1" s
  mkdir -p "$scratch" "$out/artifacts" "$here/corpus/$n"
  local flags; read -r -a flags <<< "$(opts "$n")"
  echo "== fuzz $n: ${secs} s, $w worker(s)"
  if (( w > 1 )); then
    fuzz_env "$out/fuzz-$n" "${flags[@]}" -max_total_time="$secs" -fork="$w" -ignore_crashes=0 "$scratch" "$here/corpus/$n" > "$out/$n.log" 2>&1; s=$?
  else
    fuzz_env "$out/fuzz-$n" "${flags[@]}" -max_total_time="$secs" "$scratch" "$here/corpus/$n" > "$out/$n.log" 2>&1; s=$?
  fi
  grep -E 'stat::|^Done|cov: [0-9]+ ft' "$out/$n.log" | tail -4
  if (( s != 0 )); then
    echo "fuzz $n: FAILED (status $s); its input:"; ls "$out/artifacts" | grep "^$n-" | sed 's/^/  /'
    grep -E 'ERROR|SUMMARY|fuzz-[a-z]+:|runtime error|assertion' "$out/$n.log" | head -8 | sed 's/^/  /'
    return 1
  fi
  if (( merge )); then
    fuzz_env "$out/fuzz-$n" "${flags[@]}" -merge=1 "$here/corpus/$n" "$scratch" > "$out/$n.merge.log" 2>&1 || { echo "fuzz $n: the merge failed"; return 1; }
    echo "fuzz $n: corpus $(ls "$here/corpus/$n" | wc -l) inputs"
  fi
}
replay() {
  local n status=0 flags
  mkdir -p "$out/artifacts"
  for n in "${names[@]}"; do
    read -r -a flags <<< "$(opts "$n")"
    mapfile -t files < <(find "$here/corpus/$n" -type f | sort)
    (( ${#files[@]} > 0 )) || continue
    if fuzz_env "$out/fuzz-$n" "${flags[@]}" -runs=1 "${files[@]}" > "$out/$n.replay.log" 2>&1; then echo "replay $n: ${#files[@]} inputs, ok"
    else echo "replay $n: FAILED"; grep -E 'Running:|ERROR|SUMMARY|fuzz-[a-z]+:|runtime error' "$out/$n.replay.log" | tail -6 | sed 's/^/  /'; status=1; fi
  done
  return $status
}

mode="${1:-}"
case "$mode" in
  build) build "${2:-$out}" ;;
  replay) build "$out" > /dev/null && replay ;;
  slice)
    secs="${2:-60}"; status=0
    build "$out" > /dev/null || exit 2
    replay || status=1
    # the three side by side
    for n in "${names[@]}"; do run_one "$n" "$secs" 1 0 > "$out/$n.slice" 2>&1 & done
    wait
    for n in "${names[@]}"; do cat "$out/$n.slice"; ! grep -q FAILED "$out/$n.slice" || status=1; done
    exit $status ;;
  long)
    hours="${2:-24}"; w="${3:-$(nproc)}"; status=0
    build "$out" > /dev/null || exit 2
    replay || status=1
    # the three side by side, the workers shared out
    per=$(( w / 3 > 0 ? w / 3 : 1 ))
    for n in "${names[@]}"; do run_one "$n" "$(( hours * 3600 ))" "$per" 1 > "$out/$n.long" 2>&1 & done
    wait
    for n in "${names[@]}"; do cat "$out/$n.long"; ! grep -q FAILED "$out/$n.long" || status=1; done
    exit $status ;;
  run)
    [[ -n "${2:-}" && -n "${3:-}" ]] || die "run NAME SECONDS [WORKERS]"
    build "$out" > /dev/null || exit 2
    run_one "$2" "$3" "${4:-1}" 1 ;;
  *) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
