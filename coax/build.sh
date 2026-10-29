#!/usr/bin/env bash
# Bootstrap from a private source snapshot, publish only matching artifacts.
#   coax/build.sh          build
#   coax/build.sh test     build, then run tests/run.sh
set -euo pipefail
export LC_ALL=C
cd "$(dirname "$0")"
root="$(cd .. && pwd -P)"
expected="$(./provenance.sh fingerprint "$root")"
snapshot="$(mktemp -d "${TMPDIR:-/tmp}/coax-build-snapshot.XXXXXXXX")"
trap 'rm -rf "$snapshot"' EXIT
cp -R "$root/coax" "$snapshot/coax"
rm -rf "$snapshot/coax/bin"
mkdir -p "$snapshot/scripts"
cp "$root/scripts/coax-release.sh" "$snapshot/scripts/coax-release.sh"
[[ "$("$snapshot/coax/provenance.sh" fingerprint "$snapshot")" == "$expected" ]] || { echo 'compiler inputs changed while creating snapshot' >&2; exit 1; }
"$snapshot/coax/bootstrap-build.sh" --private-snapshot
# Compare the compiled snapshot to the current target inputs before any
# compiler or supporting artifact is installed.
./provenance.sh verify "$root" "$snapshot/coax/bin/coax" "$snapshot/coax/bin/provenance-check" "$snapshot/coax/bin/provenance.record"
mkdir -p bin
for artifact in lexer.c lexer.h parser.c parser.h checker.c checker.h sources.c sources.h proto_runtime.inc proto_importer.c proto_importer.h; do
  cp "$snapshot/coax/bin/$artifact" "bin/.$artifact.$$"
  mv "bin/.$artifact.$$" "bin/$artifact"
done
./provenance.sh install "$root" "$snapshot/coax/bin"
if [[ "${1:-}" == "test" ]]; then
  rm -rf "$snapshot"; trap - EXIT
  exec tests/run.sh
fi
