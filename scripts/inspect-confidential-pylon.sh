#!/usr/bin/env bash
# Optional historical verifier oracle, not part of the Coax/product build.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
action=${1:-verify}
case "$action" in verify|round|tamper-measurement|tamper-key) ;; *) echo 'usage: inspect-confidential-pylon.sh verify|round|tamper-measurement|tamper-key' >&2; exit 2;; esac
source=$(git -C "$root" rev-parse '0b401561a0^')
work=$(mktemp -d "${TMPDIR:-/tmp}/oa-att-oracle.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
git -C "$root" archive "$source" crates/oa-att crates/nostr crates/nostr-transport \
  crates/receipts crates/agent-contract crates/psionic/vendor/openagents-workspace-hack \
  nips/openagents/schemas | tar -x -C "$work"
cat > "$work/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = ["crates/oa-att", "crates/nostr", "crates/nostr-transport", "crates/receipts", "crates/agent-contract"]
[workspace.package]
version = "1.0.0"
publish = false
edition = "2024"
rust-version = "1.97.1"
[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
[patch.crates-io]
openagents-workspace-hack = { path = "crates/psionic/vendor/openagents-workspace-hack" }
EOF
cp "$root/docs/verification/data/oct30-confidential/oracle-Cargo.lock" "$work/Cargo.lock"
export CARGO_TARGET_DIR=${OA_ATT_ORACLE_TARGET:-${XDG_CACHE_HOME:-$HOME/.cache}/openagents/att-oracle-$source}
cargo +1.97.1 build --locked --manifest-path "$work/Cargo.toml" -p oa-att --features net >&2
args=(verify --publisher 77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e)
case "$action" in
  round) args[0]=round;args+=(--state 'The weather in Lisbon is sunny today.' --question 'Is this about the weather?');;
  tamper-measurement) args+=(--tamper measurement);;
  tamper-key) args+=(--tamper unbound);;
esac
"$CARGO_TARGET_DIR/debug/oa-att" "${args[@]}"
