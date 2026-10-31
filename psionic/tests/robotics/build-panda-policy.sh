#!/usr/bin/env bash
# Firm reason5: thin Coax/C/Metal compilation effects, no arithmetic or model.
set -euo pipefail
work="${1:?usage: build-panda-policy.sh BUILD_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
cc="${CC:-cc}"
"$coax" c psionic/lib/robotics/panda-policy.coax --library --release --header "$work/panda-policy-generated.h" > "$work/panda-policy-generated.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform -Ipsionic/tests/robotics -c "$work/panda-policy-generated.c" -o "$work/panda-policy-generated.o"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform -Ipsionic/tests/robotics -c psionic/platform/panda-policy.c -o "$work/panda-policy.o"
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -ffp-contract=off -fno-fast-math -Ipsionic/platform -c psionic/platform/metal-host.m -o "$work/metal-host.o"
"$cc" -std=c11 -O2 -Ipsionic/platform psionic/tests/robotics/panda_policy_frame.c "$work/panda-policy-generated.o" "$work/panda-policy.o" "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/policy-frame"
printf 'Resident policy objects and optional frame effects driver built in %s\n' "$work"
