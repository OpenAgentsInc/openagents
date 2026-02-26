#!/usr/bin/env bash

set -euo pipefail

SCRIPT_NAME=$(basename "$0")
ROOT_CARGO_TOML="Cargo.toml"
APP_CARGO_TOML="apps/autopilot-desktop/Cargo.toml"
LOCKFILE_PATH="Cargo.lock"
APP_NAME="Autopilot"
APP_PACKAGE="autopilot-desktop"

PUBLISH=false
ALLOW_UNSIGNED=false
BUMP_TYPE=""
TARGET_VERSION=""

VERSIONS_UPDATED=false
COMMIT_CREATED=false
TAG_CREATED=false

CARGO_BUNDLE_CMD=(cargo bundle)

usage() {
  cat <<EOF
Usage:
  $SCRIPT_NAME --bump {patch|minor|major} [--publish] [--allow-unsigned]
  $SCRIPT_NAME --version X.Y.Z [--publish] [--allow-unsigned]

Description:
  Runs the macOS release flow for OpenAgents desktop:
  preflight -> version update -> test/build -> bundle DMG -> (optional) sign/notarize -> commit/tag -> (optional) publish.

Flags:
  --bump <type>         Version bump type: patch, minor, or major.
  --version <X.Y.Z>     Explicit target version.
  --publish             Push commit/tag and create a GitHub release with assets.
  --allow-unsigned      Skip macOS signing and notarization steps.
  -h, --help            Show this help message.
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

validate_semver() {
  local version="$1"
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Invalid version '$version' (expected X.Y.Z)"
}

bump_semver() {
  local version="$1"
  local bump="$2"
  local major minor patch

  validate_semver "$version"
  IFS='.' read -r major minor patch <<< "$version"

  case "$bump" in
    patch) patch=$((patch + 1)) ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    major) major=$((major + 1)); minor=0; patch=0 ;;
    *) die "Invalid bump type '$bump' (expected patch|minor|major)" ;;
  esac

  echo "${major}.${minor}.${patch}"
}

