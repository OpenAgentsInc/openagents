#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles CrashReports.swift and its Swift check with swiftc and runs it)
# Checks the apps' crash logs (CrashReports.swift, #13379) on this Mac, with no network and no window:
# a MetricKit crash diagnostic makes the dump proto/verse/crash-report-v3.json holds (the bytes the
# codec's check turns into a report), and nothing is sent before the person says yes.
#   apps/shared/host/apple/crash-reports-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
xcrun -sdk macosx swiftc -O -suppress-warnings -parse-as-library -o "$work/check" "$here/CrashReports.swift" "$here/crash-reports-check.swift" 2> "$work/build.log" || { cat "$work/build.log" >&2; exit 1; }
mkdir -p "$work/state"
perl -e 'alarm 60; exec @ARGV' "$work/check" "$here/crash-diagnostic-fixture.json" "$repo/proto/verse/crash-report-v3.json" "$work/state"
