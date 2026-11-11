#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around clang; the check is a11y-check.m). The shared Mac host's
# VoiceOver elements (#13180), in a window that is never shown and offscreen. No permission prompt.
#   apps/shared/host/apple/a11y-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/a11y-check.XXXXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
# a shader with the component shader's entry points and inputs; the stand-in draws nothing
cat > "$work/check.metal" <<'METAL'
#include <metal_stdlib>
using namespace metal;
struct In { float4 a [[attribute(0)]]; };
vertex float4 chat_fx_vs(In v [[stage_in]]) { return v.a; }
fragment float4 chat_fx_fs() { return float4(0); }
METAL
/usr/bin/clang -fobjc-arc -O1 -w -I"$here/../linux" -I"$here" "$here/a11y-check.m" "$here/ui-host.m" \
  -framework AppKit -framework Metal -framework QuartzCore -framework ImageIO -framework UniformTypeIdentifiers -o "$work/a11y-check"
"$work/a11y-check" "$work/check.metal"
