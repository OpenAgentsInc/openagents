#!/usr/bin/env bash
# Fail closed if the owned runtime and importer disagree on the message cap.
set -euo pipefail
[[ $# == 2 ]] || { echo 'usage: check-proto-cap.sh WIRE IMPORTER' >&2; exit 1; }
read_cap() {
  awk -v name="$1" '
    $1 == name {
      count++
      if (NF != 3 || $2 != "::" || $3 !~ /^(0|[1-9][0-9]*);$/) bad=1
      else { value=$3; sub(/;$/, "", value) }
    }
    END { if (count != 1 || bad) exit 1; print value }
  ' "$2"
}
wire=$(read_cap PB_MAX_BYTES "$1") || { echo 'invalid wire message cap' >&2; exit 1; }
importer=$(read_cap D_WIRE_MAX_BYTES "$2") || { echo 'invalid importer message cap' >&2; exit 1; }
[[ "$wire" == "$importer" ]] || { echo 'protobuf message caps differ' >&2; exit 1; }
