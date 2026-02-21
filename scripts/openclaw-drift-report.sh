#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
INTAKE_DIR="$ROOT_DIR/docs/plans/active/openclaw-intake"
REPORT_PATH="$ROOT_DIR/docs/plans/active/openclaw-drift-report.md"
FIXTURE_PATH="$ROOT_DIR/apps/runtime/test/fixtures/openclaw/tool_policy_parity_cases.json"

OPENCLAW_UPSTREAM_URL="${OPENCLAW_UPSTREAM_URL:-https://github.com/openclaw/openclaw.git}"
OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE="${OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE:-0}"
NOW_UTC="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
TODAY_UTC="$(date -u +"%Y-%m-%d")"

if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) is required" >&2
  exit 1
fi

if ! UPSTREAM_HEAD="$(git ls-remote "$OPENCLAW_UPSTREAM_URL" HEAD | awk 'NR==1 {print $1}')"; then
  echo "Failed to resolve upstream HEAD from $OPENCLAW_UPSTREAM_URL" >&2
  exit 1
fi

if [[ -z "$UPSTREAM_HEAD" ]]; then
  echo "Could not determine upstream HEAD for $OPENCLAW_UPSTREAM_URL" >&2
  exit 1
fi

rows=()
row_records=()

append_row() {
  local capability="$1"
  local pinned_sha="$2"
  local drift_type="$3"
  local recommendation="$4"

  rows+=("| ${capability} | ${pinned_sha} | ${UPSTREAM_HEAD} | ${drift_type} | ${recommendation} |")
  row_records+=("${capability}|${pinned_sha}|${drift_type}|${recommendation}")
}

classify_drift() {
  local sha="$1"

  if [[ -z "$sha" || "$sha" == "pending" || "$sha" == "TBD" || "$sha" == "to-be-pinned" ]]; then
    echo "missing_pin"
    return 0
  fi

  if [[ "$sha" == "$UPSTREAM_HEAD" ]]; then
    echo "in_sync"
    return 0
  fi

  if [[ "$sha" =~ ^[0-9a-f]{40}$ ]]; then
    echo "upstream_head_mismatch"
    return 0
  fi

  echo "invalid_sha"
}

recommendation_for() {
  local drift_type="$1"

  case "$drift_type" in
    in_sync)
      echo "No action. Continue periodic verification."
      ;;
    missing_pin)
      echo "Pin exact upstream SHA in intake record and add/refresh parity fixture coverage."
      ;;
    invalid_sha)
      echo "Replace non-SHA value with exact 40-char upstream commit SHA and re-run report."
      ;;
    upstream_head_mismatch)
      echo "Review upstream diff vs pinned SHA, refresh fixtures/parity tests, and open re-ingestion issue if behavior changed."
      ;;
    *)
      echo "Investigate drift classification and update intake metadata."
      ;;
  esac
}

if [[ -d "$INTAKE_DIR" ]]; then
  while IFS= read -r file; do
    capability_id="$(rg --max-count 1 "^- Capability ID:" "$file" | sed -E 's/^.*Capability ID:[[:space:]]*`?([^`]+)`?.*$/\1/' || true)"
    pinned_sha_raw="$(rg --max-count 1 "^- Upstream commit SHA:" "$file" | sed -E 's/^.*Upstream commit SHA:[[:space:]]*`?([^`]+)`?.*$/\1/' || true)"
    pinned_sha="$(echo "$pinned_sha_raw" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]')"

    # Normalize common placeholders.
    if echo "$pinned_sha" | rg -q "pinnedduringimplementation|pending|todo|tbd|unknown"; then
      pinned_sha="pending"
    fi

    if [[ -z "$capability_id" ]]; then
      capability_id="$(basename "$file" .md)"
    fi

    drift_type="$(classify_drift "$pinned_sha")"
    recommendation="$(recommendation_for "$drift_type")"
    append_row "$capability_id" "${pinned_sha:-pending}" "$drift_type" "$recommendation"
  done < <(find "$INTAKE_DIR" -maxdepth 1 -type f -name '*.md' ! -name 'TEMPLATE.md' | sort)
fi

