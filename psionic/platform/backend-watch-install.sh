#!/usr/bin/env bash
# Install only this owned timer; existing healthy backend units are never changed.
set -euo pipefail
[[ $# == 3 || ( $# == 4 && $4 == --enable ) ]] || { echo 'usage: backend-watch-install.sh BINARY START END [--enable]' >&2;exit 2; }
[[ $(id -u) == 0 ]] || { echo 'install as root on the existing backend VM' >&2;exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
bin=$1;start=$2;end=$3
wall=$("$bin" clock | jq -er '.wall')
[[ $("$bin" window "$wall" "$start" "$end") == true ]] || { echo 'the explicit six-hour window is not active' >&2;exit 1; }
marker='OpenAgents fixed local backend readiness watch'
prefix=/opt/openagents-backend-watch
if [[ -e $prefix ]];then [[ -f $prefix/owner && $(cat "$prefix/owner") == "$marker" ]] || { echo 'refusing foreign install directory' >&2;exit 1; };fi
for unit in openagents-backend-watch.service openagents-backend-watch.timer;do
  [[ ! -e /etc/systemd/system/$unit ]] || grep -Fqx "# $marker" "/etc/systemd/system/$unit" || { echo 'refusing foreign unit' >&2;exit 1; }
done
work=$(mktemp -d);trap 'rm -rf "$work"' EXIT
printf 'OA_BACKEND_WATCH_START=%s\nOA_BACKEND_WATCH_END=%s\n' "$start" "$end" > "$work/window"
if [[ -e /etc/openagents-backend-watch.env ]];then cmp -s "$work/window" /etc/openagents-backend-watch.env || { echo 'refusing to reset the existing watch window' >&2;exit 1; };fi
install -d -m 755 "$prefix"
printf '%s\n' "$marker" > "$prefix/owner"
install -m 755 "$bin" "$prefix/backend-watch"
install -m 755 "$here/backend-watch-once.sh" "$prefix/backend-watch-once.sh"
install -m 644 "$here/systemd/openagents-backend-watch.service" /etc/systemd/system/openagents-backend-watch.service
install -m 644 "$here/systemd/openagents-backend-watch.timer" /etc/systemd/system/openagents-backend-watch.timer
install -m 600 "$work/window" /etc/openagents-backend-watch.env
systemctl daemon-reload
if [[ ${4:-} == --enable ]];then systemctl enable --now openagents-backend-watch.timer;fi
systemctl show openagents-backend-watch.timer --property=ActiveState --property=SubState --property=NextElapseUSecMonotonic --no-pager
