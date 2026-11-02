#!/usr/bin/env bash
# Firm reason: 4. It checks coax/published.sh, which installs the compiler before Coax runs.
#
# A published build's manifest, names and hashes are checked before anything is
# installed: a good build passes, and a wrong source fingerprint, platform,
# file hash, missing file, extra file, or a compiler that does not run is
# refused. Uses stand-in files; needs no compiler and no network.
#   coax/tests/published-build.sh
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd -P)"
p="$repo/coax/published.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-published-test.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
sha() { if command -v shasum > /dev/null 2>&1; then shasum -a 256 | awk '{print $1}'; else sha256sum | awk '{print $1}'; fi; }
files=(coax provenance-check check-domains.c check-domains.h lexer.c lexer.h parser.c parser.h checker.c checker.h sources.c sources.h proto_runtime.inc proto_importer.c proto_importer.h mutation-survivors)
source_fp="$("$repo/coax/provenance.sh" source-fingerprint "$repo")"
platform="$("$p" platform)"
# A stand-in compiler that runs: a static one on Linux (the check refuses shared
# libraries there), a shell script elsewhere.
make_coax() {
  if [[ "$(uname -s)" == Linux ]] && command -v cc > /dev/null && printf 'int main(void){return 0;}\n' > "$work/main.c" && cc -static "$work/main.c" -o "$1" 2> /dev/null; then return 0; fi
  [[ "$(uname -s)" != Linux ]] || { echo 'published-build: needs a C compiler that links statically (skipped)'; exit 0; }
  printf '#!/bin/sh\nexit 0\n' > "$1"; chmod +x "$1"
}
build() {
  local dir="$1" n
  rm -rf "$dir"; mkdir -p "$dir"
  make_coax "$dir/coax"; cp "$dir/coax" "$dir/provenance-check"
  for n in "${files[@]:2}"; do printf '%s\n' "$n" > "$dir/$n"; done
  : > "$dir/mutation-survivors"
  { echo coax-published-v1; echo "source $source_fp"; echo "platform $platform"; echo "commit test"
    for n in "${files[@]}"; do echo "file $n $(sha < "$dir/$n")"; done; } > "$dir/manifest"
}
pass() { "$p" check "$1" "$repo" > /dev/null 2>&1 || { echo "published-build: refused a good build ($2)" >&2; "$p" check "$1" "$repo"; exit 1; }; }
refuse() { if "$p" check "$1" "$repo" > /dev/null 2>&1; then echo "published-build: accepted $2" >&2; exit 1; fi; }
d="$work/b"
build "$d"; pass "$d" good
build "$d"; sed -i.bak "s/^source .*/source 0000/" "$d/manifest"; rm -f "$d/manifest.bak"; refuse "$d" 'another source fingerprint'
build "$d"; sed -i.bak "s/^platform .*/platform plan9-mips/" "$d/manifest"; rm -f "$d/manifest.bak"; refuse "$d" 'another platform'
build "$d"; echo tampered >> "$d/lexer.c"; refuse "$d" 'a file that differs from its hash'
build "$d"; rm "$d/parser.h"; refuse "$d" 'a missing file'
build "$d"; echo x > "$d/extra"; refuse "$d" 'an extra file'
build "$d"; printf '#!/bin/sh\nexit 1\n' > "$d/coax"; sed -i.bak "s/^file coax .*/file coax $(sha < "$d/coax")/" "$d/manifest"; rm -f "$d/manifest.bak"
[[ "$(uname -s)" == Linux ]] && refuse "$d" 'a dynamically linked or failing compiler' || refuse "$d" 'a compiler that does not run'
echo "published-build: ok (a good build passes; source, platform, hash, missing, extra and failing compiler refused)"
