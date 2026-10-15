#!/usr/bin/env bash
# Extract only program binaries from a read-only copy of our public image.
set -euo pipefail
[[ $EUID == 0 ]]
mount_dir=/mnt/space-runtime
output=/var/lib/space-spike/runtime/bin
mkdir -p "$mount_dir" "$output"
! mountpoint -q "$mount_dir"
mount -o ro,noload /dev/disk/by-id/google-space-runtime-part1 "$mount_dir"
trap 'umount "$mount_dir"' EXIT
install -m 0755 "$mount_dir/home/coder/.local/bin/coder-cloud-runtime" "$output/coder"
strip "$output/coder"
# The Claude executable carries an embedded application; preserve its bytes.
install -m 0755 "$mount_dir/usr/local/lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe" "$output/claude"
"$output/coder" --version
"$output/claude" --version
sha256sum "$output/coder" "$output/claude"
