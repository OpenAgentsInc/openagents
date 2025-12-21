# Autopilot Systemd Services

This directory contains systemd service and timer files for automating autopilot tasks.

## Available Services

### Baseline Update Timer

Automatically updates autopilot baseline metrics every week.

- **Service**: `autopilot-baseline-update.service`
- **Timer**: `autopilot-baseline-update.timer`
- **Schedule**: Every Monday at 00:00
- **Command**: `cargo autopilot baseline update`

## Installation

Run the installation script:

```bash
cd systemd
./install.sh
```

This will:
1. Copy service files to `~/.config/systemd/user/`
2. Reload systemd
3. Enable and start the baseline update timer

## Manual Installation

If you prefer to install manually:

```bash
# Create user systemd directory
mkdir -p ~/.config/systemd/user

# Copy service files
cp autopilot-baseline-update.service ~/.config/systemd/user/
cp autopilot-baseline-update.timer ~/.config/systemd/user/

# Reload systemd
systemctl --user daemon-reload

# Enable and start the timer
systemctl --user enable autopilot-baseline-update.timer
systemctl --user start autopilot-baseline-update.timer
```

## Managing Services

### Check Timer Status

```bash
systemctl --user status autopilot-baseline-update.timer
```

### List All Timers

```bash
systemctl --user list-timers
```

### View Service Logs

```bash
journalctl --user -u autopilot-baseline-update.service
```

### Stop the Timer

```bash
systemctl --user stop autopilot-baseline-update.timer
```

### Disable the Timer

```bash
systemctl --user disable autopilot-baseline-update.timer
```

### Manually Trigger Baseline Update

```bash
systemctl --user start autopilot-baseline-update.service
```

## Customizing the Schedule

The timer uses systemd's `OnCalendar` directive. To change the schedule, edit the timer file:

```ini
[Timer]
# Run every Monday at 00:00
OnCalendar=Mon *-*-* 00:00:00
```

Examples:
- Daily at 3am: `OnCalendar=*-*-* 03:00:00`
- Every Sunday at midnight: `OnCalendar=Sun *-*-* 00:00:00`
- First day of every month: `OnCalendar=*-*-01 00:00:00`

After editing, reload and restart:

```bash
systemctl --user daemon-reload
systemctl --user restart autopilot-baseline-update.timer
```

## What Gets Updated

The baseline update command:
1. Fetches the last 100 autopilot sessions
2. Calculates statistics for each metric dimension:
   - Mean, standard deviation
   - p50, p90, p99 percentiles
   - Sample count
3. Stores updated baselines in `autopilot-metrics.db`

These baselines are used for:
- Regression detection
- Performance tracking
- Anomaly flagging
- Trend analysis

## Troubleshooting

### Timer Not Running

Check if the timer is active:
```bash
systemctl --user is-active autopilot-baseline-update.timer
```

### Service Fails

View logs:
```bash
journalctl --user -u autopilot-baseline-update.service -n 50
```

Common issues:
- Missing `autopilot-metrics.db` - Run autopilot at least once to create it
- Permission errors - Ensure the service has access to the working directory
- Cargo not in PATH - The service runs in a limited environment

### Verifying Baselines

After the timer runs, check baselines:

```bash
cargo autopilot baseline show
```

Or generate a full report:

```bash
cargo autopilot baseline report
```

## Related Documentation

- [Autopilot Metrics](../docs/autopilot/IMPROVEMENT-DIMENSIONS.md)
- [Directive d-004](../.openagents/directives/d-004.md)
