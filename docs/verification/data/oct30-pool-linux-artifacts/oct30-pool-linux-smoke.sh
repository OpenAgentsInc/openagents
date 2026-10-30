#!/usr/bin/env bash
set -euo pipefail
cd /tmp/oct30-pool-linux.LbbBa1
pool_clock=$(date +%s)
pool_to=$((pool_clock / 60 * 60))
pool_from=$((pool_to - 60))
base=(--pool linux-proof --from "$pool_from" --to "$pool_to")
key_a=1111111111111111111111111111111111111111111111111111111111111111
key_b=2222222222222222222222222222222222222222222222222222222222222222
key_c=3333333333333333333333333333333333333333333333333333333333333333
for mode in plain sanitized; do
 export ASAN_OPTIONS=detect_leaks=1:abort_on_error=1
 export UBSAN_OPTIONS=halt_on_error=1
 "./pool-$mode" "${base[@]}" --policy > "$mode-empty-policy.json" 2> "$mode-empty-policy.err"
 "./pool-$mode" "${base[@]}" --policy --provider "$key_b" --provider "$key_a" --provider "$key_b" --buyer "$key_c" > "$mode-role-policy.json" 2> "$mode-role-policy.err"
 for which in noargs publishverify inputonly policybundle timeout unaligned badkey; do
  args=("${base[@]}")
  case "$which" in
   noargs) args=();;
   publishverify) args+=(--verify "$key_a" --publish);;
   inputonly) args+=(--inputs);;
   policybundle) args+=(--policy --bundle);;
   timeout) args+=(--policy --timeout-ms 0);;
   unaligned) args+=(--from "$((pool_from+1))" --policy);;
   badkey) args+=(--provider bad --policy);;
  esac
  if "./pool-$mode" "${args[@]}" > "$mode-$which.out" 2> "$mode-$which.err"; then echo "FAIL accepted invalid $mode/$which";exit 1;fi
  test ! -s "$mode-$which.out"
  if grep -Eq 'AddressSanitizer|runtime error:|LeakSanitizer' "$mode-$which.err";then cat "$mode-$which.err";exit 1;fi
 done
 printf 'PASS %s: empty/default and sorted/deduplicated role policies; seven invalid mode/budget/window/key cases; no sanitizer errors\n' "$mode"
done
cmp plain-empty-policy.json sanitized-empty-policy.json
cmp plain-role-policy.json sanitized-role-policy.json
printf 'WINDOW from=%s to=%s seconds; all smoke modes offline, no live Pylon touched\n' "$pool_from" "$pool_to"
