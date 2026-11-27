#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh and XDG desktop file installation; holds no logic)
# Builds and installs OpenAgents Desktop for Linux into ~/.openagents/ and ~/.local/share/applications/.
#   apps/openagents-desktop/linux/install.sh
# OpenAgents Desktop builds a single unified profile (#14116); --minimal and --full are accepted as no-op aliases.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin_dir="${OPENAGENTS_BIN_DIR:-$HOME/.openagents/bin}"
app_dir="${OPENAGENTS_APP_DIR:-$HOME/.openagents/desktop}"
xdg_apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
xdg_icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --minimal|--full) ;;  # unified build profile (#14116); kept as compatibility aliases
    *) echo "install.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

"$here/build.sh"
src_dir="$here/build"

mkdir -p "$bin_dir" "$app_dir" "$xdg_apps" "$xdg_icons"

rm -rf "$app_dir/resources"
cp -R "$src_dir/resources" "$app_dir/resources"
# each file beside, then renamed over: a running app or daemon keeps the file it started from
put() { install -m 0755 "$1" "$2.new" && mv -f "$2.new" "$2"; }
put_data() { install -m 0644 "$1" "$2.new" && mv -f "$2.new" "$2"; }
put "$src_dir/OpenAgents" "$app_dir/OpenAgents"
# the workers and the run daemon build.sh puts beside OpenAgents (host-main.c looks for them there)
rm -f "$app_dir/inky" "$app_dir/mcworker"
helpers=(oa-worker claudeworker agyworker gitguard coder-host openagents-daemon psionic-inkling-serve)
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

echo "installed $bin_dir/openagents-desktop"
echo "desktop entry: $xdg_apps/openagents-desktop.desktop"
