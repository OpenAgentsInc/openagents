#!/usr/bin/env bash
# Measures the harness spread on retained workspaces (issue #9657).
#
# Runs `coder-one checks metric-target measure` with one warmup and five
# alternated repeats per workspace, in a container of the task's image
# with no network. The vf2 image gets NetworkX 3.4.2, the version the
# instruction names, committed once as metric-target-9657/vf2-nx:
#
#   docker run --name vf2-nx-9657 IMAGE \
#     pip install --break-system-packages networkx==3.4.2
#   docker commit vf2-nx-9657 metric-target-9657/vf2-nx
#
# Usage: measure-spread.sh [ROOT]   (ROOT: the checkout, default: here)
# ONLY=vf2 or ONLY=vf2v1 runs one vf2 harness version and stops.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="${1:-$(cd "$here/../../../.." && pwd)}"
jobs="$HOME/.openagents/terminal-bench/jobs"
out="$here/records"
mkdir -p "$out"

measure() {
  local name="$1" target="$2" harness="$3"
  (cd "$root" && cargo run -q -p coder-one -- checks metric-target measure \
    --workspace /tmp --target "$here/harness/$target" "${@:4}" \
    --harness "$harness" --warmup 1 --repeats 5 --run-sec 300 --budget-sec 3600 \
    --out "$out/spread-$name.json")
  jq -r '.line' "$out/spread-$name.json"
}

for trial in \
  tb4--coder-one-tunable-v3--vf2-speedup-networkx/vf2-speedup-networkx__jgSFKBL \
  tb4--coder-one-tunable-v2--vf2-speedup-networkx/vf2-speedup-networkx__ArPJp6A; do
  app="$jobs/$trial/artifacts/app"
  # vf2.py, the first version, reused two graph objects for every call
  # and read a cache; vf2-fresh.py builds fresh graphs for each call.
  for version in vf2v1:vf2.py vf2:vf2-fresh.py; do
    [ -n "${ONLY:-}" ] && [ "${version%%:*}" != "$ONLY" ] && continue
    measure "${version%%:*}-${trial##*__}" vf2-target.json \
      "docker run --rm --network none -v $app:/app:ro -v $here/harness:/h:ro metric-target-9657/vf2-nx python3 /h/${version#*:} \"\$1\"" \
      --sided
  done
done
[ -n "${ONLY:-}" ] && exit 0

vigenere_image=tbench-warm/interleaved-vigenere:environment-37a3e35da105ad0d814a
for trial in \
  tb4--coder-one-microluna-v12--interleaved-vigenere--manual-20260924T153948/interleaved-vigenere__WXSZXNq \
  tb4--coder-one-microluna-v11--interleaved-vigenere--manual-20260924T151338/interleaved-vigenere__sbNeZhK \
  tb4--coder-one-microluna-solo--interleaved-vigenere--manual-20260924T135150/interleaved-vigenere__LpxW5dB; do
  cracker="$jobs/$trial/artifacts/app/cracker.py"
  measure "vigenere-${trial##*__}" vigenere-target.json \
    "docker run --rm --network none -v $cracker:/app/cracker.py:ro -v $here/harness:/h:ro $vigenere_image python3 /h/vigenere.py"
done
