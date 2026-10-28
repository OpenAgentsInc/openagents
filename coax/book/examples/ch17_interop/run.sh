#!/usr/bin/env bash
# Chapter 17: build the book's Coax files as C libraries and call them from
# C and Swift. Prints what run.out holds.
set -euo pipefail
cd "$(dirname "$0")"
COAX=../../../bin/coax
CC=${COAX_CC:-/usr/bin/cc}
out=${TMPDIR:-/tmp}/coax-ch17
mkdir -p "$out"
for name in ease path; do
  $COAX header ../ch17_$name.coax -o "$out/$name.h"
  $COAX lib ../ch17_$name.coax -o "$out/$name.o"
done
$COAX header meter.coax -o "$out/meter.h"
$COAX lib meter.coax -o "$out/meter.o"

echo "== C calls ease"
$CC -std=c11 -Wall -I"$out" -o "$out/ease_host" ease_host.c "$out/ease.o"
"$out/ease_host"
echo "== C++ calls ease"
${COAX_CXX:-/usr/bin/c++} -std=c++17 -Wall -I"$out" -o "$out/ease_hostpp" ease_host.cpp "$out/ease.o"
"$out/ease_hostpp"
echo "== C calls path"
$CC -std=c11 -Wall -I"$out" -o "$out/path_host" path_host.c "$out/path.o"
"$out/path_host"
echo "== Coax calls C (meter)"
$CC -std=c11 -Wall -I"$out" -o "$out/meter_host" meter_host.c "$out/meter.o"
"$out/meter_host"
if command -v swiftc >/dev/null; then
  echo "== Swift calls path"
  swiftc -I "$out" -import-objc-header Bridge.h main.swift path_modes.swift "$out/path.o" "$out/ease.o" -o "$out/swift_host"
  "$out/swift_host"
fi
