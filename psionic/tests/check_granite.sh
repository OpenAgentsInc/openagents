#!/usr/bin/env bash
# Granite 4.2 8B (issue #12870): binding, tokenizer and chat template on the
# CPU; the dense decoder on Metal against its CPU repeat; parity with the
# packaged llama.cpp (LLAMA_PREFIX, default /opt/homebrew) unless
# PSIONIC_GRANITE_PARITY=0. The pinned files live
# in PSIONIC_GRANITE_DIR (default ~/models/psionic/granite-4.2-8b-a7042144125c);
# without them only the owned tiny model runs. Nothing is downloaded.
#   psionic/tests/check_granite.sh [--mutate] [--jobs N]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)echo "usage: check_granite.sh [--mutate] [--jobs N]" >&2;exit 2;;esac;done
model=${PSIONIC_GRANITE_DIR:-$HOME/models/psionic/granite-4.2-8b-a7042144125c}
ggufs=()
if [[ -f $model/SHA256SUMS ]];then for q in Q4_K_M Q8_0;do f=$model/gguf-93f3f6a8938e/granite-4.2-8b-$q.gguf;[[ -f $f ]] && ggufs+=("$f");done;fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-granite.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/granite_fixture.py --coax | cmp - psionic/fixtures/granite/tiny.coax
python3 -I psionic/tests/granite_unicode.py | cmp - psionic/fixtures/granite/unicode.coax || echo "note: this Python's Unicode tables differ from the recorded ones"
python3 -I psionic/tests/granite_text.py --cases "$out/cases.bin"
# Steps 1-3: binding, tokenizer and chat, in Coax and natively.
for unit in model tokenizer chat;do for mode in debug release;do
  flags=(--run-steps 200000000);[[ $mode == release ]] && flags+=(--release)
  "$coax" test psionic/lib/granite/$unit.coax "${flags[@]}" | tail -1
