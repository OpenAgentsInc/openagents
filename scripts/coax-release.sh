#!/usr/bin/env bash
# Coax owns metadata/validation/rendering. This adapter compiles it, transports
# its fixed output bundle and compares/writes files. No Python build dependency.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
mode=${1:---check}; [[ $mode == --check || $mode == --write ]] || { echo 'usage: scripts/coax-release.sh --check|--write [--binary FILE]' >&2; exit 2; }
shift || true
binary=""
if [[ $# != 0 ]]; then [[ $# == 2 && $1 == --binary ]] || exit 2; binary=$2; fi
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-release.XXXXXX")
trap 'find "$work" -depth -delete' EXIT
compiler=${COAX_RELEASE_COMPILER:-$root/coax/bin/coax}
# This executable runs during Xcode builds too. The generated catalogue is a
# host tool, never an iPhone/Simulator binary, even under an inherited SDKROOT.
host_flags=()
if [[ $(uname -s) == Darwin ]]; then
  export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
  unset IPHONEOS_DEPLOYMENT_TARGET
  host_flags=(-target "${COAX_HOST_TARGET:-$(uname -m)-apple-macosx11.0}")
elif [[ -n ${COAX_HOST_TARGET:-} ]]; then host_flags=(-target "$COAX_HOST_TARGET"); fi
if [[ ! -x $compiler ]]; then
  # Bootstrap only a temporary generator frontend, never publish an unchecked
  # compiler. The checked-in header is the release seed for this first stage.
  mkdir -p "$work/seed"
  for part in lexer parser checker sources; do
    "${CC:-cc}" -std=c11 -O1 "${host_flags[@]}" -c "coax/bootstrap/$part.c" -o "$work/seed/$part.o"
  done
  : > "$work/proto_runtime.inc"
  "${CXX:-c++}" -std=c++20 -O1 "${host_flags[@]}" -DCOAX_BOOTSTRAP_IMPORTER -I coax/bootstrap -I "$work" coax/src/main.cpp "$work/seed/lexer.o" "$work/seed/parser.o" "$work/seed/checker.o" "$work/seed/sources.o" -o "$work/stage0"
  compiler=$work/stage0
fi
"$compiler" c "$root/coax/release.coax" > "$work/catalogue.c"
"${CC:-cc}" -std=c11 -O1 -w -ffp-contract=off "${host_flags[@]}" "$work/catalogue.c" -lm -o "$work/catalogue"
"$work/catalogue" > "$work/bundle"
mkdir -p "$work/outputs"
awk -v dest="$work/outputs" '
BEGIN {
 allowed["coax/src/release.h"]=1;allowed["coax/release-status.json"]=1;
 allowed["coax/STATUS.md"]=1;allowed["coax/book/appendix-a-keywords.md"]=1;
 allowed["coax/generated/current.md"]=1;allowed["coax/generated/version-example.md"]=1;
}
/^@@coax-file / {
 if (active || !($2 in allowed) || seen[$2]++) exit 10;
 path=$2;gsub("/","_",path);file=dest "/" path;active=1;next;
}
/^@@coax-end$/ {if(!active)exit 11;close(file);active=0;next;}
{if(!active)exit 12;print $0 > file;}
END {if(active)exit 13;for(path in allowed)if(!(path in seen))exit 14;}
' "$work/bundle"
files=(coax/src/release.h coax/release-status.json coax/STATUS.md coax/book/appendix-a-keywords.md coax/generated/current.md coax/generated/version-example.md)
failed=0
# Version banners are bounded regions; unrelated prose and historical versions
# are never searched/replaced by value.
managed=(coax/README.md coax/spec.md coax/ROADMAP.md coax/book/ch01-getting-started.md coax/book/ch06-memory.md coax/book/ch20-inside-coax0.md coax/book/ch21-road-ahead.md coax/book/ch22-organizing-programs.md coax/book/appendix-b-error-codes.md)
for path in "${managed[@]}"; do
  awk -v fragment="$work/outputs/coax_generated_current.md" '
  /^<!-- coax-release:current:start -->$/ {print;while((getline line < fragment)>0)print line;close(fragment);active=1;seen++;next;}
  /^<!-- coax-release:current:end -->$/ {if(!active)exit 20;active=0;print;next;}
  !active {print;}
  END {if(active || seen!=1)exit 21;}
  ' "$path" > "$work/outputs/${path//\//_}"
done
path=coax/book/ch01-getting-started.md
awk -v fragment="$work/outputs/coax_generated_version-example.md" '
/^<!-- coax-release:version-example:start -->$/ {print;while((getline line < fragment)>0)print line;close(fragment);active=1;seen++;next;}
/^<!-- coax-release:version-example:end -->$/ {if(!active)exit 22;active=0;print;next;}
!active {print;}
END {if(active || seen!=1)exit 23;}
' "$work/outputs/${path//\//_}" > "$work/managed"
mv "$work/managed" "$work/outputs/${path//\//_}"
if [[ -n $binary ]]; then
  "$binary" status --json > "$work/binary.json"
  "$binary" version > "$work/version"
  "$binary" status > "$work/binary.md"
  if ! cmp -s "$work/binary.json" "$work/outputs/coax_release-status.json" || ! cmp -s "$work/binary.md" "$work/outputs/coax_STATUS.md"; then echo 'release/status drift: compiler binary metadata' >&2; failed=1; fi
  awk -F '"' '/COAX_RELEASE_VERSION\[\]/{print $2}' "$work/outputs/coax_src_release.h" > "$work/expected-version"
  if ! cmp -s "$work/version" "$work/expected-version"; then echo 'release/status drift: compiler version' >&2; failed=1; fi
fi
# Preflight every region and optional binary before writing any snapshots.
[[ $failed == 0 ]] || exit 1
for path in "${files[@]}" "${managed[@]}"; do
  source_file="$work/outputs/${path//\//_}"
  if [[ $mode == --write ]]; then mkdir -p "$(dirname "$path")"; cp "$source_file" "$path";
  elif ! cmp -s "$path" "$source_file"; then echo "release/status drift: $path (run scripts/coax-release.sh --write)" >&2; failed=1; fi
done
[[ $failed == 0 ]]
