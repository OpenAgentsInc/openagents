#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
work=$(mktemp -d);trap 'rm -rf "$work"' EXIT
for mode in debug release;do
 args=();opt=-O0
 if [[ "$mode" == release ]];then args+=(--release);opt=-O2;fi
 "$coax" c coax/tests/net/relay_poll_budget.coax --library "${args[@]}" --header "$work/budget.h" > "$work/budget.c"
 "$cc" -std=c11 "$opt" -g -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-variable \
   -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer \
   -I"$work" "$work/budget.c" coax/tests/net/relay_poll_budget.c -lm -o "$work/check"
 "$work/check"
 echo "relay poll budget $mode ASan/UBSan passed"
done
