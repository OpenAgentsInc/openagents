#!/usr/bin/env bash
set -euo pipefail

# One-command redeploy for the Effect Native component gallery
# (openagents #8570, epic #8566). Pulls the latest public effect-native@main,
# builds the fully static gallery (`bun run gallery:build` -> dist/gallery),
# stages it next to the Dockerfile, and ships it to the dedicated Cloud Run
# service. Operator-run only — no hosted CI.
#
# Auth: uses whatever gcloud auth is active. For the automation service
# account, prefix with the isolated config, e.g.:
#   CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
#     apps/effect-native-gallery/scripts/deploy-cloudrun.sh
#
# Env overrides:
#   EFFECT_NATIVE_DIR  existing effect-native checkout to build from
#                      (default: fresh shallow clone of main in a temp dir;
#                      an existing checkout is used as-is, dirty or not —
#                      handy for previews, but releases should build main)
#   SERVICE / REGION / PROJECT  Cloud Run target overrides

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE="${SERVICE:-effect-native-gallery}"
REGION="${REGION:-us-central1}"
PROJECT="${PROJECT:-openagentsgemini}"
REPO_URL="https://github.com/OpenAgentsInc/effect-native.git"

command -v bun >/dev/null || { echo "bun is required" >&2; exit 1; }
command -v gcloud >/dev/null || { echo "gcloud is required" >&2; exit 1; }

CLEANUP_DIR=""
if [[ -n "${EFFECT_NATIVE_DIR:-}" ]]; then
  SRC_DIR="$EFFECT_NATIVE_DIR"
  echo "==> Using existing effect-native checkout: $SRC_DIR"
else
  CLEANUP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/effect-native-gallery-src.XXXXXX")"
  SRC_DIR="$CLEANUP_DIR/effect-native"
  echo "==> Cloning latest effect-native@main into $SRC_DIR"
  git clone --depth 1 --branch main "$REPO_URL" "$SRC_DIR"
fi
trap '[[ -n "$CLEANUP_DIR" ]] && rm -rf "$CLEANUP_DIR"' EXIT

echo "==> Building static gallery (bun run gallery:build)"
(cd "$SRC_DIR" && bun install --frozen-lockfile && bun run gallery:build)

GALLERY_DIST="$SRC_DIR/dist/gallery"
[[ -f "$GALLERY_DIST/index.html" ]] || { echo "gallery build missing index.html at $GALLERY_DIST" >&2; exit 1; }
[[ -f "$GALLERY_DIST/app.js" ]] || { echo "gallery build missing app.js at $GALLERY_DIST" >&2; exit 1; }

echo "==> Staging artifact into $APP_DIR/gallery"
rm -rf "$APP_DIR/gallery"
cp -R "$GALLERY_DIST" "$APP_DIR/gallery"

SRC_COMMIT="$(git -C "$SRC_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)"
echo "==> Deploying $SERVICE ($REGION, $PROJECT) from effect-native@$SRC_COMMIT"
gcloud run deploy "$SERVICE" \
  --source "$APP_DIR" \
  --region "$REGION" \
  --project "$PROJECT" \
  --allow-unauthenticated \
  --port 8080 \
  --memory 256Mi \
  --labels "source-repo=effect-native,source-commit=$SRC_COMMIT"

URL="$(gcloud run services describe "$SERVICE" --region "$REGION" --project "$PROJECT" --format='value(status.url)')"

echo "==> Smoke checks against $URL"
fail=0
check() { # path, expected_status, grep_pattern (optional)
  local path="$1" want="$2" pattern="${3:-}"
  local body status
  body="$(curl -sS -w '\n%{http_code}' "$URL$path")"
  status="${body##*$'\n'}"
  body="${body%$'\n'*}"
  if [[ "$status" != "$want" ]]; then
    echo "FAIL $path -> $status (want $want)"; fail=1; return
  fi
  if [[ -n "$pattern" ]] && ! grep -q "$pattern" <<<"$body"; then
    echo "FAIL $path -> $want but missing pattern: $pattern"; fail=1; return
  fi
  echo "ok   $path -> $status${pattern:+ (matched: $pattern)}"
}
check "/" 200 "Effect Native Component Gallery"
check "/stories/button-primary" 200 "Effect Native Component Gallery"
check "/?story=button-primary" 200 "Effect Native Component Gallery"
check "/app.js" 200 ""
check "/definitely-missing-asset.js" 404 ""
[[ "$fail" == 0 ]] || { echo "Smoke checks FAILED" >&2; exit 1; }

echo "==> Done. Live revision:"
gcloud run services describe "$SERVICE" --region "$REGION" --project "$PROJECT" \
  --format='value(status.latestReadyRevisionName)'
echo "    $URL"
