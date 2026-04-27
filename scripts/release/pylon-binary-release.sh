#!/usr/bin/env bash

set -euo pipefail

SCRIPT_NAME=$(basename "$0")
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
REPO_SLUG="${OPENAGENTS_RELEASE_REPO:-OpenAgentsInc/openagents}"
DEFAULT_PSIONIC_REPO="${REPO_ROOT}/../psionic"
if [[ ! -f "${DEFAULT_PSIONIC_REPO}/Cargo.toml" && -f "${REPO_ROOT}/../../psionic/Cargo.toml" ]]; then
  DEFAULT_PSIONIC_REPO="${REPO_ROOT}/../../psionic"
fi
PSIONIC_REPO="${OPENAGENTS_PSIONIC_REPO:-${DEFAULT_PSIONIC_REPO}}"

VERSION=""
PUBLISH=false

usage() {
  cat <<EOF
Usage:
  $SCRIPT_NAME --version <X.Y.Z[-rcN]> [--publish]

Description:
  Build standalone Pylon binaries for the current host, package them into a
  GitHub-release-friendly tarball, and optionally publish it to GitHub. When
  the target release already exists, --publish appends or replaces this host's
  assets on the existing tag instead of failing.

Flags:
  --version <version>  Release version label without the leading product name.
  --publish            Create or update the GitHub release with the packaged
                       assets for the current host platform.
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

sha256_file() {
  local path="$1"
  local output="$2"

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$path")" && sha256sum "$(basename "$path")" >"$output")
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    (cd "$(dirname "$path")" && shasum -a 256 "$(basename "$path")" >"$output")
    return
  fi

  die "Missing required command: sha256sum or shasum"
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

release_exists() {
  local tag="$1"
  gh release view "$tag" --repo "$REPO_SLUG" >/dev/null 2>&1
}

build_binaries() {
  cargo build --release -p pylon -p pylon-tui
  cargo build --manifest-path "${PSIONIC_REPO}/Cargo.toml" --release -p psionic-train
}

ensure_psionic_runtime_source() {
  [[ -f "${PSIONIC_REPO}/Cargo.toml" ]] || die "Missing Psionic checkout at ${PSIONIC_REPO}; set OPENAGENTS_PSIONIC_REPO"
  [[ -f "${PSIONIC_REPO}/Cargo.lock" ]] || die "Missing Psionic Cargo.lock at ${PSIONIC_REPO}"
  [[ -f "${PSIONIC_REPO}/TRAIN" ]] || die "Missing Psionic TRAIN entrypoint at ${PSIONIC_REPO}"
  [[ -f "${PSIONIC_REPO}/crates/psionic-train/Cargo.toml" ]] || die "Missing Psionic psionic-train Cargo.toml at ${PSIONIC_REPO}"
  [[ -f "${PSIONIC_REPO}/crates/psionic-train/src/main.rs" ]] || die "Missing Psionic psionic-train main.rs at ${PSIONIC_REPO}"
  [[ -f "${PSIONIC_REPO}/crates/psionic-train/src/train_runtime.rs" ]] || die "Missing Psionic psionic-train train_runtime.rs at ${PSIONIC_REPO}"
}

ensure_clean_psionic_git() {
  git -C "$PSIONIC_REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "Psionic repo must be a Git worktree: ${PSIONIC_REPO}"
  [[ -z "$(git -C "$PSIONIC_REPO" status --porcelain)" ]] || die "Psionic worktree must be clean before cutting a Pylon release"
}

psionic_train_binary_name() {
  case "$(host_os)" in
    windows) echo "psionic-train.exe" ;;
    *) echo "psionic-train" ;;
  esac
}

install_psionic_train_runtime_surface() {
  local source_binary="${PSIONIC_REPO}/target/release/$(psionic_train_binary_name)"
  local runtime_root="${STAGE_DIR}/psionic"

  [[ -x "$source_binary" ]] || die "Missing built psionic-train binary at ${source_binary}"

  mkdir -p \
    "${runtime_root}/target/release" \
    "${runtime_root}/crates/psionic-train/src"

  install -m 0755 "$source_binary" "${runtime_root}/target/release/$(psionic_train_binary_name)"
  cp -p "${PSIONIC_REPO}/Cargo.toml" "${runtime_root}/Cargo.toml"
  cp -p "${PSIONIC_REPO}/Cargo.lock" "${runtime_root}/Cargo.lock"
  cp -p "${PSIONIC_REPO}/TRAIN" "${runtime_root}/TRAIN"
  cp -p "${PSIONIC_REPO}/crates/psionic-train/Cargo.toml" "${runtime_root}/crates/psionic-train/Cargo.toml"
  cp -p "${PSIONIC_REPO}/crates/psionic-train/src/main.rs" "${runtime_root}/crates/psionic-train/src/main.rs"
  cp -p "${PSIONIC_REPO}/crates/psionic-train/src/train_runtime.rs" "${runtime_root}/crates/psionic-train/src/train_runtime.rs"
  chmod 0755 "${runtime_root}/TRAIN"
}

