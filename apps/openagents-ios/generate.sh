#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: generate the Coax Native starter on a build host).
set -euo pipefail
cd "$(dirname "$0")/../.."
exec coax/remote/remote.sh run -- bash coax/native/generate.sh
