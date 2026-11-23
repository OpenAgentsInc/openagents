#!/usr/bin/env bash
# Firm reason: 5 (thin gcloud storage deployment wrapper publishing apps/openagents-site to production openagents.com).
set -euo pipefail
dir="$(cd "$(dirname "$0")" && pwd)"
OA_SITE_BUCKET="openagentsgemini-openagents-web" exec "$dir/deploy-staging.sh" "$@"
