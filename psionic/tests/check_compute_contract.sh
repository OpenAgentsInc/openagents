#!/usr/bin/env bash
# The minimum tensor-compute consumer and its refusals (psionic/fixtures/compute/).
# No GPU: the consumer runs on the CPU, both device profiles must admit it, and
# each refuse-*.coax must fail with the code its first line names. When the
# Metal toolchain is installed, the generated Metal source is also compiled.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};dir=psionic/fixtures/compute
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-compute.XXXXXX");trap 'rm -rf "$out"' EXIT
"$coax" test $dir/consumer.coax | tail -1
"$coax" test $dir/consumer.coax --release | tail -1
for src in $dir/consumer.coax psionic/lib/device/project.coax;do
 "$coax" cuda "$src" > "$out/kernel.cu"
 "$coax" metal-compute "$src" > "$out/kernel.metal"
 if xcrun -sdk macosx -f metal >/dev/null 2>&1;then xcrun -sdk macosx metal -c "$out/kernel.metal" -o "$out/kernel.air";fi
 echo "admitted by coax cuda and coax metal-compute: $src"
done
n=0
for src in $dir/refuse-*.coax;do
 read -r _ _ _ cuda _ metal _ < "$src"
 for profile in cuda:$cuda metal-compute:$metal;do
  if "$coax" ${profile%%:*} "$src" > "$out/refused" 2>&1;then echo "$src: ${profile%%:*} admitted it" >&2;exit 1;fi
  grep -q "error\[${profile#*:}\]" "$out/refused" || { echo "$src: ${profile%%:*} did not refuse with ${profile#*:}" >&2;cat "$out/refused" >&2;exit 1;}
 done
 n=$((n+1))
done
echo "$n refusals hold in both device profiles"
