#!/usr/bin/env bash
set -euo pipefail
cd /tmp/oct30-pool-linux.LbbBa1
mapfile -d '' -t pool_public_args < <(/home/christopherdavid/.nix-profile/bin/bun -e 'const a=await Bun.file("verify-argv.json").json();if(!Array.isArray(a)||!a.every(x=>typeof x==="string"))throw Error("args");a.shift();for(const v of a)process.stdout.write(v+"\0");')
ASAN_OPTIONS=detect_leaks=1:abort_on_error=1 UBSAN_OPTIONS=halt_on_error=1 ./pool-v4-sanitized "${pool_public_args[@]}" < pool.pb > v4-public-verified.json 2> v4-public-verified.err
/home/christopherdavid/.nix-profile/bin/bun -e 'const x=await Bun.file("v4-public-verified.json").json();if(x.verified!==true||x.id!=="36669dc4fd9b01ffaa4e66e7df63cc9750404f97d0a59150987fe42bac25f459")throw Error("verification failed");'
test ! -s v4-public-verified.err
printf 'PASS Linux v4 sanitizer: actual public2-Pylon capacity snapshot independently verified from regular stdin; no sanitizer errors\n'
cat v4-public-verified.json
sha256sum pool.pb pool-v4-sanitized
