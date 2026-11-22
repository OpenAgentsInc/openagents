#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh and XDG desktop file installation; holds no logic)
# Builds and installs OpenAgents Desktop for Linux into ~/.openagents/ and ~/.local/share/applications/.
#   apps/openagents-desktop/linux/install.sh [--full|--minimal]
# --minimal (or OA_RELEASE_MINIMAL=1): build and install only the v1.0 minimal artifact set
#   (omitting claudeworker; keeping agyworker for the onboarding's Connect Google Antigravity). Default is --full.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin_dir="${OPENAGENTS_BIN_DIR:-$HOME/.openagents/bin}"
app_dir="${OPENAGENTS_APP_DIR:-$HOME/.openagents/desktop}"
xdg_apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
xdg_icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"

minimal=0
if [[ "${OA_RELEASE_MINIMAL:-0}" == "1" ]]; then minimal=1; fi
while [[ $# -gt 0 ]]; do
  case "$1" in
    --minimal) minimal=1 ;;
    --full) minimal=0 ;;
    *) echo "install.sh: unknown argument $1 (use --full or --minimal)" >&2; exit 2 ;;
  esac
  shift
done

if [[ "$minimal" == 1 ]]; then
  "$here/build.sh" --minimal
  src_dir="$here/build/minimal-stage"
else
  "$here/build.sh" --full
  src_dir="$here/build"
fi

mkdir -p "$bin_dir" "$app_dir" "$xdg_apps" "$xdg_icons"

rm -rf "$app_dir/resources"
cp -R "$src_dir/resources" "$app_dir/resources"
# each file beside, then renamed over: a running app or daemon keeps the file it started from
put() { install -m 0755 "$1" "$2.new" && mv -f "$2.new" "$2"; }
put_data() { install -m 0644 "$1" "$2.new" && mv -f "$2.new" "$2"; }
put "$src_dir/OpenAgents" "$app_dir/OpenAgents"
# the workers and the run daemon build.sh puts beside OpenAgents (host-main.c looks for them there)
if [[ "$minimal" == 1 ]]; then
  rm -f "$app_dir/claudeworker" "$app_dir/openagents-daemon"
  helpers=(mcworker agyworker coder-host gitguard psionic-inkling-serve)
else
  helpers=(mcworker claudeworker agyworker gitguard coder-host openagents-daemon psionic-inkling-serve)
fi
for name in "${helpers[@]}"; do
  if [[ -x "$src_dir/$name" ]]; then put "$src_dir/$name" "$app_dir/$name"; fi
done
for kfile in inkling-kernels-cuda.cu inkling-kernels-vulkan.spv; do
  if [[ -f "$src_dir/$kfile" ]]; then put_data "$src_dir/$kfile" "$app_dir/$kfile"; fi
done

cat > "$bin_dir/openagents-desktop" <<WRAP
#!/usr/bin/env bash
# Firm reason: 5 (thin launcher wrapper; holds no logic)
exec "$app_dir/OpenAgents" --resources "$app_dir/resources" "\$@"
WRAP
chmod 0755 "$bin_dir/openagents-desktop"

sed "s|^Exec=.*|Exec=$bin_dir/openagents-desktop %U|" "$here/openagents-desktop.desktop" > "$xdg_apps/openagents-desktop.desktop"
if [[ -f "$repo/components/icons/oa-mark.svg" ]]; then
  cp "$repo/components/icons/oa-mark.svg" "$xdg_icons/openagents-desktop.svg"
fi

if [[ "$minimal" == 1 ]]; then
  echo "installed $bin_dir/openagents-desktop (minimal)"
else
  echo "installed $bin_dir/openagents-desktop"
fi
echo "desktop entry: $xdg_apps/openagents-desktop.desktop"
