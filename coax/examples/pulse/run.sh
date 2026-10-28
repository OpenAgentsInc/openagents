#!/usr/bin/env bash
# Build pulse.coax as a C library and header, link it into host.c, and run.
set -euo pipefail
cd "$(dirname "$0")"
COAX=../../bin/coax
[[ -x $COAX ]] || ../../build.sh
out=${TMPDIR:-/tmp}/coax-pulse
mkdir -p "$out"
$COAX header pulse.coax -o "$out/pulse.h"
$COAX lib pulse.coax --release -o "$out/pulse.o"
${COAX_CC:-cc} -std=c11 -O2 -I"$out" -o "$out/host" host.c "$out/pulse.o"
"$out/host"
