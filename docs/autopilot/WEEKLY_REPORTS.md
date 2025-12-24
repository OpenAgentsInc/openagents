# Automated Weekly Trend Reports

Automatically generate weekly summary reports showing metric trends and improvements.

## Overview

The weekly report feature compares the current week to the previous week for all key metrics, highlighting improvements, regressions, and providing actionable recommendations.

Reports include:
- Executive summary (improving/stable/degrading metrics count)
- Detailed metric trends with percent change
- Regression detection with severity levels
- Specific recommendations for detected issues

## Manual Report Generation

Generate a report on-demand:

```bash
# Using the binary
openagents autopilot metrics report

# Custom output directory
openagents autopilot metrics report -o /path/to/reports

# Custom metrics database
openagents autopilot metrics report --metrics-db /path/to/metrics.db
```

## Automated Report Generation

### Option 1: Systemd Timer (Recommended for Linux)

Systemd timers provide reliable scheduling with logging and error handling.

**Installation:**

```bash
# Copy timer and service files to systemd user directory
mkdir -p ~/.config/systemd/user
cp docs/autopilot/weekly-report.service ~/.config/systemd/user/
cp docs/autopilot/weekly-report.timer ~/.config/systemd/user/

# Edit the service file to use your actual paths
nano ~/.config/systemd/user/weekly-report.service

# Enable and start the timer
systemctl --user enable weekly-report.timer
systemctl --user start weekly-report.timer

# Check status
systemctl --user status weekly-report.timer
systemctl --user list-timers
```

**View logs:**

```bash
# View recent report generation logs
journalctl --user -u weekly-report.service -n 50

# Follow logs in real-time
journalctl --user -u weekly-report.service -f
```

**Customize schedule:**

Edit `~/.config/systemd/user/weekly-report.timer` and change the `OnCalendar` line:

```ini
# Every Monday at 9:00 AM (default)
OnCalendar=Mon *-*-* 09:00:00

# Every Sunday at midnight
OnCalendar=Sun *-*-* 00:00:00

# First day of every month at 8:00 AM
OnCalendar=*-*-01 08:00:00

# Every day at 6:00 PM
OnCalendar=*-*-* 18:00:00
```

After editing, reload systemd:

```bash
systemctl --user daemon-reload
systemctl --user restart weekly-report.timer
```

### Option 2: Cron (with helper script)

For systems without systemd or if you prefer cron.

**Installation using the helper script:**

The `scripts/weekly-metrics-report.sh` script provides a complete weekly report generation workflow including:
- Metrics report generation with automatic output directory creation
- JSON export for programmatic analysis
- Weekly summary with key metrics
- Anomaly and regression detection
- Historical report cleanup (keeps last 12 weeks)
- Optional webhook notifications

```bash
# Edit your crontab
crontab -e

# Add the following line (adjust path to match your setup):
# Every Monday at 9:00 AM
0 9 * * 1 /home/user/code/openagents/scripts/weekly-metrics-report.sh

# Or with custom environment variables:
0 9 * * 1 METRICS_DB=/custom/path/autopilot-metrics.db REPORTS_OUTPUT=/custom/reports /home/user/code/openagents/scripts/weekly-metrics-report.sh
```

**Direct command (without script):**

```bash
# Add the following line (adjust path to match your setup):
0 9 * * 1 cd /home/user/code/openagents && ./target/release/openagents autopilot metrics report

# Or with email notification:
0 9 * * 1 cd /home/user/code/openagents && ./target/release/openagents autopilot metrics report && echo "Weekly report generated at $(date)" | mail -s "Autopilot Weekly Report" you@example.com
```

**Environment variables for the script:**

- `METRICS_DB` - Path to metrics database (default: `./autopilot-metrics.db`)
- `REPORTS_OUTPUT` - Output directory (default: `./docs/autopilot/reports/`)
- `NOTIFY_WEBHOOK` - Optional webhook URL for Slack/Discord notifications

**Cron schedule format:**

```
┌───────────── minute (0 - 59)
│ ┌───────────── hour (0 - 23)
│ │ ┌───────────── day of month (1 - 31)
│ │ │ ┌───────────── month (1 - 12)
│ │ │ │ ┌───────────── day of week (0 - 6) (Sunday=0)
│ │ │ │ │
│ │ │ │ │
* * * * * command to execute
```

**Examples:**

```bash
# Every Monday at 9:00 AM
0 9 * * 1 /path/to/command

# Every Sunday at midnight
0 0 * * 0 /path/to/command

# First day of every month at 8:00 AM
0 8 1 * * /path/to/command

# Every Friday at 5:00 PM
0 17 * * 5 /path/to/command
```

## Report Output

Reports are saved to `docs/autopilot/reports/YYYY-WWW.md` by default (e.g., `2025-W51.md`).

### Example Report Structure

```markdown
# Autopilot Weekly Report - 2025 Week 51

Generated: 2025-12-23 09:00:00 UTC

## Executive Summary

- 5 metrics improving
- 2 metrics stable
- 1 metrics degrading

⚠️  **1 regressions detected** - immediate attention recommended

## Metric Trends

| Metric | Direction | Change | This Week | Last Week |
|--------|-----------|--------|-----------|-----------|
| cost_per_issue | ✅ | -15.3% | 0.42 | 0.49 |
| tool_error_rate | ⚠️ | +8.2% | 0.12 | 0.11 |
| completion_rate | ✅ | +5.1% | 0.95 | 0.90 |
| duration_per_issue | ➖ | +1.2% | 342.50 | 338.40 |

## ⚠️ Regressions Detected

### 🟡 WARNING - tool_error_rate

- **Baseline**: 0.11
- **Current**: 0.12
- **Degradation**: 8.2% worse
- **Statistical significance**: 2.1σ

## Recommendations

Based on this week's data:

- **Tool Error Rate**: Investigate failing tools. Check recent error logs and consider updating tool implementations.
```

## Metrics Tracked

The weekly report analyzes:

- **Performance**: Duration per issue, tool latency
- **Cost**: Cost per issue, token usage efficiency
- **Quality**: Completion rate, error rates
- **Tool Usage**: Error rates by tool, most common failures

## Integration with Issue Tracking

Detected regressions are automatically stored as anomalies in the metrics database. You can create issues from these anomalies using:

```bash
openagents autopilot metrics create-issues
```

This will scan for uninvestigated anomalies and create corresponding issues linked to directive d-004 (Continual Constant Improvement).

## Troubleshooting

**No metrics data available:**
- Ensure autopilot sessions have been run and metrics imported
- Check metrics database exists: `ls -lh autopilot-metrics.db`
- Import metrics manually: `openagents autopilot metrics backfill`

**Timer not running:**
```bash
# Check if timer is active
systemctl --user is-active weekly-report.timer

# View timer details
systemctl --user cat weekly-report.timer

# Check for errors
systemctl --user status weekly-report.service
```

**Cron not executing:**
```bash
# Check cron logs (location varies by system)
grep CRON /var/log/syslog  # Debian/Ubuntu
grep CRON /var/log/cron    # RedHat/CentOS

# Verify crontab is set
crontab -l

# Test the command manually
cd /home/user/code/openagents && ./target/release/openagents autopilot metrics report
```

## See Also

- [Metrics Documentation](./METRICS.md)
- [Autopilot Documentation](../../crates/autopilot/README.md)
- [Directive d-004: Continual Constant Improvement](../directives/d-004.md)
