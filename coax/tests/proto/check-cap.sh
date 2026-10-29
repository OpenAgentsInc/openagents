#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
temp=$(mktemp -d "${TMPDIR:-/tmp}/coax-cap-check.XXXXXXXX")
trap 'rm -rf "$temp"' EXIT
check="$here/check-proto-cap.sh"
printf 'PB_MAX_BYTES :: 1048576;\n' > "$temp/wire"
printf 'D_WIRE_MAX_BYTES :: 1048576;\n' > "$temp/importer"
"$check" "$temp/wire" "$temp/importer"
printf 'D_WIRE_MAX_BYTES :: 1048577;\n' > "$temp/importer"
if "$check" "$temp/wire" "$temp/importer" 2>/dev/null; then echo 'mismatch accepted' >&2; exit 1; fi
printf 'D_WIRE_MAX_BYTES :: 1048576;\nD_WIRE_MAX_BYTES :: 1048576;\n' > "$temp/importer"
if "$check" "$temp/wire" "$temp/importer" 2>/dev/null; then echo 'duplicate accepted' >&2; exit 1; fi
printf 'other :: 1048576;\n' > "$temp/importer"
if "$check" "$temp/wire" "$temp/importer" 2>/dev/null; then echo 'missing accepted' >&2; exit 1; fi
printf 'D_WIRE_MAX_BYTES :: 01048576;\n' > "$temp/importer"
if "$check" "$temp/wire" "$temp/importer" 2>/dev/null; then echo 'noncanonical accepted' >&2; exit 1; fi
printf 'D_WIRE_MAX_BYTES :: 1048576; trailing\n' > "$temp/importer"
if "$check" "$temp/wire" "$temp/importer" 2>/dev/null; then echo 'malformed accepted' >&2; exit 1; fi
printf 'protobuf cap parity controls passed\n'
