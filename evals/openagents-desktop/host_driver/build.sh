#!/usr/bin/env bash
# Firm reason: 5 (thin build wrapper invoking coax and the C compiler for the headless eval host driver)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/build"

mkdir -p "$work"

if [ ! -x "$coax" ]; then
  echo "host_driver/build.sh: no coax compiler at $coax; run coax/build.sh first" >&2
  exit 127
fi

# Rebuild gallery.c/gallery.h if app.coax or page.coax is newer than gallery.c
need_coax=0
if [ ! -f "$work/gallery.c" ] || [ ! -f "$work/gallery.h" ]; then
  need_coax=1
elif [ -n "$(find "$repo/apps/openagents-desktop" "$repo/coax/lib/agent/engine" -name '*.coax' -newer "$work/gallery.c" 2>/dev/null | head -1)" ]; then
  # the page is page.coax and its parts (page/*.coax, page/layout/*.coax) and the worker codec
  need_coax=1
fi

if [ "$need_coax" -eq 1 ]; then
  "$coax" c "$repo/apps/openagents-desktop/app.coax" --library --header "$work/gallery.h" > "$work/gallery.c"
fi

if [ "$need_coax" -eq 0 ] && [ -x "$work/host_driver" ] \
   && [ "$work/host_driver" -nt "$work/gallery.c" ] \
   && [ "$work/host_driver" -nt "$work/gallery.h" ] \
   && [ "$work/host_driver" -nt "$repo/apps/openagents-desktop/carrier.c" ] \
   && [ "$work/host_driver" -nt "$here/ui_host_stub.c" ] \
   && [ "$work/host_driver" -nt "$here/ui_host_stub.h" ] \
   && [ "$work/host_driver" -nt "$here/host_driver.c" ] \
   && [ "$work/host_driver" -nt "$here/build.sh" ]; then
  exit 0
fi

# clang, or the C compiler when this clang has no C library headers (NixOS's unwrapped clang).
cc_bin="${CC:-clang}"
if ! printf '#include <signal.h>\n' | "$cc_bin" -x c -fsyntax-only - >/dev/null 2>&1; then cc_bin=cc; fi
# No warnings for the generated code, but an undeclared call stays an error: its pointer result
# would be cut to an int (ui_view on Linux, #13734). -w would hide it on older clang.
quiet=(-Wno-everything)
"$cc_bin" --version 2>/dev/null | grep -qi clang || quiet=(-w)
cflags=(-O2 -ffp-contract=off -fno-fast-math "${quiet[@]}" -Werror=implicit-function-declaration -I"$work" -I"$repo/apps/shared/host/linux" -I"$repo/apps/openagents-desktop" -I"$here")

"$cc_bin" -std=gnu11 "${cflags[@]}" -include "$here/ui_host_stub.h" -c "$repo/apps/openagents-desktop/carrier.c" -o "$work/carrier.o"
"$cc_bin" -std=gnu11 "${cflags[@]}" -c "$work/gallery.c" -o "$work/gallery.o"
"$cc_bin" -std=gnu11 "${cflags[@]}" -c "$here/ui_host_stub.c" -o "$work/ui_host_stub.o"
"$cc_bin" -std=gnu11 "${cflags[@]}" -c "$here/host_driver.c" -o "$work/host_driver.o"
ldflags=(-lm)
if [ "$(uname -s)" = "Darwin" ]; then
  ldflags+=(-framework CoreFoundation -framework CoreGraphics -framework ImageIO)
fi
"$cc_bin" "${cflags[@]}" "$work/carrier.o" "$work/gallery.o" "$work/ui_host_stub.o" "$work/host_driver.o" "${ldflags[@]}" -o "$work/host_driver"
