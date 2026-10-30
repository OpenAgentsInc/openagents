#!/usr/bin/env bash
# Changed Chapter21 authority roots, without unrelated model/text oracles.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
jobs=${PSIONIC_JOBS:-4}
native_only=0 list=0
request=()
while [[ $# -gt 0 ]]; do
  case $1 in
    --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || exit 2; jobs=$2; shift 2;;
    --native-only) native_only=1; shift;;
    --list) list=1; shift;;
    --*) echo "unknown mesh check option: $1" >&2; exit 2;;
    *) request+=("$1"); shift;;
  esac
done
all=(selection opening witness binding receipt compare totals pool-wire pool-bundle pool-policy pool-input-policy)
selected=()
for unit in "${all[@]}"; do
  take=0
  if [[ ${#request[@]} -eq 0 ]]; then take=1; else
    for name in "${request[@]}"; do
      name=${name%.coax}; name=${name#psionic/lib/}
      if [[ $name == mesh || $name == "mesh/$unit" || $name == "$unit" ]]; then take=1; fi
    done
  fi
  if [[ $take -eq 1 ]]; then selected+=("$unit"); fi
done
if [[ ${#selected[@]} -eq 0 ]]; then echo 'no mesh authority unit selected' >&2; exit 2; fi
for name in "${request[@]+"${request[@]}"}"; do
  name=${name%.coax}; name=${name#psionic/lib/}; hit=0
  for unit in "${selected[@]}"; do if [[ $name == mesh || $name == "mesh/$unit" || $name == "$unit" ]]; then hit=1; fi; done
  if [[ $hit -eq 0 ]]; then echo "unknown mesh unit: $name" >&2; exit 2; fi
done
if [[ $list -eq 1 ]]; then printf 'mesh/%s\n' "${selected[@]}"; exit 0; fi
if [[ ! -x $coax ]]; then
  if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
  coax/build.sh
fi
for unit in "${selected[@]}"; do
  if [[ $native_only -eq 0 ]]; then
    "$coax" test "psionic/lib/mesh/$unit.coax"
    "$coax" test "psionic/lib/mesh/$unit.coax" --release
    "$coax" mutate "psionic/lib/mesh/$unit.coax" --library --root-only --jobs "$jobs"
  fi
  case $unit in
    compare|totals|pool-wire|pool-bundle)
      env COAX="$coax" COAX_CC="$cc" "psionic/tests/mesh-$unit-native.sh";;
  esac
done
# Real signed/encrypted native mesh integration is a separate opt-in gate:
# build mesh-build.sh artifacts, then run tests/mesh21/mesh_e2e.ts.
