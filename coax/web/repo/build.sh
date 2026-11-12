#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: coax c and cc, no logic)
# Builds oa-repo, the indexed repo service (#13326; docs/deployment/repo-vm.md), to OUT:
# serve.coax compiled to a C library (repo_head, repo_serve, repo_fetch_all), linked with its
# host serve.c and the platform's process effects (git runs through process-native.c).
#   coax/web/repo/build.sh OUT [--sanitize]
# vm-deploy.sh deploy --staging runs it on a build host (Debian 12, x86-64, as the VM).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
out="${1:?usage: build.sh OUT [--sanitize]}"
flags=(-std=c11 -O2 -g -D_DEFAULT_SOURCE)
if [[ "${2:-}" == "--sanitize" ]]; then flags=(-std=c11 -O1 -g -D_DEFAULT_SOURCE -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
work="$(mktemp -d "${TMPDIR:-/tmp}/oa-repo-build.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
"${COAX:-$root/coax/bin/coax}" c "$here/serve.coax" --library --run-steps 250000000 --header "$work/serve-lib.h" > "$work/serve-lib.c"
mkdir -p "$(dirname "$out")"
cc "${flags[@]}" -w -c "$work/serve-lib.c" -o "$work/serve-lib.o"
cc "${flags[@]}" -I"$root/coax/platform" -c "$root/coax/platform/process-native.c" -o "$work/process-native.o"
cc "${flags[@]}" -Wall -Wextra -Werror -I"$work" -c "$here/serve.c" -o "$work/serve.o"
cc "${flags[@]}" "$work/serve.o" "$work/serve-lib.o" "$work/process-native.o" -lm -o "$out"
echo "built $out"
