#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: starts a headless compositor, a throwaway home and the mock worker, then
# runs the app's --input-test; every check is in host-main.c run_input)
# The OpenAgents Desktop input test on Linux: the real Wayland window in a headless sway nobody sees,
# typed into with a virtual keyboard (wtype), pressed with a virtual pointer (vpointer.c), pasted into
# from the clipboard (wl-copy), on the mock worker (evals/openagents-desktop/mock_worker, never a model
# or a key). It covers what the offscreen flow test cannot: keys, text, the clipboard and presses
# arriving through the compositor, and the seat's keyboard leaving and coming back.
#   apps/openagents-desktop/linux/input-test.sh            build, then test (on a Linux host)
#   apps/openagents-desktop/linux/input-test.sh --no-build test the binary build.sh made
# OA_INPUT_SHOT=PATH keeps a PNG of the window at the end.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"

if [[ "${1:-}" != --no-build ]]; then
  exec "$here/build.sh" --no-worker --input-test
fi
if [[ -z "${OA_INPUT_TEST_SHELL:-}" ]] && command -v nix-shell >/dev/null 2>&1; then
  export OA_INPUT_TEST_SHELL=1
  exec nix-shell -p sway wtype wl-clipboard grim mesa --run "exec $(printf '%q ' "$0" "$@")"
fi
for tool in sway wtype wl-copy python3; do
  command -v "$tool" >/dev/null || { echo "input-test: $tool is needed" >&2; exit 2; }
done
bin="$here/build/OpenAgents"
[[ -x "$bin" ]] || { echo "input-test: no $bin (run build.sh first)" >&2; exit 2; }
[[ -x "$here/build/vpointer" ]] || { echo "input-test: no $here/build/vpointer (build.sh makes it with wlr-protocols)" >&2; exit 2; }

# A short runtime folder: a Wayland socket's path must fit in 108 bytes.
tmp="$(mktemp -d /tmp/oa-input-XXXXXX)"
sway_pid=""
cleanup() { [[ -z "$sway_pid" ]] || kill "$sway_pid" 2>/dev/null || true; rm -rf -- "${tmp:?}"; }
trap cleanup EXIT
mkdir -p "$tmp/run" "$tmp/home" "$tmp/data" "$tmp/work"
chmod 700 "$tmp/run"

cat > "$tmp/sway.conf" <<'CONF'
output HEADLESS-1 resolution 1400x900 position 0,0
default_border none
focus_follows_mouse no
CONF
cat > "$tmp/script.json" <<'JSON'
{"turns": [{"steps": [{"emit": {"kind": "reply", "text": "Reply from the worker."}},
                      {"emit": {"kind": "ended", "ending": "finished", "text": "Reply from the worker."}}]}]}
JSON
python3="$(command -v python3)"
cat > "$tmp/worker" <<SH
#!/bin/sh
exec "$python3" "$repo/evals/openagents-desktop/mock_worker/mock_worker.py" --script "$tmp/script.json" --record "$tmp/record.jsonl"
SH
chmod +x "$tmp/worker"

# The compositor draws with pixman and the app with Mesa's software Vulkan unless told otherwise, so
# the test runs the same on a host with no GPU.
export XDG_RUNTIME_DIR="$tmp/run"
unset WAYLAND_DISPLAY DISPLAY SWAYSOCK
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER="${WLR_RENDERER:-pixman}" sway -c "$tmp/sway.conf" > "$tmp/sway.log" 2>&1 &
sway_pid=$!
for _ in $(seq 50); do [[ -S "$tmp/run/wayland-1" ]] && break; sleep 0.1; done
[[ -S "$tmp/run/wayland-1" ]] || { echo "input-test: sway did not start" >&2; cat "$tmp/sway.log" >&2; exit 1; }
export WAYLAND_DISPLAY=wayland-1
if [[ -z "${VK_DRIVER_FILES:-}${VK_ICD_FILENAMES:-}" ]]; then
  for d in $(echo "${XDG_DATA_DIRS:-} /run/opengl-driver/share /usr/share" | tr ':' ' '); do
    icd="$(ls "$d"/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -n 1 || true)"
    if [[ -n "$icd" ]]; then export VK_DRIVER_FILES="$icd"; break; fi
  done
  if [[ -z "${VK_DRIVER_FILES:-}" && -d /nix/store ]]; then
    icd="$(ls -1 /nix/store/*mesa*/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -n 1 || true)"
    [[ -z "$icd" ]] || export VK_DRIVER_FILES="$icd"
  fi
fi

shot=()
[[ -n "${OA_INPUT_SHOT:-}" ]] && shot=(--shot "$OA_INPUT_SHOT")
status=0
env -u CODER_DESKTOP_DIR -u CODER_SETTINGS_FILE HOME="$tmp/home" XDG_DATA_HOME="$tmp/data" \
  "$bin" --input-test --resources "$here/build/resources" --worker "$tmp/worker" --cwd "$tmp/work" ${shot[@]+"${shot[@]}"} || status=$?
exit "$status"
