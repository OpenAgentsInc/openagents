#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd python3
require_cmd rsync

CONTEXT_DIR="${1:-$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX")}"
NEXUS_LOCKFILE_PATH="${ROOT_DIR}/apps/nexus-relay/deploy/Cargo.nexus.lock"
REAL_WORKSPACE_PATHS=(
  "apps/nexus-control"
  "apps/nexus-relay"
  "crates/openagents-kernel-core"
  "crates/openagents-kernel-proto"
  "crates/openagents-provider-substrate"
  "crates/psionic-train-contract"
  "crates/openagents-validator-service"
  "crates/nostr/client"
  "crates/nostr/core"
  "crates/spark"
)

mkdir -p "$CONTEXT_DIR"
find "$CONTEXT_DIR" -mindepth 1 -maxdepth 1 -exec rm -rf {} +

cp "$ROOT_DIR/Cargo.toml" "$CONTEXT_DIR/Cargo.toml"
if [[ -f "$NEXUS_LOCKFILE_PATH" ]]; then
  cp "$NEXUS_LOCKFILE_PATH" "$CONTEXT_DIR/Cargo.lock"
else
  cp "$ROOT_DIR/Cargo.lock" "$CONTEXT_DIR/Cargo.lock"
fi

COPY_PATHS=(
  ".cargo"
  ".dockerignore"
  ".gcloudignore"
  "proto"
  "scripts/dev/protocw"
  "apps/nexus-control"
  "apps/nexus-relay"
  "crates/openagents-kernel-core"
  "crates/openagents-kernel-proto"
  "crates/openagents-provider-substrate"
  "crates/psionic-train-contract"
  "crates/openagents-validator-service"
  "crates/nostr/client"
  "crates/nostr/core"
  "crates/spark"
  "third_party/nostr-rs-relay"
)

for relative_path in "${COPY_PATHS[@]}"; do
  if [[ -e "${ROOT_DIR}/${relative_path}" ]]; then
    destination_dir="${CONTEXT_DIR}/$(dirname "${relative_path}")"
    mkdir -p "${destination_dir}"
    rsync -a "${ROOT_DIR}/${relative_path}" "${destination_dir}/"
  fi
done

python3 - "$ROOT_DIR" "$CONTEXT_DIR" "${REAL_WORKSPACE_PATHS[@]}" <<'PY'
import re
import sys
from pathlib import Path

root_dir = Path(sys.argv[1])
context_dir = Path(sys.argv[2])
real_paths = set(sys.argv[3:])

root_manifest = (root_dir / "Cargo.toml").read_text()
member_block = re.search(r"(?ms)^members = \[\n(.*?)^\]\n", root_manifest)
if member_block is None:
    raise SystemExit("could not read workspace members from Cargo.toml")
workspace_members = re.findall(r'"([^"]+)"', member_block.group(1))


def package_block(text: str) -> str:
    match = re.search(r"(?ms)^\[package\]\n(.*?)(?:^\[|\Z)", text)
    if match is None:
        raise SystemExit("could not read [package] block from workspace member manifest")
    return match.group(1)


def first_match(block: str, pattern: str):
    match = re.search(pattern, block, re.MULTILINE)
    return match.group(1) if match else None

for member in workspace_members:
    if member in real_paths:
        continue

    original_manifest_path = root_dir / member / "Cargo.toml"
    if not original_manifest_path.exists():
        continue

    package = package_block(original_manifest_path.read_text())
    package_name = first_match(package, r'^name\s*=\s*"([^"]+)"')
    if not package_name:
        raise SystemExit(f"workspace member {member} is missing package.name")

    lines = [
        "[package]",
        f'name = "{package_name}"',
    ]

    version = first_match(package, r'^version\s*=\s*"([^"]+)"')
    if version is not None:
        lines.append(f'version = "{version}"')
    else:
        lines.append("version.workspace = true")

    edition = first_match(package, r'^edition\s*=\s*"([^"]+)"')
    if edition is not None:
        lines.append(f'edition = "{edition}"')
    else:
        lines.append("edition.workspace = true")

    lines.extend(
        [
            "publish = false",
            "",
            "[lints]",
            "workspace = true",
            "",
            "[lib]",
            'path = "src/lib.rs"',
            "",
        ]
    )

    member_dir = context_dir / member
    member_dir.mkdir(parents=True, exist_ok=True)
    (member_dir / "Cargo.toml").write_text("\n".join(lines))
    src_dir = member_dir / "src"
    src_dir.mkdir(parents=True, exist_ok=True)
    (src_dir / "lib.rs").write_text("pub fn placeholder() {}\n")
PY

printf '%s\n' "$CONTEXT_DIR"
