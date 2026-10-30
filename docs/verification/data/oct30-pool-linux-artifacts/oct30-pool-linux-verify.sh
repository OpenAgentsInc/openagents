#!/usr/bin/env bash
set -euo pipefail
cd /tmp/oct30-pool-linux.LbbBa1
mapfile -d '' -t pool_verify_args < <(/home/christopherdavid/.nix-profile/bin/bun -e 'const a=await Bun.file("offline-args.json").json();if(!Array.isArray(a)||!a.every(x=>typeof x==="string"))throw Error("args");for(const v of a)process.stdout.write(v+"\0");')
for mode in plain sanitized; do
 ASAN_OPTIONS=detect_leaks=1:abort_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "./pool-$mode" "${pool_verify_args[@]}" < pool-baseline-public.pb > "$mode-verified.json" 2> "$mode-verified.err"
 /home/christopherdavid/.nix-profile/bin/bun -e 'const [f]=process.argv.slice(1);const x=await Bun.file(f).json();if(x.verified!==true||x.id!=="fa816cfd0f5cf9fb535785f6452b3f5daa910de787780621a80b310b7823c135")throw Error("verification failed");' "$mode-verified.json"
 test ! -s "$mode-verified.err"
 printf 'PASS Linux %s: signed public bundle independently recomputed from regular-file stdin\n' "$mode"
 cat "$mode-verified.json"
done
sha256sum pool-baseline-public.pb pool-plain pool-sanitized
