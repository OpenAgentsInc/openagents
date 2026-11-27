#!/usr/bin/env bash
# Firm reason: 5 (thin gcloud storage deployment wrapper publishing apps/openagents-site to staging.openagents.com).
set -euo pipefail
dir="$(cd "$(dirname "$0")" && pwd)"
dmg="${1:-}"
bucket="${OA_SITE_BUCKET:-openagentsgemini-openagents-web-staging}"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"

echo "==> Deploying apps/openagents-site to gs://${bucket}/"

# Upload immutable/static assets with sensible cache headers
gcloud storage cp "$dir/mark.svg" "gs://${bucket}/mark.svg" \
  --content-type="image/svg+xml" --cache-control="public, max-age=3600"
gcloud storage cp "$dir/favicon.svg" "gs://${bucket}/favicon.svg" \
  --content-type="image/svg+xml" --cache-control="public, max-age=3600"
if [[ -f "$dir/app-preview.webp" ]]; then
  gcloud storage cp "$dir/app-preview.webp" "gs://${bucket}/app-preview.webp" \
    --content-type="image/webp" --cache-control="public, max-age=3600"
fi
if [[ -f "$dir/app-preview.jpg" ]]; then
  gcloud storage cp "$dir/app-preview.jpg" "gs://${bucket}/app-preview.jpg" \
    --content-type="image/jpeg" --cache-control="public, max-age=3600"
fi
if [[ -f "$dir/og/home.jpg" ]]; then
  gcloud storage cp "$dir/og/home.jpg" "gs://${bucket}/og/home.jpg" \
    --content-type="image/jpeg" --cache-control="public, max-age=3600"
fi
if [[ -f "$dir/fonts/CommitMono-Variable.woff2" ]]; then
  gcloud storage cp "$dir/fonts/CommitMono-Variable.woff2" "gs://${bucket}/fonts/CommitMono-Variable.woff2" \
    --content-type="font/woff2" --cache-control="public, max-age=31536000, immutable"
fi
if [[ -n "$dmg" && -f "$dmg" ]]; then
  echo "==> Uploading Mac installer $(basename "$dmg") -> gs://${bucket}/downloads/OpenAgents.dmg"
  gcloud storage cp "$dmg" "gs://${bucket}/downloads/OpenAgents.dmg" \
    --content-type="application/x-apple-diskimage" \
    --content-disposition='attachment; filename="OpenAgents.dmg"' \
    --cache-control="public, max-age=300"
fi

# Upload HTML & Markdown entrypoints last with no-cache so browsers see updates immediately
gcloud storage cp "$dir/index.md" "gs://${bucket}/index.md" \
  --content-type="text/markdown; charset=utf-8" --cache-control="no-cache"
gcloud storage cp "$dir/index.html" "gs://${bucket}/index.html" \
  --content-type="text/html; charset=utf-8" --cache-control="no-cache"

url="https://staging.openagents.com/"
if [[ "$bucket" == "openagentsgemini-openagents-web" ]]; then
  url="https://openagents.com/"
fi
echo "==> Deployed to gs://${bucket}/ (${url})"
