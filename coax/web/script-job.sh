#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: takes the environment away, applies the limits the Coax tool states, denies the network where the platform can, then runs the tool)
# The compile job for a realm script someone else wrote (metaverse-engine/tools/script-job.coax,
# GitHub #13467). The API runs it through OA_SCRIPT_JOB:
#   coax/web/script-job.sh SOURCE.coax NAME OUTDIR    OUTDIR/NAME.wasm and NAME.json, or why not (exit 1)
#   coax/web/script-job.sh --warm                     builds the two tools the job runs, outside the limits
# What it enforces, and what it does not:
#   - No inherited environment (env -i): the job sees PATH, HOME and TMPDIR (both a directory of its
#     own), and where the compilers are. No token, key or database address of the caller reaches it.
#   - CPU seconds and file size (ulimit -t, -f) on macOS and Linux. Address space (ulimit -v) on Linux;
#     macOS does not enforce it, so memory is bounded there only by the time limits.
#   - The network: on macOS sandbox-exec denies it (and writes outside the job's directory); on Linux a
#     new user and network namespace (unshare -rn) where the kernel allows one. OA_SCRIPT_JOB_SANDBOX says
#     what happens where neither works: `required` refuses to run (the setting for a server), `try` (the
#     default, for a developer's machine) runs without and says so on stderr, `off` does not try.
#   - It does not hide the file system from reads: the job runs as the caller's user and can read what
#     that user can. A server runs it as a user that holds nothing (see docs/protocol/world-scripts-v1.md).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
source "$here/toolchain.sh"
coax="${COAX:-$repo/coax/bin/coax}"
job="$repo/metaverse-engine/tools/script-job.coax"
if [[ "${1:-}" == --warm ]]; then
  COAX="$coax" "$here/tool.sh" "$job" --limits > /dev/null
  COAX="$coax" "$here/tool.sh" "$repo/metaverse-engine/tools/script-build.coax" > /dev/null 2>&1 || true   # no arguments: built, then its usage
  exit 0
fi
[[ $# -eq 3 ]] || { echo "usage: coax/web/script-job.sh SOURCE.coax NAME OUTDIR | --warm" >&2; exit 2; }
limits="$(COAX="$coax" "$here/tool.sh" "$job" --limits)"
limit() { printf '%s\n' "$limits" | sed -n "s/^$1 //p"; }
home="$(mktemp -d)"; trap 'rm -rf "$home"' EXIT
mode="${OA_SCRIPT_JOB_SANDBOX:-try}"; box=()
if [[ "$mode" != off ]]; then
  if [[ "$(uname -s)" == Darwin ]]; then
    profile="(version 1)(allow default)(deny network*)(deny file-write* (subpath \"/Users\") (subpath \"/etc\") (subpath \"/usr\") (subpath \"/opt\"))(allow file-write* (subpath \"$repo/coax/web/build/tools\"))"
    /usr/bin/sandbox-exec -p "$profile" /usr/bin/true 2> /dev/null && box=(/usr/bin/sandbox-exec -p "$profile")
  elif command -v unshare > /dev/null && unshare -rn true 2> /dev/null; then box=(unshare -rn)
  fi
  if [[ ${#box[@]} -eq 0 ]]; then
    [[ "$mode" != required ]] || { echo "script-job: no sandbox here to deny the network, and OA_SCRIPT_JOB_SANDBOX=required" >&2; exit 2; }
    echo "script-job: the network is not denied here (no sandbox-exec or unshare)" >&2
  fi
fi
ulimit -t "$(limit cpu_s)"
ulimit -f "$(limit file_kb)"
ulimit -v "$(limit memory_kb)" 2> /dev/null || true   # macOS does not enforce an address-space limit
set +e
env -i PATH="$PATH" HOME="$home" TMPDIR="$home" COAX="$coax" COAX_TOOL_CACHE=0 COAXIAL_CLANG="$COAXIAL_CLANG" COAXIAL_RESOURCES="$COAXIAL_RESOURCES" \
  ${box[@]+"${box[@]}"} "$here/tool.sh" "$job" "$@"
code=$?
exit $code
