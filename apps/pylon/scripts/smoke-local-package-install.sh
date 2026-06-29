#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workspace_root="$(cd "$repo_root/../.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

cd "$repo_root"
pack_output="$(bun pm pack)"
tarball="$(printf '%s\n' "$pack_output" | awk '/openagentsinc-pylon-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$tarball" || ! -f "$repo_root/$tarball" ]]; then
  printf 'failed to locate packed Pylon tarball\n' >&2
  printf '%s\n' "$pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/nip90"
nip90_pack_output="$(bun pm pack)"
nip90_tarball="$(printf '%s\n' "$nip90_pack_output" | awk '/openagentsinc-nip90-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$nip90_tarball" || ! -f "$workspace_root/packages/nip90/$nip90_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/nip90 tarball\n' >&2
  printf '%s\n' "$nip90_pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/tassadar-executor"
tassadar_pack_output="$(bun pm pack)"
tassadar_tarball="$(printf '%s\n' "$tassadar_pack_output" | awk '/openagentsinc-tassadar-executor-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$tassadar_tarball" || ! -f "$workspace_root/packages/tassadar-executor/$tassadar_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/tassadar-executor tarball\n' >&2
  printf '%s\n' "$tassadar_pack_output" >&2
  exit 1
fi

# Pylon workspace contract packages: Pylon's bundled runtime imports them,
# so they are transitive deps of the published @openagentsinc/pylon. Pack them
# locally and supply via overrides so this smoke validates the current repo
# contract without requiring registry publishes to have landed first.
cd "$workspace_root/packages/agent-runtime-schema"
agent_runtime_schema_pack_output="$(bun pm pack)"
agent_runtime_schema_tarball="$(printf '%s\n' "$agent_runtime_schema_pack_output" | awk '/openagentsinc-agent-runtime-schema-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$agent_runtime_schema_tarball" || ! -f "$workspace_root/packages/agent-runtime-schema/$agent_runtime_schema_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/agent-runtime-schema tarball\n' >&2
  printf '%s\n' "$agent_runtime_schema_pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/autopilot-control-protocol"
autopilot_control_protocol_pack_output="$(bun pm pack)"
autopilot_control_protocol_tarball="$(printf '%s\n' "$autopilot_control_protocol_pack_output" | awk '/openagentsinc-autopilot-control-protocol-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$autopilot_control_protocol_tarball" || ! -f "$workspace_root/packages/autopilot-control-protocol/$autopilot_control_protocol_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/autopilot-control-protocol tarball\n' >&2
  printf '%s\n' "$autopilot_control_protocol_pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/design-tokens"
design_tokens_pack_output="$(bun pm pack)"
design_tokens_tarball="$(printf '%s\n' "$design_tokens_pack_output" | awk '/openagentsinc-design-tokens-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$design_tokens_tarball" || ! -f "$workspace_root/packages/design-tokens/$design_tokens_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/design-tokens tarball\n' >&2
  printf '%s\n' "$design_tokens_pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/provider-account-schema"
provider_account_pack_output="$(bun pm pack)"
provider_account_tarball="$(printf '%s\n' "$provider_account_pack_output" | awk '/openagentsinc-provider-account-schema-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$provider_account_tarball" || ! -f "$workspace_root/packages/provider-account-schema/$provider_account_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/provider-account-schema tarball\n' >&2
  printf '%s\n' "$provider_account_pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/blueprint-contracts"
blueprint_contracts_pack_output="$(bun pm pack)"
blueprint_contracts_tarball="$(printf '%s\n' "$blueprint_contracts_pack_output" | awk '/openagentsinc-blueprint-contracts-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$blueprint_contracts_tarball" || ! -f "$workspace_root/packages/blueprint-contracts/$blueprint_contracts_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/blueprint-contracts tarball\n' >&2
  printf '%s\n' "$blueprint_contracts_pack_output" >&2
  exit 1
fi

cd "$workspace_root/packages/mcp-contract"
mcp_contract_pack_output="$(bun pm pack)"
mcp_contract_tarball="$(printf '%s\n' "$mcp_contract_pack_output" | awk '/openagentsinc-mcp-contract-.*\.tgz$/ {print $1}' | tail -1)"

if [[ -z "$mcp_contract_tarball" || ! -f "$workspace_root/packages/mcp-contract/$mcp_contract_tarball" ]]; then
  printf 'failed to locate packed @openagentsinc/mcp-contract tarball\n' >&2
  printf '%s\n' "$mcp_contract_pack_output" >&2
  exit 1
fi

cd "$tmp_dir"
cat > package.json <<EOF
{
  "name": "pylon-install-smoke",
  "private": true,
  "type": "module",
  "dependencies": {
    "@openagentsinc/pylon": "file:$repo_root/$tarball"
  },
  "overrides": {
    "@openagentsinc/agent-runtime-schema": "file:$workspace_root/packages/agent-runtime-schema/$agent_runtime_schema_tarball",
    "@openagentsinc/autopilot-control-protocol": "file:$workspace_root/packages/autopilot-control-protocol/$autopilot_control_protocol_tarball",
    "@openagentsinc/design-tokens": "file:$workspace_root/packages/design-tokens/$design_tokens_tarball",
    "@openagentsinc/nip90": "file:$workspace_root/packages/nip90/$nip90_tarball",
    "@openagentsinc/tassadar-executor": "file:$workspace_root/packages/tassadar-executor/$tassadar_tarball",
    "@openagentsinc/provider-account-schema": "file:$workspace_root/packages/provider-account-schema/$provider_account_tarball",
    "@openagentsinc/blueprint-contracts": "file:$workspace_root/packages/blueprint-contracts/$blueprint_contracts_tarball",
    "@openagentsinc/mcp-contract": "file:$workspace_root/packages/mcp-contract/$mcp_contract_tarball"
  }
}
EOF
bun --dns-result-order=ipv4first install >/dev/null
PYLON_HOME="$tmp_dir/pylon-home" bunx pylon bootstrap --json > bootstrap.json
bun -e 'const summary = await Bun.file("bootstrap.json").json(); if (summary.packageName !== "@openagentsinc/pylon" || summary.bin !== "pylon" || !summary.platform.supported) process.exit(1);'

printf 'local package install smoke passed\n'
