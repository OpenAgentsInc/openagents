#!/usr/bin/env bash
# Focused owned-physics checks; Python is independent development tooling only.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax=${COAX:-$PWD/coax/bin/coax};jobs=4;mutate=0;list=0
units=(oscillator oscillator-data oscillator-learn oscillator-evaluate inverse-data inverse-physics inverse-fit inverse-evaluate oscillator-replay oscillator-replay-codec);chosen=();replay_native=0
while [[ $# -gt 0 ]];do
 case "$1" in
  --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || exit 2;jobs=$2;shift 2;;
  --mutate) mutate=1;shift;; --list) list=1;shift;;
  *) item=${1%.coax};item=${item#psionic/lib/};item=${item#physics/}
   case "$item" in physics|all) chosen=("${units[@]}");;replay) chosen+=(oscillator-replay oscillator-replay-codec);;replay-native) chosen+=(oscillator-replay oscillator-replay-codec);replay_native=1;;inverse) chosen+=(inverse-data inverse-physics inverse-fit inverse-evaluate);;oscillator|oscillator-data|oscillator-learn|oscillator-evaluate|inverse-data|inverse-physics|inverse-fit|inverse-evaluate|oscillator-replay|oscillator-replay-codec) chosen+=("$item");;*) echo "unknown physics unit: $1" >&2;exit 2;;esac;shift;;
 esac
done
if [[ ${#chosen[@]} -eq 0 ]];then chosen=("${units[@]}");fi
if [[ $list == 1 ]];then printf '%s\n' "${chosen[@]}";exit;fi
work=$(mktemp -d "${TMPDIR:-/tmp}/psionic-physics.XXXXXX");trap 'rm -rf "$work"' EXIT
oscillator=0;inverse=0
for unit in "${chosen[@]}";do
 case "$unit" in oscillator|oscillator-data|oscillator-learn|oscillator-evaluate) oscillator=1;;inverse*) inverse=1;;esac
 source=psionic/lib/physics/$unit.coax
 "$coax" test "$source"
 "$coax" test "$source" --release
 if [[ $mutate == 1 ]];then "$coax" mutate "$source" --root-only --jobs "$jobs";fi
done
if [[ $oscillator == 1 ]];then
 COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/physics/check-oscillator.sh "$work/native-oscillator"
fi
if [[ $inverse == 1 ]];then
 COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/physics/check-inverse.sh "$work/native-inverse"
fi
if [[ $replay_native == 1 ]];then
 COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/physics/oscillator-replay-check.sh "$work/native-replay"
fi
examples=()
if [[ $oscillator == 1 ]];then examples+=(learn-a-physical-system);fi
if [[ $inverse == 1 ]];then examples+=(recover-the-physics);fi
for example in ${examples[@]+"${examples[@]}"};do
 for mode in debug release;do
  options=();if [[ $mode == release ]];then options=(--release);fi
  "$coax" run "psionic/examples/$example.coax" ${options[@]+"${options[@]}"} > "$work/$example-$mode.out"
  cmp "psionic/fixtures/$example.out" "$work/$example-$mode.out"
 done
done
python3 psionic/tests/book.py