write_readme() {
  local path="$1"
  local archive_dir="$2"
  cat >"$path" <<EOF
Pylon standalone binaries
Version: ${VERSION}
Platform: $(host_os)-$(host_arch)

This archive contains:
- pylon: the default user entrypoint plus headless worker/provider CLI
- pylon-tui: the minimal homework-earning terminal dashboard
- psionic/target/release/psionic-train: the packaged machine-training runtime
  used by Pylon for admin-triggered homework/training work

Quick start:
  cd ${archive_dir}
  ./pylon --help
  ./pylon init
  ./pylon status --json
  ./pylon inventory --json
  ./pylon config show
  ./pylon
  # Interactive ./pylon opens the homework dashboard and keeps the worker online.
  # Noninteractive ./pylon and ./pylon --config-path <path> run the worker directly.
  # Gemma diagnostics/downloads are optional and not part of homework onboarding.
  # Run them only when validating local inference separately:
  #   ./pylon gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3

Important:
- These binaries run without a Rust toolchain.
- Pylon keeps its local state under ~/.openagents/pylon by default.
- Keep the dashboard open to stay eligible for admin-triggered homework jobs.
- Curated GGUF downloads are optional local cache only; they do not make the sellable lane ready by themselves.
- The dashboard starts and supervises the earning worker automatically.
- First-run Gemma diagnostics persist to ~/.openagents/pylon/diagnostics/gemma/latest.json only when run explicitly.
- When the node is online and eligible, the long-lived worker loop publishes or refreshes provider presence.
- The packaged psionic-train runtime is enough for the current homework/training
  earning lane. The retained Gemma benchmark path still shells into a full
  sibling Psionic checkout; set OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic
  when you need that lane.
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
./pylon
# Interactive ./pylon opens the homework dashboard and keeps the worker online.
# Noninteractive ./pylon and ./pylon --config-path <path> run the worker directly.
# Gemma diagnostics/downloads are optional and not part of homework onboarding.
# Run them only when validating local inference separately:
#   ./pylon gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3
\`\`\`

Notes:
- This release is unsigned and not notarized.
- The current user-facing paid lane is admin-triggered homework/training work.
- This archive includes a minimal packaged Psionic runtime surface at
  \`./psionic\`, including \`psionic/target/release/psionic-train\`, so the
  default homework worker can advertise training capability without a separate
  sibling checkout.
- Bare interactive \`pylon\` opens the minimal homework dashboard and supervises the worker.
- Noninteractive \`pylon\` remains the direct worker/service path for automation.
- Curated GGUF downloads are optional local cache only; they do not make the sellable lane ready by themselves.
- First-run Gemma diagnostics persist to \`~/.openagents/pylon/diagnostics/gemma/latest.json\` only when run explicitly.
- When the node is online and eligible, the long-lived worker loop publishes or refreshes provider presence.
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
cd "$REPO_ROOT"
ensure_clean_git
ensure_psionic_runtime_source
ensure_clean_psionic_git

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
install_psionic_train_runtime_surface
write_readme "${STAGE_DIR}/README.txt" "$ARCHIVE_DIR"

rm -f "$ARCHIVE_PATH" "$SHA_PATH"
tar -C "$STAGE_ROOT" -czf "$ARCHIVE_PATH" "$ARCHIVE_DIR"
sha256_file "$ARCHIVE_PATH" "$SHA_PATH"

log "Wrote archive: $ARCHIVE_PATH"
log "Wrote checksum: $SHA_PATH"

if [[ "$PUBLISH" == true ]]; then
  NOTES_FILE=$(mktemp)
  trap 'rm -f "$NOTES_FILE"' EXIT
  write_release_notes "$NOTES_FILE" "$(basename "$ARCHIVE_PATH")" "$(basename "$SHA_PATH")"

  if release_exists "$TAG"; then
    log "Uploading assets to existing GitHub release $TAG"
    gh release upload \
      "$TAG" \
      "$ARCHIVE_PATH" \
      "$SHA_PATH" \
      --repo "$REPO_SLUG" \
      --clobber
  else
    declare -a RELEASE_FLAGS=()
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
      "${RELEASE_FLAGS[@]+"${RELEASE_FLAGS[@]}"}"
  fi

  log "Published release: $TAG"
fi