done;done
for mode in debug release;do
 flags=(--run-steps 200000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O2;fi
 "$coax" c psionic/lib/granite/chat.coax --library --header "$out/chat.h" "${flags[@]}" > "$out/chat-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
    -I"$out" -Ipsionic/platform -DGRANITE_COAX_HEADER='"chat.h"' "$out/chat-$mode.c" psionic/platform/granite-file.c psionic/tests/granite_text.c -lm -o "$out/text-$mode-$kind"
  for f in "${ggufs[@]}";do
   ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/text-$mode-$kind" "$f" "$out/cases.bin" > "$out/text.out"
   python3 -I psionic/tests/granite_text.py --check "$out/text.out" "$model" "$f" | sed "s|^|$(basename "$f" .gguf) $mode $kind: |"
  done
 done
done
# Step 4: the kernels against binary64, then the decoder on Metal against
# its CPU repeat (the owned tiny model, then the pinned files).
for unit in psionic/tests/granite-kernel-checks.coax psionic/lib/granite/host.coax;do for mode in debug release;do
  flags=(--run-steps 200000000);[[ $mode == release ]] && flags+=(--release)
  "$coax" test $unit "${flags[@]}" | tail -1
done;done
"$coax" metal-compute psionic/lib/granite/kernels.coax --release > "$out/k.metal"
"$coax" c psionic/lib/granite/host.coax --library --release --header "$out/host.h" > "$out/host.c"
F=(-std=c11 -O2 -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-missing-field-initializers -I"$out" -Ipsionic/platform '-DGRANITE_COAX_HEADER="host.h"')
"$cc" "${F[@]}" -c "$out/host.c" -o "$out/host.o"
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -c psionic/platform/metal-host.m -o "$out/metal-host.o"
"$cc" "${F[@]}" -fblocks psionic/platform/granite-file.c psionic/platform/granite-metal.c psionic/tests/granite_run.c "$out/host.o" "$out/metal-host.o" -framework Metal -framework Foundation -lm -o "$out/granite-run"
python3 -I psionic/tests/granite_fixture.py --gguf "$out/tiny.gguf"
"$out/granite-run" "$out/tiny.gguf" "$out/k.metal" compare --raw --prompt "hello world hello" --steps 8 --capacity 512 | tail -1
# Past 8 tokens the prompt takes the prefill products and attention; compare
# also holds it within 1e-3 of the decoder fed a token at a time.
"$out/granite-run" "$out/tiny.gguf" "$out/k.metal" compare --raw --prompt "hello world hello hello world hello world hello world hello" --steps 8 --capacity 512 | tail -2
GRANITE_CHUNK=13 "$out/granite-run" "$out/tiny.gguf" "$out/k.metal" compare --raw --prompt "hello world hello hello world hello world hello world hello" --steps 8 --capacity 512 | tail -1 | sed "s|^|chunks of 13: |"
for f in "${ggufs[@]}";do
 q=$(basename "$f" .gguf)
 "$out/granite-run" "$f" "$out/k.metal" compare --prompt "Move the oak tree" --steps 2 --capacity 512 | tail -2 | sed "s|^|$q: |"
 "$out/granite-run" "$f" "$out/k.metal" generate --prompt "What is the capital of France? Answer in one sentence." --steps 24 --capacity 1024 > "$out/answer.txt"
 grep -q "Paris" "$out/answer.txt" || { echo "$q: the answer does not name Paris" >&2; cat "$out/answer.txt" >&2; exit 1; }
 tail -2 "$out/answer.txt" | sed "s|^|$q: |"
 # Tool calling: the model calls get_weather in the template's format, the
 # result goes back as a tool turn, and the answer uses it.
 "$out/granite-run" "$f" "$out/k.metal" tools --tools psionic/fixtures/granite/tools.jsonl --prompt "What's the weather in Paris right now?" --steps 64 --capacity 1024 --result '{"sky": "light rain", "temp_c": 11}' > "$out/tools.txt"
 grep -qx "call 0: get_weather(city=Paris)" "$out/tools.txt" || { echo "$q: no get_weather(city=Paris) call" >&2; cat "$out/tools.txt" >&2; exit 1; }
 grep -q "11" <(sed -n '/^turn 2/,$p' "$out/tools.txt") || { echo "$q: the answer does not use the tool's result" >&2; cat "$out/tools.txt" >&2; exit 1; }
 echo "$q: tool call get_weather(city=Paris), and the answer uses its result"
done
# Step 5: parity with the packaged llama.cpp (a reference, never a runtime
# dependency): token ids, every layer's residual stream, full logits and a
# greedy continuation, with the prompt fed a token at a time.
llama=${LLAMA_PREFIX:-/opt/homebrew}
if [[ ${#ggufs[@]} -gt 0 && ${PSIONIC_GRANITE_PARITY:-1} == 1 && ! -f $llama/include/llama.h ]];then echo "SKIP: parity: no llama.h in $llama (set LLAMA_PREFIX)";fi
if [[ ${#ggufs[@]} -gt 0 && ${PSIONIC_GRANITE_PARITY:-1} == 1 && -f $llama/include/llama.h ]];then
 "$cc" -std=c11 -O2 -Wall -Wextra -I"$llama/include" psionic/tests/granite_llama.c -L"$llama/lib" -lllama -lggml -lggml-base -Wl,-rpath,"$llama/lib" -o "$out/granite-llama"
 for f in "${ggufs[@]}";do
  q=$(basename "$f" .gguf)
  "$out/granite-run" "$f" "$out/k.metal" dump --prompt "Move the oak tree two meters to the left of the well." --steps 16 --capacity 1024 --out "$out/ps.bin" > /dev/null
  ids=$(python3 -I -c 'import struct,sys;b=open(sys.argv[1],"rb").read();n=struct.unpack_from("<q",b)[0];print(",".join(map(str,struct.unpack_from(f"<{n}q",b,40))))' "$out/ps.bin")
  GRANITE_LLAMA_ONE=1 "$out/granite-llama" "$f" "$ids" 16 "$out/ll.bin" > /dev/null
  python3 -I psionic/tests/granite_parity.py "$out/ps.bin" "$out/ll.bin" --require | sed "s|^|$q: |"
 done
fi
[[ ${#ggufs[@]} -gt 0 ]] || echo "SKIP: no pinned Granite files in $model; only owned checks ran"
if [[ $mutate == 1 ]];then for unit in model tokenizer chat host;do "$coax" mutate psionic/lib/granite/$unit.coax --root-only --jobs "$jobs" --run-steps 200000000;done;fi
