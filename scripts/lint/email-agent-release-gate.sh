#!/usr/bin/env bash
set -euo pipefail

scripts/lint/email-agent-quality-gate.sh
cargo test -p openagents-email-agent e2e_harness
