#!/usr/bin/env bash

set -euo pipefail

SCRIPT_NAME=$(basename "$0")
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
REPO_SLUG="${OPENAGENTS_RELEASE_REPO:-OpenAgentsInc/openagents}"

VERSION=""
PUBLISH=false

usage() {
  cat <<EOF
Usage:
  $SCRIPT_NAME --version <X.Y.Z[-rcN]> [--publish]

Description:
  Build standalone Pylon binaries for the current host, package them into a
  GitHub-release-friendly tarball, and optionally publish a prerelease or
  release to GitHub.

Flags:
  --version <version>  Release version label without the leading product name.
  --publish            Create a GitHub release and upload the packaged assets.
  -h, --help           Show this help message.
EOF
}

log() {
  echo "[$SCRIPT_NAME] $*"
}

die() {
  echo "[$SCRIPT_NAME] ERROR: $*" >&2
  exit 1
}

require_command() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Missing required command: $cmd"
}

normalize_version() {
  local value="$1"
  value="${value#v}"
  [[ -n "$value" ]] || die "Version must not be empty"
  echo "$value"
}

host_os() {
  case "$(uname -s)" in
    Darwin) echo "darwin" ;;
    Linux) echo "linux" ;;
    *) die "Unsupported host OS: $(uname -s)" ;;
  esac
}

host_arch() {
  case "$(uname -m)" in
    arm64 | aarch64) echo "arm64" ;;
    x86_64 | amd64) echo "x86_64" ;;
    *) die "Unsupported host arch: $(uname -m)" ;;
  esac
}

ensure_clean_git() {
  [[ -z "$(git status --porcelain)" ]] || die "Git worktree must be clean before cutting a release"
}

build_binaries() {
  cargo build --release -p pylon -p pylon-tui
}

write_readme() {
  local path="$1"
  local archive_dir="$2"
  cat >"$path" <<EOF
Pylon standalone binaries
Version: ${VERSION}
Platform: $(host_os)-$(host_arch)

This archive contains:
- pylon: the terminal UI launcher plus headless provider CLI
- pylon-tui: the terminal UI shell

Quick start:
  cd ${archive_dir}
  ./pylon
  ./pylon init
  ./pylon status --json
  ./pylon inventory --json
  ./pylon config show
  # Sellable Gemma supply still requires a local runtime at 127.0.0.1:11434.
  # On macOS the shortest path today is:
  #   brew install ollama
  #   brew services start ollama
  #   ollama pull gemma4:e4b
  ./pylon gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3

Important:
- These binaries run without a Rust toolchain.
- Pylon keeps its local state under ~/.openagents/pylon by default.
- Curated GGUF downloads are optional local cache only; they do not make the sellable lane ready by themselves.
- First-run diagnostics persist to ~/.openagents/pylon/diagnostics/gemma/latest.json.
- The retained Gemma benchmark path still shells into a sibling Psionic checkout.
  Set OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic when you need that lane.
- Source builds remain the fallback for unsupported platforms or when you need
  to modify the code.
EOF
}

write_release_notes() {
  local path="$1"
  local archive_name="$2"
  local sha_name="$3"
  cat >"$path" <<EOF
Pylon binary prerelease ${VERSION}

Assets:
- ${archive_name}: standalone \`pylon\` and \`pylon-tui\` binaries for $(host_os)-$(host_arch)
- ${sha_name}: SHA-256 checksum for the archive

Quick start:
\`\`\`bash
tar -xzf ${archive_name}
cd pylon-v${VERSION}-$(host_os)-$(host_arch)
./pylon --help
./pylon init
./pylon status --json
./pylon inventory --json
./pylon config show
# Sellable Gemma supply still requires a local runtime at 127.0.0.1:11434.
# On macOS the shortest path today is:
#   brew install ollama
#   brew services start ollama
#   ollama pull gemma4:e4b
./pylon gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3
\`\`\`

Notes:
- This release is unsigned and not notarized.
- The current standalone sellable lane is \`psionic.local.inference.gemma.single_node\`.
- Curated GGUF downloads are optional local cache only; they do not make the sellable lane ready by themselves.
- First-run diagnostics persist to \`~/.openagents/pylon/diagnostics/gemma/latest.json\`.
- Source builds are still the fallback for unsupported platforms or local development.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || die "--version requires a value"
      VERSION=$(normalize_version "$2")
      shift 2
      ;;
    --publish)
      PUBLISH=true
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      die "Unknown argument: $1"
      ;;
  esac
done

[[ -n "$VERSION" ]] || die "--version is required"

require_command cargo
require_command git
require_command tar
require_command shasum

cd "$REPO_ROOT"
ensure_clean_git

if [[ "$PUBLISH" == true ]]; then
  require_command gh
fi

TAG="pylon-v${VERSION}"
PLATFORM="$(host_os)-$(host_arch)"
ARCHIVE_DIR="pylon-v${VERSION}-${PLATFORM}"
OUTPUT_DIR="${REPO_ROOT}/target/pylon-release"
STAGE_ROOT="${OUTPUT_DIR}/stage"
STAGE_DIR="${STAGE_ROOT}/${ARCHIVE_DIR}"
ARCHIVE_PATH="${OUTPUT_DIR}/${ARCHIVE_DIR}.tar.gz"
SHA_PATH="${OUTPUT_DIR}/${ARCHIVE_DIR}.tar.gz.sha256"

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR" "$OUTPUT_DIR"

log "Building standalone Pylon binaries"
build_binaries

install -m 0755 "${REPO_ROOT}/target/release/pylon" "${STAGE_DIR}/pylon"
install -m 0755 "${REPO_ROOT}/target/release/pylon-tui" "${STAGE_DIR}/pylon-tui"
write_readme "${STAGE_DIR}/README.txt" "$ARCHIVE_DIR"

rm -f "$ARCHIVE_PATH" "$SHA_PATH"
tar -C "$STAGE_ROOT" -czf "$ARCHIVE_PATH" "$ARCHIVE_DIR"
(cd "$OUTPUT_DIR" && shasum -a 256 "$(basename "$ARCHIVE_PATH")" >"$(basename "$SHA_PATH")")

log "Wrote archive: $ARCHIVE_PATH"
log "Wrote checksum: $SHA_PATH"

if [[ "$PUBLISH" == true ]]; then
  gh release view "$TAG" --repo "$REPO_SLUG" >/dev/null 2>&1 && die "Release already exists: $TAG"

  NOTES_FILE=$(mktemp)
  trap 'rm -f "$NOTES_FILE"' EXIT
  write_release_notes "$NOTES_FILE" "$(basename "$ARCHIVE_PATH")" "$(basename "$SHA_PATH")"

  RELEASE_FLAGS=()
  if [[ "$VERSION" == *-* ]]; then
    RELEASE_FLAGS+=(--prerelease)
  fi

  log "Publishing GitHub release $TAG"
  gh release create \
    "$TAG" \
    "$ARCHIVE_PATH" \
    "$SHA_PATH" \
    --repo "$REPO_SLUG" \
    --target "$(git rev-parse HEAD)" \
    --title "Pylon v${VERSION}" \
    --notes-file "$NOTES_FILE" \
    "${RELEASE_FLAGS[@]}"

  log "Published release: $TAG"
fi
