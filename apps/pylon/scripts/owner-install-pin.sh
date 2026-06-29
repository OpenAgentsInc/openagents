#!/usr/bin/env bash
# Owner install pin for the unpublished Pylon 0.3.0 source checkout.
#
# Context (#4858): the published npm package is 0.2.5 and the stable 0.3.0
# publish is blocked on npm credentials for the @openagentsinc scope. Until
# the publish lands, this script gives the owner a pinned daily-driver
# command that does not depend on unpublished package semantics, per the
# readiness audit's P0 item 8
# (docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md).
#
# It installs a `pylon-dev` launcher into ~/.local/bin that runs this source
# checkout via bun, and records a pin manifest (checkout path, pinned commit,
# dirty state, installedAt) at ~/.config/openagents/pylon-pin.json so the pin
# is inspectable and honest about what it points at.
#
# This is an owner-only dogfood convenience. It does NOT satisfy the
# pylon.local_claude_agent_bridge.v1 packaged-binary blocker, whose
# verification requires a published stable binary (#4859).
set -euo pipefail

checkout_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin_dir="${HOME}/.local/bin"
config_dir="${HOME}/.config/openagents"
launcher="${bin_dir}/pylon-dev"
pin_manifest="${config_dir}/pylon-pin.json"

if ! command -v bun >/dev/null 2>&1; then
  echo "error: bun is required on PATH" >&2
  exit 1
fi

commit="$(git -C "${checkout_dir}" rev-parse HEAD)"
dirty_count="$(git -C "${checkout_dir}" status --porcelain | wc -l | tr -d ' ')"
installed_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

mkdir -p "${bin_dir}" "${config_dir}"

cat > "${launcher}" <<LAUNCHER
#!/usr/bin/env bash
# Pinned Pylon source launcher (owner install pin, #4858).
# Pinned checkout: ${checkout_dir}
exec bun "${checkout_dir}/src/index.ts" "\$@"
LAUNCHER
chmod +x "${launcher}"

cat > "${pin_manifest}" <<MANIFEST
{
  "schema": "openagents.pylon.owner_install_pin.v1",
  "checkoutDir": "${checkout_dir}",
  "pinnedCommit": "${commit}",
  "dirtyFileCountAtInstall": ${dirty_count},
  "installedAt": "${installed_at}",
  "launcher": "${launcher}",
  "publishedPackageFallbackFor": "@openagentsinc/pylon@0.3.0 (unpublished; npm credential repair pending, #4858)",
  "doesNotSatisfy": "pylon.local_claude_agent_bridge.v1 packaged-binary blocker (#4859)"
}
MANIFEST

echo "installed ${launcher} -> ${checkout_dir} @ ${commit} (dirty files at install: ${dirty_count})"
echo "pin manifest: ${pin_manifest}"
if [ "${dirty_count}" != "0" ]; then
  echo "warning: checkout had uncommitted changes at install time" >&2
fi
case ":$PATH:" in
  *":${bin_dir}:"*) ;;
  *) echo "note: add ${bin_dir} to PATH to use 'pylon-dev'" ;;
esac
