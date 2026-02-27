#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEFAULT_PROTOCOL_COMMON_RS="/Users/christopherdavid/code/codex/codex-rs/app-server-protocol/src/protocol/common.rs"
PROTOCOL_COMMON_RS="${CODEX_PROTOCOL_COMMON_RS:-$DEFAULT_PROTOCOL_COMMON_RS}"

printf 'Running Codex protocol parity gate...\n'

if [[ -f "$PROTOCOL_COMMON_RS" ]]; then
    printf 'Using upstream protocol source: %s\n' "$PROTOCOL_COMMON_RS"
    CODEX_PROTOCOL_COMMON_RS="$PROTOCOL_COMMON_RS" \
        cargo test -p codex-client --test protocol_conformance -- --nocapture
else
    printf 'WARN: upstream protocol source missing at %s; conformance test may skip upstream diff.\n' \
        "$PROTOCOL_COMMON_RS" >&2
    cargo test -p codex-client --test protocol_conformance -- --nocapture
fi

cargo test -p autopilot-desktop thread_lifecycle_notifications_are_normalized -- --nocapture
cargo test -p autopilot-desktop apps_and_remote_skill_export_emit_notifications -- --nocapture
cargo test -p autopilot-desktop labs_api_smoke_commands_emit_responses_and_notifications -- --nocapture
cargo test -p autopilot-desktop wire_log_path_is_forwarded_to_lane_runtime -- --nocapture
cargo test -p autopilot-desktop pane_registry::tests::codex_feature_family_commands_are_registered -- --nocapture

printf 'Codex protocol parity gate passed.\n'
