#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh and XDG desktop file installation; holds no logic)
# Builds and installs OpenAgents Desktop for Linux into ~/.openagents/ and ~/.local/share/applications/.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin_dir="${OPENAGENTS_BIN_DIR:-$HOME/.openagents/bin}"
app_dir="${OPENAGENTS_APP_DIR:-$HOME/.openagents/desktop}"
xdg_apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
xdg_icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"

"$here/build.sh"

work="$here/build"
mkdir -p "$bin_dir" "$app_dir" "$xdg_apps" "$xdg_icons"

rm -rf "$app_dir/resources"
cp -R "$work/resources" "$app_dir/resources"
# each file beside, then renamed over: a running app or daemon keeps the file it started from
put() { install -m 0755 "$1" "$2.new" && mv -f "$2.new" "$2"; }
put "$work/OpenAgents" "$app_dir/OpenAgents"
# the workers and the run daemon build.sh puts beside OpenAgents (host-main.c looks for them there)
for name in mcworker claudeworker agyworker coder-host openagents-daemon; do
  if [[ -x "$work/$name" ]]; then put "$work/$name" "$app_dir/$name"; fi
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
