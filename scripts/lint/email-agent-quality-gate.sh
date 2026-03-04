#!/usr/bin/env bash
set -euo pipefail

cargo test -p openagents-email-agent quality_gate_thresholds_hold_for_golden_set
