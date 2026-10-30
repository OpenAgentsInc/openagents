#!/usr/bin/env bash
# Source checks/mutation and native policy/effect adapter. No Google requests.
set -euo pipefail
recovery_here=$(cd "$(dirname "$0")" && pwd)
recovery_root=$(cd "$recovery_here/../.." && pwd)
recovery_work=$(mktemp -d);trap 'rm -rf "$recovery_work"' EXIT
cd "$recovery_root"
coax/bin/coax test psionic/lib/ops/spot-recovery.coax
coax/bin/coax mutate psionic/lib/ops/spot-recovery.coax --root-only --jobs 4
coax/bin/coax c psionic/lib/ops/spot-recovery.coax --library --header "$recovery_work/spot-recovery.h" > "$recovery_work/generated.c"
cmp "$recovery_work/generated.c" "$recovery_here/spot-recovery.generated.c"
cmp "$recovery_work/spot-recovery.h" "$recovery_here/spot-recovery.h"
cc -std=c11 -O2 -I"$recovery_work" "$recovery_work/generated.c" "$recovery_here/spot-recovery-face.c" -lm -o "$recovery_work/decider"
PYLON_RECOVERY_TEST_DECIDER="$recovery_work/decider" bun test psionic/platform/spot-recovery.test.ts
