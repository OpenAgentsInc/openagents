#!/bin/bash
#
# Weekly Autopilot Metrics Report Generator
#
# This script generates a comprehensive autopilot metrics report for the past week
# and saves it to docs/metrics/weekly/ for historical tracking.
#
# Designed to be run as a weekly cron job:
#   0 9 * * MON /path/to/weekly-metrics-report.sh
#
# Environment variables:
#   AUTOPILOT_DB      - Path to autopilot.db (default: ./autopilot.db)
#   METRICS_OUTPUT    - Output directory (default: ./docs/metrics/weekly/)
#   NOTIFY_WEBHOOK    - Optional webhook URL for notifications
#

set -e

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
AUTOPILOT_DB="${AUTOPILOT_DB:-$PROJECT_ROOT/autopilot.db}"
METRICS_OUTPUT="${METRICS_OUTPUT:-$PROJECT_ROOT/docs/metrics/weekly}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
WEEK_START=$(date -d '7 days ago' +%Y-%m-%d)
WEEK_END=$(date +%Y-%m-%d)
REPORT_FILE="$METRICS_OUTPUT/report-$TIMESTAMP.txt"
JSON_FILE="$METRICS_OUTPUT/report-$TIMESTAMP.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Ensure output directory exists
mkdir -p "$METRICS_OUTPUT"

echo -e "${BLUE}=== Weekly Autopilot Metrics Report ===${NC}"
echo -e "${BLUE}Period: $WEEK_START to $WEEK_END${NC}"
echo -e "${BLUE}Database: $AUTOPILOT_DB${NC}"
echo ""

# Check if database exists
if [ ! -f "$AUTOPILOT_DB" ]; then
    echo -e "${RED}Error: Database not found at $AUTOPILOT_DB${NC}"
    exit 1
fi

# Change to project root to run cargo commands
cd "$PROJECT_ROOT"

# Generate the report using the analyze command
echo -e "${BLUE}Generating metrics report...${NC}"

# Run analyze command with compare flag for the past week
if cargo run -p autopilot --bin autopilot -- analyze \
    --compare "$WEEK_START" "$WEEK_END" \
    > "$REPORT_FILE" 2>&1; then
    echo -e "${GREEN}Report generated successfully${NC}"
    echo -e "${GREEN}Text report: $REPORT_FILE${NC}"
else
    echo -e "${YELLOW}Warning: analyze command exited with non-zero status${NC}"
    echo -e "${YELLOW}Report may be incomplete but saved to: $REPORT_FILE${NC}"
fi

# Generate JSON export for programmatic analysis
echo ""
echo -e "${BLUE}Generating JSON export...${NC}"

if cargo run -p autopilot --bin autopilot -- metrics export \
    --format json \
    --since "$WEEK_START" \
    --output "$JSON_FILE" 2>&1; then
    echo -e "${GREEN}JSON export: $JSON_FILE${NC}"
else
    echo -e "${YELLOW}Warning: JSON export failed${NC}"
fi

# Extract key metrics for summary
echo ""
echo -e "${BLUE}=== Weekly Summary ===${NC}"

# Count sessions in the period
SESSION_COUNT=$(sqlite3 "$AUTOPILOT_DB" \
    "SELECT COUNT(*) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")

# Calculate total issues completed
ISSUES_COMPLETED=$(sqlite3 "$AUTOPILOT_DB" \
    "SELECT COALESCE(SUM(issues_completed), 0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")

# Calculate total cost
TOTAL_COST=$(sqlite3 "$AUTOPILOT_DB" \
    "SELECT COALESCE(SUM(cost_usd), 0.0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0.0")

# Calculate average APM
AVG_APM=$(sqlite3 "$AUTOPILOT_DB" \
    "SELECT COALESCE(AVG(apm), 0.0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END' AND apm > 0" \
    2>/dev/null || echo "0.0")

# Calculate tool error rate
TOOL_ERRORS=$(sqlite3 "$AUTOPILOT_DB" \
    "SELECT COALESCE(SUM(tool_errors), 0) FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END'" \
    2>/dev/null || echo "0")
TOOL_CALLS=$(sqlite3 "$AUTOPILOT_DB" \
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
SUMMARY_FILE="$METRICS_OUTPUT/summary-$TIMESTAMP.txt"
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

# Check for anomalies and regressions
echo ""
echo -e "${BLUE}Checking for anomalies...${NC}"

ANOMALY_COUNT=$(sqlite3 "$AUTOPILOT_DB" \
    "SELECT COUNT(*) FROM anomalies WHERE session_id IN (SELECT id FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END')" \
    2>/dev/null || echo "0")

if [ "$ANOMALY_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}Warning: $ANOMALY_COUNT anomalies detected this week${NC}"

    # List anomalies
    sqlite3 "$AUTOPILOT_DB" \
        "SELECT printf('%s: expected=%.2f, actual=%.2f (severity=%s)', dimension, expected_value, actual_value, severity) FROM anomalies WHERE session_id IN (SELECT id FROM sessions WHERE timestamp >= '$WEEK_START' AND timestamp < '$WEEK_END') ORDER BY severity DESC LIMIT 10" \
        2>/dev/null || true
else
    echo -e "${GREEN}No anomalies detected${NC}"
fi

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
find "$METRICS_OUTPUT" -name "report-*.txt" -mtime +90 -delete 2>/dev/null || true
find "$METRICS_OUTPUT" -name "report-*.json" -mtime +90 -delete 2>/dev/null || true
find "$METRICS_OUTPUT" -name "summary-*.txt" -mtime +90 -delete 2>/dev/null || true

REMAINING_REPORTS=$(find "$METRICS_OUTPUT" -name "report-*.txt" | wc -l)
echo -e "${GREEN}Historical reports: $REMAINING_REPORTS${NC}"

echo ""
echo -e "${GREEN}=== Report Generation Complete ===${NC}"
echo -e "${BLUE}View full report: cat $REPORT_FILE${NC}"
echo ""

exit 0
