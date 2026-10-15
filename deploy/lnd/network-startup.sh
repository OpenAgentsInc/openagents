#!/usr/bin/env bash
# Multiple interfaces need loose reverse-path checks for metadata replies.
set -euo pipefail
cat > /etc/sysctl.d/80-openagents-lnd-network.conf <<'SYSCTL'
net.ipv4.conf.all.rp_filter=2
net.ipv4.conf.default.rp_filter=2
SYSCTL
sysctl -q -w net.ipv4.conf.all.rp_filter=2 net.ipv4.conf.default.rp_filter=2
