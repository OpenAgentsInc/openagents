#!/usr/bin/env bash
# Part III focused gate. No full mutation runs or original CPU decisions.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
if [[ -n ${PSIONIC_LIVE_OUT:-} ]];then mkdir -p "$PSIONIC_LIVE_OUT";out=$(mktemp -d "$PSIONIC_LIVE_OUT/run.XXXXXX");else out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-live.XXXXXX");trap 'rm -rf "$out"' EXIT;fi
for mode in ${PSIONIC_LIVE_MODES:-debug release};do
    flags=(--run-steps 30000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O2;fi
    if [[ ${PSIONIC_LIVE_SKIP_COAX:-0} != 1 ]];then "$coax" test psionic/lib/serve-proto.coax "${flags[@]}";fi
    for kind in ${PSIONIC_LIVE_KINDS:-plain sanitized};do
        dir="$out/$mode-$kind";COAX="$coax" COAX_CC="$cc" psionic/platform/serve-build.sh "$dir" "$mode" "$kind"
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        common=(-std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$dir" -Ipsionic/platform)
        "$cc" "${common[@]}" psionic/tests/serve_busy.c psionic/platform/serve-busy.c "$dir/serve.o" -lm -o "$dir/busy-test"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/busy-test"
        "$cc" "${common[@]}" psionic/tests/serve_notify.c psionic/platform/serve-notify.c -o "$dir/notify-test"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/notify-test"
        "$cc" "${common[@]}" psionic/tests/verified_file.c psionic/platform/verified-file.c "$dir/serve.o" -lm -o "$dir/vf-test"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/vf-test"
        "$cc" "${common[@]}" psionic/tests/serve.c "$dir/serve.o" "$dir/prepare.o" "$dir/answer.o" -lm -o "$dir/serve-test"
        mkdir -p "$dir/logs"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/serve-test" "$dir/psionic-serve" "$dir/logs"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 python3 -I psionic/tests/service_live.py "$dir/psionic-serve" "$dir/operations"
        if [[ $mode == release && $kind == plain ]];then
            "$cc" "${common[@]}" psionic/tests/service_host_probe.c "$dir/serve.o" -lm -o "$dir/host-probe"
            "$dir/host-probe" > "$dir/host-probe.json"
        fi
    done
    for example in time-decision resident-decision live-service reuse-decision-prefix;do
        "$coax" run "psionic/examples/$example.coax" "${flags[@]}" > "$out/$example-$mode.out"
        cmp "psionic/fixtures/$example.out" "$out/$example-$mode.out"
    done
done
python3 -I psionic/tests/book.py
echo "Part III focused checks pass: $out; full mutation and original CPU decisions not run"
