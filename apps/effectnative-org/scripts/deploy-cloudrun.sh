#!/usr/bin/env bash
set -euo pipefail

# One-command redeploy for https://effectnative.org (openagents #8571,
# epic #8566). Pulls the latest public effect-native@main, builds the fully
# static site (`bun run site:build` -> dist/site, gallery included at
# /components), stages it next to the Dockerfile, and ships it to the
# dedicated Cloud Run service. Operator-run only — no hosted CI.
#
# Auth: uses whatever gcloud auth is active. For the automation service
# account, prefix with the isolated config, e.g.:
#   CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
#     apps/effectnative-org/scripts/deploy-cloudrun.sh
#
# Env overrides:
#   EFFECT_NATIVE_DIR  existing effect-native checkout to build from
#                      (default: fresh shallow clone of main in a temp dir;
#                      an existing checkout is used as-is, dirty or not —
#                      handy for previews, but releases should build main)
#   SERVICE / REGION / PROJECT  Cloud Run target overrides

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE="${SERVICE:-effectnative-org}"
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
  CLEANUP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/effectnative-org-src.XXXXXX")"
  SRC_DIR="$CLEANUP_DIR/effect-native"
  echo "==> Cloning latest effect-native@main into $SRC_DIR"
  git clone --depth 1 --branch main "$REPO_URL" "$SRC_DIR"
fi
trap '[[ -n "$CLEANUP_DIR" ]] && rm -rf "$CLEANUP_DIR"' EXIT

echo "==> Building static site (bun run site:build)"
(cd "$SRC_DIR" && bun install --frozen-lockfile && bun run site:build)

SITE_DIST="$SRC_DIR/dist/site"
[[ -f "$SITE_DIST/index.html" ]] || { echo "site build missing index.html at $SITE_DIST" >&2; exit 1; }
[[ -f "$SITE_DIST/components/index.html" ]] || { echo "site build missing gallery at /components" >&2; exit 1; }

echo "==> Staging artifact into $APP_DIR/site"
rm -rf "$APP_DIR/site"
cp -R "$SITE_DIST" "$APP_DIR/site"

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
check "/" 200 "Effect Native"
check "/docs/" 200 "Effect Native"
check "/components/" 200 ""
check "/sitemap.xml" 200 "effectnative.org"
check "/definitely-not-a-page" 404 ""
[[ "$fail" == 0 ]] || { echo "Smoke checks FAILED" >&2; exit 1; }

echo "==> Done. Live revision:"
gcloud run services describe "$SERVICE" --region "$REGION" --project "$PROJECT" \
  --format='value(status.latestReadyRevisionName)'
echo "    $URL (custom domain: https://effectnative.org once the mapping exists)"