if [[ -f "$FIXTURE_PATH" ]]; then
  fixture_sha_raw="$(rg --max-count 1 '"commit":' "$FIXTURE_PATH" | sed -E 's/^.*"commit":[[:space:]]*"([^"]*)".*$/\1/' || true)"
  fixture_sha="$(echo "$fixture_sha_raw" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]')"
  fixture_drift_type="$(classify_drift "$fixture_sha")"
  fixture_recommendation="$(recommendation_for "$fixture_drift_type")"

  append_row "openclaw-tool-policy-fixtures" "${fixture_sha:-pending}" "$fixture_drift_type" "$fixture_recommendation"
fi

count_in_sync=0
count_mismatch=0
count_missing=0
count_invalid=0
actionable_count=0
actionable_records=()

for record in "${row_records[@]}"; do
  IFS='|' read -r capability pinned_sha drift_type recommendation <<< "$record"

  case "$drift_type" in
    in_sync)
      count_in_sync=$((count_in_sync + 1))
      ;;
    upstream_head_mismatch)
      count_mismatch=$((count_mismatch + 1))
      actionable_count=$((actionable_count + 1))
      actionable_records+=("$record")
      ;;
    missing_pin)
      count_missing=$((count_missing + 1))
      actionable_count=$((actionable_count + 1))
      actionable_records+=("$record")
      ;;
    invalid_sha)
      count_invalid=$((count_invalid + 1))
      actionable_count=$((actionable_count + 1))
      actionable_records+=("$record")
      ;;
    *)
      actionable_count=$((actionable_count + 1))
      actionable_records+=("$record")
      ;;
  esac
done

{
  echo "# OpenClaw Drift Report"
  echo
  echo "Date: ${TODAY_UTC}"
  echo "Generated: ${NOW_UTC}"
  echo "Upstream: ${OPENCLAW_UPSTREAM_URL}"
  echo "Upstream HEAD: ${UPSTREAM_HEAD}"
  echo
  echo "## Classification"
  echo
  echo "- \`in_sync\`: pinned SHA matches upstream HEAD"
  echo "- \`upstream_head_mismatch\`: pinned SHA differs from upstream HEAD"
  echo "- \`missing_pin\`: intake/fixture has no exact pinned SHA"
  echo "- \`invalid_sha\`: value is not a valid 40-char SHA"
  echo
  echo "## Drift Summary"
  echo
  echo "- In sync: ${count_in_sync}"
  echo "- Upstream head mismatch: ${count_mismatch}"
  echo "- Missing pin: ${count_missing}"
  echo "- Invalid SHA: ${count_invalid}"
  echo "- Actionable rows: ${actionable_count}"
  echo
  echo "## Capability Drift Table"
  echo
  echo "| Capability | Pinned SHA | Upstream HEAD | Drift Type | Recommended Action |"
  echo "|---|---|---|---|---|"

  if [[ "${#rows[@]}" -eq 0 ]]; then
    echo "| (none) | - | ${UPSTREAM_HEAD} | missing_pin | Add intake records in docs/plans/active/openclaw-intake/ |"
  else
    for row in "${rows[@]}"; do
      echo "$row"
    done
  fi

  echo
  echo "## Next Step Rule"
  echo
  echo "For any \`upstream_head_mismatch\` or \`missing_pin\` row, open/update an ingestion issue that includes:"
  echo "1. Diff scope summary (upstream vs pinned SHA)"
  echo "2. Fixture/parity impact"
  echo "3. Port/adapt/adopt decision"
  echo "4. Rollout risk and test updates"

  if [[ "${#actionable_records[@]}" -gt 0 ]]; then
    echo
    echo "## Actionable Follow-ups"
    echo
    for record in "${actionable_records[@]}"; do
      IFS='|' read -r capability pinned_sha drift_type recommendation <<< "$record"

      echo "- Capability: \`${capability}\`"
      echo "  - Drift type: \`${drift_type}\`"
      echo "  - Pinned SHA: \`${pinned_sha}\`"
      echo "  - Action: ${recommendation}"
      echo "  - Suggested issue command:"
      echo "    \`gh issue create --title \"[OpenClaw Drift] ${capability} (${drift_type})\" --label planning --body \"Drift detected by scripts/openclaw-drift-report.sh on ${TODAY_UTC}.\\n\\nPinned SHA: ${pinned_sha}\\nUpstream HEAD: ${UPSTREAM_HEAD}\\n\\nAction: ${recommendation}\" \`"
    done
  fi
} > "$REPORT_PATH"

echo "Wrote drift report: $REPORT_PATH"
echo "Actionable drift rows: ${actionable_count}"

if [[ "$OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE" == "1" && "$actionable_count" -gt 0 ]]; then
  echo "Failing due to actionable drift rows (OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1)." >&2
  exit 2
fi
