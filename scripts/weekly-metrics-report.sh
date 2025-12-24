#!/bin/bash
#
# Weekly Autopilot Metrics Report Generator
#
# This script generates a comprehensive autopilot metrics report for the past week
# and saves it to docs/autopilot/reports/ for historical tracking.
#
# Designed to be run as a weekly cron job:
#   0 9 * * MON /path/to/weekly-metrics-report.sh
#
# Environment variables:
#   METRICS_DB        - Path to autopilot-metrics.db (default: ./autopilot-metrics.db)
#   REPORTS_OUTPUT    - Output directory (default: ./docs/autopilot/reports/)
#   NOTIFY_WEBHOOK    - Optional webhook URL for notifications
#

set -e

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
METRICS_DB="${METRICS_DB:-$PROJECT_ROOT/autopilot-metrics.db}"
REPORTS_OUTPUT="${REPORTS_OUTPUT:-$PROJECT_ROOT/docs/autopilot/reports}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
WEEK_START=$(date -d '7 days ago' +%Y-%m-%d)
WEEK_END=$(date +%Y-%m-%d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Ensure output directory exists
mkdir -p "$REPORTS_OUTPUT"

echo -e "${BLUE}=== Weekly Autopilot Metrics Report ===${NC}"
echo -e "${BLUE}Period: $WEEK_START to $WEEK_END${NC}"
echo -e "${BLUE}Database: $METRICS_DB${NC}"
echo ""

# Check if database exists
if [ ! -f "$METRICS_DB" ]; then
    echo -e "${RED}Error: Metrics database not found at $METRICS_DB${NC}"
    exit 1
fi

# Change to project root to run cargo commands
cd "$PROJECT_ROOT"

# Generate the weekly report using the new metrics report command
echo -e "${BLUE}Generating weekly trend report...${NC}"

if cargo run -p autopilot --bin autopilot -- metrics report \
    --metrics-db "$METRICS_DB" \
    --output "$REPORTS_OUTPUT"; then
    echo -e "${GREEN}✓ Weekly report generated successfully${NC}"

    # Find the most recently generated report
    LATEST_REPORT=$(ls -t "$REPORTS_OUTPUT"/*.md 2>/dev/null | head -1)
    if [ -n "$LATEST_REPORT" ]; then
        echo -e "${GREEN}Report location: $LATEST_REPORT${NC}"
        REPORT_FILE="$LATEST_REPORT"
    fi
else
    echo -e "${YELLOW}Warning: Report generation exited with non-zero status${NC}"
fi

# Generate JSON export for programmatic analysis
echo ""
echo -e "${BLUE}Generating JSON export...${NC}"

JSON_FILE="$REPORTS_OUTPUT/export-$TIMESTAMP.json"
if cargo run -p autopilot --bin autopilot -- metrics export \
    --db "$METRICS_DB" \
    --period "this-week" \
    --format json \
    --output "$JSON_FILE"; then
    echo -e "${GREEN}JSON export: $JSON_FILE${NC}"
else
    echo -e "${YELLOW}Warning: JSON export failed${NC}"
fi

# Extract key metrics for summary from metrics database
echo ""
echo -e "${BLUE}=== Weekly Summary ===${NC}"

# Count sessions in the period
SESSION_COUNT=$(sqlite3 "$METRICS_DB" \
    "SELECT COUNT(*) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")

# Calculate total issues completed
ISSUES_COMPLETED=$(sqlite3 "$METRICS_DB" \
    "SELECT COALESCE(SUM(issues_completed), 0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")

# Calculate total cost
TOTAL_COST=$(sqlite3 "$METRICS_DB" \
    "SELECT COALESCE(SUM(cost_usd), 0.0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0.0")

# Calculate average APM
AVG_APM=$(sqlite3 "$METRICS_DB" \
    "SELECT COALESCE(AVG(apm), 0.0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END' AND apm > 0" \
    2>/dev/null || echo "0.0")

# Calculate tool error rate
TOOL_ERRORS=$(sqlite3 "$METRICS_DB" \
    "SELECT COALESCE(SUM(tool_errors), 0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")
TOOL_CALLS=$(sqlite3 "$METRICS_DB" \
    "SELECT COALESCE(SUM(tool_calls), 0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")

if [ "$TOOL_CALLS" -gt 0 ]; then
    ERROR_RATE=$(echo "scale=2; ($TOOL_ERRORS * 100.0) / $TOOL_CALLS" | bc)
else
    ERROR_RATE="0.0"
fi

echo -e "${GREEN}Sessions:         $SESSION_COUNT${NC}"
echo -e "${GREEN}Issues Completed: $ISSUES_COMPLETED${NC}"
echo -e "${GREEN}Total Cost:       \$$TOTAL_COST${NC}"
echo -e "${GREEN}Average APM:      $AVG_APM${NC}"
echo -e "${GREEN}Tool Error Rate:  $ERROR_RATE%${NC}"

# Write summary to a separate file
SUMMARY_FILE="$REPORTS_OUTPUT/summary-$TIMESTAMP.txt"
cat > "$SUMMARY_FILE" <<EOF
Weekly Autopilot Metrics Summary
Period: $WEEK_START to $WEEK_END
Generated: $(date)

Sessions:         $SESSION_COUNT
Issues Completed: $ISSUES_COMPLETED
Total Cost:       \$$TOTAL_COST
Average APM:      $AVG_APM
Tool Error Rate:  $ERROR_RATE%

Full Report: $REPORT_FILE
JSON Export: $JSON_FILE
EOF

echo ""
echo -e "${GREEN}Summary saved to: $SUMMARY_FILE${NC}"

# Check for regressions and anomalies
echo ""
echo -e "${BLUE}Checking for regressions and anomalies...${NC}"

# Run metrics analyze command to detect anomalies
cargo run -p autopilot --bin autopilot -- metrics analyze \
    --period "this-week" \
    --anomalies \
    --db "$METRICS_DB" 2>/dev/null | grep -E "(anomaly|regression)" || echo -e "${GREEN}No anomalies or regressions detected${NC}"

# Send notification if webhook configured
if [ -n "$NOTIFY_WEBHOOK" ]; then
    echo ""
    echo -e "${BLUE}Sending notification...${NC}"

    NOTIFICATION_PAYLOAD=$(cat <<EOF
{
  "text": "Weekly Autopilot Metrics Report",
  "blocks": [
    {
      "type": "header",
      "text": {
        "type": "plain_text",
        "text": "📊 Weekly Autopilot Report"
      }
    },
    {
      "type": "section",
      "fields": [
        {
          "type": "mrkdwn",
          "text": "*Period:*\n$WEEK_START to $WEEK_END"
        },
        {
          "type": "mrkdwn",
          "text": "*Sessions:*\n$SESSION_COUNT"
        },
        {
          "type": "mrkdwn",
          "text": "*Issues Completed:*\n$ISSUES_COMPLETED"
        },
        {
          "type": "mrkdwn",
          "text": "*Average APM:*\n$AVG_APM"
        },
        {
          "type": "mrkdwn",
          "text": "*Tool Error Rate:*\n$ERROR_RATE%"
        },
        {
          "type": "mrkdwn",
          "text": "*Anomalies:*\n$ANOMALY_COUNT"
        }
      ]
    }
  ]
}
EOF
)

    if curl -X POST -H 'Content-Type: application/json' \
        -d "$NOTIFICATION_PAYLOAD" \
        "$NOTIFY_WEBHOOK" \
        --silent --show-error; then
        echo -e "${GREEN}Notification sent${NC}"
    else
        echo -e "${YELLOW}Warning: Failed to send notification${NC}"
    fi
fi

# Cleanup old reports (keep last 12 weeks = 3 months)
echo ""
echo -e "${BLUE}Cleaning up old reports...${NC}"

# Find and delete reports older than 90 days
find "$REPORTS_OUTPUT" -name "*.md" -mtime +90 -delete 2>/dev/null || true
find "$REPORTS_OUTPUT" -name "export-*.json" -mtime +90 -delete 2>/dev/null || true
find "$REPORTS_OUTPUT" -name "summary-*.txt" -mtime +90 -delete 2>/dev/null || true

REMAINING_REPORTS=$(find "$REPORTS_OUTPUT" -name "*.md" | wc -l)
echo -e "${GREEN}Historical reports: $REMAINING_REPORTS${NC}"

echo ""
echo -e "${GREEN}=== Report Generation Complete ===${NC}"
if [ -n "$REPORT_FILE" ] && [ -f "$REPORT_FILE" ]; then
    echo -e "${BLUE}View full report: cat $REPORT_FILE${NC}"
fi
echo ""

exit 0
