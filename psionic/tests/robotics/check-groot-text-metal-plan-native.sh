#!/usr/bin/env bash
# Thin native ABI/original-header metadata gate, no GPU or CPU model replay.
set -euo pipefail
cd "$(dirname "$0")/../../.."
work="${1:?usage: check-groot-text-metal-plan-native.sh NEW_OUTPUT_DIRECTORY}"
if [[ -e "$work" ]];then echo "native text-plan directory already exists: $work" >&2;exit 2;fi
mkdir -p "$work"
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
"$coax" c psionic/lib/robotics/groot-text-metal-plan.coax --library --header "$work/text-plan.h" > "$work/debug.c"
"$coax" c psionic/lib/robotics/groot-text-metal-plan.coax --release --library --header "$work/text-plan.h" > "$work/release.c"
"$cc" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/groot-text-metal-plan-native.c -lm -o "$work/debug"
"$work/debug" psionic/fixtures/robotics/groot-libero "$work/debug.facts"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/release.c" psionic/tests/robotics/groot-text-metal-plan-native.c -lm -o "$work/release"
"$work/release" psionic/fixtures/robotics/groot-libero "$work/release.facts"
"$cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/groot-text-metal-plan-native.c -lm -o "$work/sanitized"
"$work/sanitized" psionic/fixtures/robotics/groot-libero "$work/sanitized.facts"
python3 -I psionic/tests/robotics/groot_text_plan_reference.py psionic/fixtures/robotics/groot-libero "$work/debug.facts" "$work/release.facts" "$work/sanitized.facts" --record "$work/reference.json"
