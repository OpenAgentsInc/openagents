#!/usr/bin/env bash
# Install an already built CUDA service under this user's systemd manager.
# No root, downloads, public listener, Python runtime or automatic boot enablement.
set -euo pipefail
if [[ $# -lt 3 || $# -gt 5 ]];then echo 'usage: install-systemone.sh BUILD MODEL_DIRECTORY TOKENIZER_PST [PORT [DEVICE_MIB]]' >&2;exit 2;fi
build=$1 model=$2 table=$3 port=${4:-8798} device=${5:-1024}
[[ $port =~ ^[0-9]+$ && $port -ge 1 && $port -le 65535 && $device =~ ^[0-9]+$ && $device -ge 1 && $device -le 1048576 ]] || exit 2
for path in "$build" "$model" "$table";do [[ $path == /* && $path != *$'\n'* && $path != *$'\r'* ]] || { echo 'absolute paths without line breaks required' >&2;exit 2; };done
identity=$(awk -F'"' '/^#define PSIONIC_BUILD_ID /{print $2}' "$build/serve-build-id.h")
[[ $identity =~ ^[0-9a-f]{64}$ ]] || exit 2
root=${XDG_DATA_HOME:-$HOME/.local/share}/psionic/systemone/$identity
state=${XDG_STATE_HOME:-$HOME/.local/state}/psionic/systemone
units=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
mkdir -p "$root" "$state" "$units"
chmod 700 "$state"
for name in psionic-serve psionic-supervise serve-build.json serve-build-id.h project.cu layer.cu warp.cu;do
    [[ -f $build/$name ]] || { echo "missing build artifact: $name" >&2;exit 2; }
    if [[ -f $root/$name ]];then cmp "$build/$name" "$root/$name";else cp "$build/$name" "$root/$name";fi
done
chmod 755 "$root/psionic-serve" "$root/psionic-supervise"
if [[ -f $root/qwen35.pst ]];then cmp "$table" "$root/qwen35.pst";else cp "$table" "$root/qwen35.pst";fi
table=$root/qwen35.pst
unit=$units/psionic-systemone.service
if [[ -e $unit ]] && ! rg -q '^# Managed by Psionic Part III$' "$unit";then echo 'existing unit is not managed by this installer' >&2;exit 2;fi
quote_unit() { local value=$1;value=${value//\\/\\\\};value=${value//\"/\\\"};value=${value//%/%%};value=${value//\$/\$\$};printf '"%s"' "$value"; }
pending_dir=$(mktemp -d "$units/.psionic-unit.XXXXXX")
pending=$pending_dir/psionic-systemone.service
trap 'rmdir "$pending_dir" 2>/dev/null || true' EXIT
{
    echo '# Managed by Psionic Part III'
    echo '[Unit]'
    echo 'Description=Psionic resident Elephant decision API (loopback)'
    echo '[Service]'
    echo 'Type=notify'
    echo 'NotifyAccess=all'
    printf 'ExecStart=';quote_unit "$root/psionic-supervise";printf ' --restart-limit 5 --backoff-ms 250 -- ';quote_unit "$root/psionic-serve";printf ' --elephant-gpu ';quote_unit "$model";printf ' --table ';quote_unit "$table";printf ' --port %s --capacity 512 --device-mib %s --metrics ' "$port" "$device";quote_unit "$state/usage.jsonl";printf '\n'
    printf 'Environment=';quote_unit "LD_LIBRARY_PATH=$HOME/cuda-13.2/lib";printf '\n'
    echo 'UMask=0077'
    echo 'Restart=no'
    echo 'TimeoutStartSec=720'
    echo 'TimeoutStopSec=10'
    echo 'KillMode=control-group'
} > "$pending"
# Verify the concrete unit before replacing its prior definition.
systemd-analyze --user verify "$pending"
mv "$pending" "$unit"
rmdir "$pending_dir"
systemctl --user daemon-reload
systemctl --user restart psionic-systemone.service
systemctl --user is-active psionic-systemone.service
printf 'Ready build %s on http://127.0.0.1:%s/v1/systemone\n' "$identity" "$port"