read_version_from_section() {
  local file="$1"
  local section_regex="$2"
  local version

  version=$(awk -v section_regex="$section_regex" '
    BEGIN { in_section = 0 }
    $0 ~ "^\\[" section_regex "\\]$" { in_section = 1; next }
    in_section && /^\[/ { in_section = 0 }
    in_section && $0 ~ /^version[[:space:]]*=/ {
      line = $0
      sub(/^[^"]*"/, "", line)
      sub(/".*$/, "", line)
      print line
      exit
    }
  ' "$file")

  [[ -n "$version" ]] || die "Could not read version from section [$section_regex] in $file"
  echo "$version"
}

update_version_in_section() {
  local file="$1"
  local section_regex="$2"
  local new_version="$3"
  local tmp

  tmp=$(mktemp)
  if ! awk -v section_regex="$section_regex" -v new_version="$new_version" '
    BEGIN {
      in_section = 0
      found_section = 0
      updated = 0
    }
    $0 ~ "^\\[" section_regex "\\]$" {
      in_section = 1
      found_section = 1
      print
      next
    }
    in_section && /^\[/ {
      in_section = 0
    }
    in_section && $0 ~ /^version[[:space:]]*=/ {
      sub(/"[^"]*"/, "\"" new_version "\"")
      updated = 1
    }
    { print }
    END {
      if (!found_section || !updated) {
        exit 2
      }
    }
  ' "$file" > "$tmp"; then
    rm -f "$tmp"
    die "Failed to update version in section [$section_regex] in $file"
  fi

  mv "$tmp" "$file"
}

ensure_cargo_bundle() {
  if cargo bundle --version >/dev/null 2>&1; then
    CARGO_BUNDLE_CMD=(cargo bundle)
    return
  fi

  local install_root="${REPO_ROOT}/target/release-tools"
  local bundle_bin="${install_root}/bin/cargo-bundle"

  mkdir -p "$install_root"
  log "Installing cargo-bundle into $install_root"
  cargo install cargo-bundle --locked --root "$install_root"

  [[ -x "$bundle_bin" ]] || die "cargo-bundle install succeeded but binary was not found at $bundle_bin"
  CARGO_BUNDLE_CMD=("$bundle_bin")
}

require_signing_env() {
  local required_envs=(
    MACOS_SIGNING_IDENTITY
    MACOS_TEAM_ID
    APPLE_NOTARIZATION_KEY
    APPLE_NOTARIZATION_KEY_ID
    APPLE_NOTARIZATION_ISSUER_ID
  )
  local var
  for var in "${required_envs[@]}"; do
    [[ -n "${!var:-}" ]] || die "Missing required env var for signed release: $var"
  done
}

cleanup_on_error() {
  local line="$1"
  echo "[$SCRIPT_NAME] ERROR: Failed at line $line" >&2

  if [[ "$VERSIONS_UPDATED" == true && "$COMMIT_CREATED" == false ]]; then
    echo "[$SCRIPT_NAME] Reverting version file changes" >&2
    git checkout -- "$ROOT_CARGO_TOML" "$APP_CARGO_TOML" || true
  fi

  if [[ "$COMMIT_CREATED" == true ]]; then
    echo "[$SCRIPT_NAME] Recovery commands:" >&2
    echo "  git reset --hard HEAD~1" >&2
    if [[ "$TAG_CREATED" == true ]]; then
      echo "  git tag -d v$TARGET_VERSION" >&2
    fi
  fi
}

trap 'cleanup_on_error $LINENO' ERR

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bump)
      [[ $# -ge 2 ]] || die "--bump requires a value"
      BUMP_TYPE="$2"
      shift 2
      ;;
    --version)
      [[ $# -ge 2 ]] || die "--version requires a value"
      TARGET_VERSION="$2"
      shift 2
      ;;
    --publish)
      PUBLISH=true
      shift
      ;;
    --allow-unsigned)
      ALLOW_UNSIGNED=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "Unknown argument: $1"
      ;;
  esac
done

if [[ -n "$BUMP_TYPE" && -n "$TARGET_VERSION" ]]; then
  die "Use either --bump or --version, not both"
fi

if [[ -z "$BUMP_TYPE" && -z "$TARGET_VERSION" ]]; then
  die "You must provide either --bump or --version"
fi

if [[ "$ALLOW_UNSIGNED" == false ]]; then
  require_signing_env
fi

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || true)
[[ -n "$REPO_ROOT" ]] || die "Must run inside a git repository"
cd "$REPO_ROOT"

[[ "$(uname -s)" == "Darwin" ]] || die "This script only supports macOS"

require_command git
require_command cargo
require_command hdiutil
require_command shasum
if [[ "$PUBLISH" == true ]]; then
  require_command gh
fi
if [[ "$ALLOW_UNSIGNED" == false ]]; then
  require_command codesign
  require_command xcrun
fi

[[ -f "$ROOT_CARGO_TOML" ]] || die "Missing $ROOT_CARGO_TOML"
[[ -f "$APP_CARGO_TOML" ]] || die "Missing $APP_CARGO_TOML"

[[ -z "$(git status --porcelain)" ]] || die "Git working tree must be clean"

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
[[ "$CURRENT_BRANCH" == "main" ]] || die "Releases must run from main branch (current: $CURRENT_BRANCH)"

log "Fetching origin/main"
git fetch origin main --tags

LOCAL_HEAD=$(git rev-parse HEAD)
REMOTE_HEAD=$(git rev-parse origin/main)
[[ "$LOCAL_HEAD" == "$REMOTE_HEAD" ]] || die "Local main is not synced with origin/main"

WORKSPACE_VERSION=$(read_version_from_section "$ROOT_CARGO_TOML" "workspace\\.package")
APP_VERSION=$(read_version_from_section "$APP_CARGO_TOML" "package")

[[ "$WORKSPACE_VERSION" == "$APP_VERSION" ]] || die "Version mismatch: workspace=$WORKSPACE_VERSION app=$APP_VERSION"

if [[ -n "$BUMP_TYPE" ]]; then
  TARGET_VERSION=$(bump_semver "$WORKSPACE_VERSION" "$BUMP_TYPE")
else
  validate_semver "$TARGET_VERSION"
fi

if git rev-parse -q --verify "refs/tags/v$TARGET_VERSION" >/dev/null; then
  die "Tag v$TARGET_VERSION already exists"
fi

log "Releasing version $TARGET_VERSION"

update_version_in_section "$ROOT_CARGO_TOML" "workspace\\.package" "$TARGET_VERSION"
update_version_in_section "$APP_CARGO_TOML" "package" "$TARGET_VERSION"
VERSIONS_UPDATED=true

UPDATED_WORKSPACE_VERSION=$(read_version_from_section "$ROOT_CARGO_TOML" "workspace\\.package")
UPDATED_APP_VERSION=$(read_version_from_section "$APP_CARGO_TOML" "package")
[[ "$UPDATED_WORKSPACE_VERSION" == "$TARGET_VERSION" ]] || die "Workspace version update failed"
[[ "$UPDATED_APP_VERSION" == "$TARGET_VERSION" ]] || die "App version update failed"

log "Running ownership boundary check"
./scripts/lint/ownership-boundary-check.sh

log "Running workspace tests"
cargo test --workspace

log "Building release binary"
cargo build --release -p "$APP_PACKAGE"

ensure_cargo_bundle

log "Bundling macOS app"
"${CARGO_BUNDLE_CMD[@]}" --release -p "$APP_PACKAGE" --format osx

APP_PATH="target/release/bundle/osx/${APP_NAME}.app"
[[ -d "$APP_PATH" ]] || die "Bundled app not found at $APP_PATH"

if [[ "$ALLOW_UNSIGNED" == false ]]; then
  log "Code-signing app bundle"
  codesign --force --timestamp --options runtime --sign "$MACOS_SIGNING_IDENTITY" "${APP_PATH}/Contents/MacOS/${APP_PACKAGE}"
  codesign --force --timestamp --options runtime --sign "$MACOS_SIGNING_IDENTITY" "$APP_PATH"
  codesign --verify --deep --strict "$APP_PATH"
fi

mkdir -p target/release
DMG_PATH="target/release/${APP_NAME}-${TARGET_VERSION}.dmg"
CHECKSUM_PATH="${DMG_PATH}.sha256"

DMG_STAGING_DIR=$(mktemp -d)
cp -R "$APP_PATH" "$DMG_STAGING_DIR/"
ln -s /Applications "$DMG_STAGING_DIR/Applications"

log "Creating DMG $DMG_PATH"
hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$DMG_STAGING_DIR" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

rm -rf "$DMG_STAGING_DIR"

if [[ "$ALLOW_UNSIGNED" == false ]]; then
  log "Submitting DMG for notarization"
  NOTARY_KEY_FILE=$(mktemp)
  printf '%s' "$APPLE_NOTARIZATION_KEY" > "$NOTARY_KEY_FILE"
  xcrun notarytool submit "$DMG_PATH" \
    --wait \
    --key "$NOTARY_KEY_FILE" \
    --key-id "$APPLE_NOTARIZATION_KEY_ID" \
    --issuer "$APPLE_NOTARIZATION_ISSUER_ID" \
    --team-id "$MACOS_TEAM_ID"
  rm -f "$NOTARY_KEY_FILE"

  log "Stapling notarization ticket"
  xcrun stapler staple "$DMG_PATH"
fi

log "Writing checksum file $CHECKSUM_PATH"
(
  cd "$(dirname "$DMG_PATH")"
  shasum -a 256 "$(basename "$DMG_PATH")" > "$(basename "$CHECKSUM_PATH")"
)

log "Creating release commit and tag"
FILES_TO_COMMIT=("$ROOT_CARGO_TOML" "$APP_CARGO_TOML")
if ! git diff --quiet -- "$LOCKFILE_PATH"; then
  log "Including Cargo.lock changes in release commit"
  FILES_TO_COMMIT+=("$LOCKFILE_PATH")
fi
git add "${FILES_TO_COMMIT[@]}"
git commit -m "chore(release): v$TARGET_VERSION"
COMMIT_CREATED=true

git tag -a "v$TARGET_VERSION" -m "${APP_NAME} v$TARGET_VERSION"
TAG_CREATED=true

if [[ "$PUBLISH" == true ]]; then
  log "Pushing commit and tag"
  git push origin main
  git push origin "v$TARGET_VERSION"

  log "Creating GitHub release"
  gh release create "v$TARGET_VERSION" \
    "$DMG_PATH" \
    "$CHECKSUM_PATH" \
    --title "${APP_NAME} v$TARGET_VERSION" \
    --notes "${APP_NAME} macOS release v$TARGET_VERSION"
fi

log "Release flow complete"
log "DMG: $DMG_PATH"
log "Checksum: $CHECKSUM_PATH"
