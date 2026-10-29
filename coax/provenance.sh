#!/usr/bin/env bash
# Consistency record for one checked compiler build. This is not a signature.
set -euo pipefail
export LC_ALL=C
fail() { echo "coax provenance: $*" >&2; exit 1; }
sha() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then sha256sum | awk '{print $1}'
  else fail 'SHA-256 tool unavailable'; fi
}
file_hash() { [[ -f "$1" && ! -L "$1" ]] || fail "not a regular file: $1"; sha < "$1"; }
fingerprint() (
  local root="$1" dir path relative cc cxx cc_path cxx_path cc_version cxx_version sdk_path sdk_version digest listing links
  listing="$(mktemp "${TMPDIR:-/tmp}/coax-inputs.XXXXXXXX")"
  links="$(mktemp "${TMPDIR:-/tmp}/coax-input-links.XXXXXXXX")"
  trap 'rm -f "$listing" "$links"' EXIT
  root="$(cd "$root" && pwd -P)"
  for dir in coax coax/lib scripts coax/src coax/bootstrap coax/lib/compiler coax/lib/proto; do
    [[ -d "$root/$dir" && ! -L "$root/$dir" ]] || fail "missing or linked input directory: $dir"
  done
  for dir in coax/src coax/bootstrap coax/lib/compiler coax/lib/proto; do
    find "$root/$dir" -type l -print -quit > "$links" || fail "cannot inspect $dir"
    [[ ! -s "$links" ]] || fail "linked compiler input under $dir"
  done
  find "$root/coax/src" "$root/coax/bootstrap" "$root/coax/lib/compiler" "$root/coax/lib/proto" -type f -print0 | sort -z > "$listing" || fail 'cannot inventory compiler inputs' 
  cc="${CC:-cc}"; cxx="${CXX:-c++}"
  cc_path="$(command -v "$cc")" || fail 'C compiler unavailable'
  cxx_path="$(command -v "$cxx")" || fail 'C++ compiler unavailable'
  cc_version="$("$cc" --version)" || fail 'cannot identify C compiler'
  cxx_version="$("$cxx" --version)" || fail 'cannot identify C++ compiler'
  if [[ "$(uname -s)" == Darwin ]]; then
    sdk_path="$(xcrun --show-sdk-path)" || fail 'cannot identify default SDK'
    sdk_version="$(xcrun --show-sdk-version)" || fail 'cannot identify default SDK version'
  fi
  {
    printf 'coax-input-v1\0'
    for path in coax/build.sh coax/bootstrap-build.sh coax/check-proto-cap.sh coax/provenance.sh coax/provenance-bridge.c coax/release.coax coax/release-model.coax scripts/coax-release.sh; do
      digest="$(file_hash "$root/$path")" || exit 1
      printf '%s\0%s\0' "$path" "$digest"
    done
    while IFS= read -r -d '' path; do
      relative="${path#"$root"/}"
      digest="$(file_hash "$path")" || exit 1
      printf '%s\0%s\0' "$relative" "$digest"
    done < "$listing"
    printf 'os\0%s\0architecture\0%s\0' "$(uname -s)" "$(uname -m)"
    printf 'cc\0%s\0%s\0cxx\0%s\0%s\0' "$cc_path" "$cc_version" "$cxx_path" "$cxx_version"
    if [[ "$(uname -s)" == Darwin ]]; then
      printf 'default-sdk\0%s\0%s\0' "$sdk_path" "$sdk_version"
    fi
    for path in COAX_HOST_TARGET SDKROOT MACOSX_DEPLOYMENT_TARGET IPHONEOS_DEPLOYMENT_TARGET DEVELOPER_DIR CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH; do
      printf '%s\0%s\0' "$path" "${!path-}"
    done
  } | sha
)
[[ $# -ge 2 ]] || fail 'usage: provenance.sh fingerprint ROOT | write ROOT BINARY HELPER RECORD EXPECTED | verify ROOT BINARY HELPER RECORD | install ROOT BIN_DIR'
mode="$1"; root="$2"
if [[ "$mode" == fingerprint ]]; then [[ $# == 2 ]] || fail 'invalid arguments'; fingerprint "$root"; exit; fi
if [[ "$mode" == install ]]; then
  [[ $# == 3 ]] || fail 'invalid arguments'
  source_bin="$3"; target_bin="$root/coax/bin"
  for name in coax provenance-check provenance.record; do
    [[ -f "$source_bin/$name" && ! -L "$source_bin/$name" ]] || fail "missing or linked artifact: $name"
  done
  mkdir -p "$target_bin"
  staging="$(mktemp -d "$target_bin/.provenance-install.XXXXXXXX")"
  trap 'rm -rf "$staging"' EXIT
  cp "$source_bin/coax" "$staging/coax"
  cp "$source_bin/provenance-check" "$staging/provenance-check"
  cp "$source_bin/provenance.record" "$staging/provenance.record"
  "$0" verify "$root" "$staging/coax" "$staging/provenance-check" "$staging/provenance.record"
  mv "$staging/coax" "$target_bin/coax"
  mv "$staging/provenance-check" "$target_bin/provenance-check"
  mv "$staging/provenance.record" "$target_bin/provenance.record"
  exit
fi
if [[ "$mode" == write ]]; then [[ $# == 6 ]] || fail 'invalid arguments'
elif [[ "$mode" == verify ]]; then [[ $# == 5 ]] || fail 'invalid arguments'
else fail 'invalid arguments'; fi
binary="$3"; helper="$4"; record="$5"
[[ -x "$binary" && ! -L "$binary" && -x "$helper" && ! -L "$helper" ]] || fail 'missing or linked executable'
input_hash="$(fingerprint "$root")"; binary_hash="$(file_hash "$binary")"; helper_hash="$(file_hash "$helper")"
if [[ "$mode" == write ]]; then
  "$helper" "$input_hash" "$6" "$binary_hash" "$binary_hash" "$helper_hash" "$helper_hash" || fail 'compiler inputs changed during bootstrap'
  temp="$(mktemp "$(dirname "$record")/.coax-provenance.XXXXXXXX")"
  trap 'rm -f "$temp"' EXIT
  printf 'coax-provenance-v1\n%s\n%s\n%s\n' "$input_hash" "$binary_hash" "$helper_hash" > "$temp"
  mv "$temp" "$record"; trap - EXIT; exit
fi
[[ -f "$record" && ! -L "$record" ]] || fail 'missing or linked build record'
{
  IFS= read -r schema || fail 'malformed record'
  IFS= read -r saved_input || fail 'malformed record'
  IFS= read -r saved_binary || fail 'malformed record'
  IFS= read -r saved_helper || fail 'malformed record'
  if IFS= read -r extra || [[ -n "${extra:-}" ]]; then fail 'malformed record'; fi
} < "$record"
[[ "$schema" == coax-provenance-v1 ]] || fail 'unsupported record'
# Do not run a substituted helper before comparing its actual bytes.
[[ "$helper_hash" == "$saved_helper" ]] || fail 'helper differs from build record'
"$helper" "$input_hash" "$saved_input" "$binary_hash" "$saved_binary" "$helper_hash" "$saved_helper" || fail 'compiler inputs or binary differ from build record'
