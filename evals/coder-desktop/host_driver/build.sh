#!/usr/bin/env bash
# Firm reason: 5 (thin build wrapper invoking coax and clang for the headless eval host driver)
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
elif [ -n "$(find "$repo/apps/coder-desktop" "$repo/coax/lib/agent/engine" -name '*.coax' -newer "$work/gallery.c" 2>/dev/null | head -1)" ]; then
  # the page is page.coax and its parts (page/*.coax, page/layout/*.coax) and the worker codec
  need_coax=1
fi

if [ "$need_coax" -eq 1 ]; then
  "$coax" c "$repo/apps/coder-desktop/app.coax" --library --header "$work/gallery.h" > "$work/gallery.c"
fi

# No warnings for the generated code, but an undeclared call stays an error: its pointer result
# would be cut to an int (ui_view on Linux, #13734). -w would hide it on older clang.
cflags=(-O2 -ffp-contract=off -fno-fast-math -Wno-everything -Werror=implicit-function-declaration -I"$work" -I"$repo/apps/shared/host/linux" -I"$repo/apps/coder-desktop" -I"$here")

clang -std=gnu11 "${cflags[@]}" -include "$here/ui_host_stub.h" -c "$repo/apps/coder-desktop/carrier.c" -o "$work/carrier.o"
clang -std=gnu11 "${cflags[@]}" -c "$work/gallery.c" -o "$work/gallery.o"
clang -std=gnu11 "${cflags[@]}" -c "$here/ui_host_stub.c" -o "$work/ui_host_stub.o"
clang -std=gnu11 "${cflags[@]}" -c "$here/host_driver.c" -o "$work/host_driver.o"
clang "${cflags[@]}" "$work/carrier.o" "$work/gallery.o" "$work/ui_host_stub.o" "$work/host_driver.o" -lm -o "$work/host_driver"
