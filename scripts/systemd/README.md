# Systemd Service Files

This directory contains systemd service and timer files for OpenAgents automation.

## Available Services

### Autopilot Metrics (`autopilot-metrics.service` + `autopilot-metrics.timer`)

Generates weekly metrics reports every Monday at 9:00 AM.

**Installation:**

```bash
# Copy files to systemd directory
sudo cp scripts/systemd/autopilot-metrics.service /etc/systemd/system/
sudo cp scripts/systemd/autopilot-metrics.timer /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable timer (starts on boot)
sudo systemctl enable autopilot-metrics.timer

# Start timer immediately
sudo systemctl start autopilot-metrics.timer
```

**Management:**

```bash
# Check timer status
sudo systemctl status autopilot-metrics.timer

# List all timers
systemctl list-timers --all

# Check when next run is scheduled
systemctl list-timers autopilot-metrics.timer

# View service logs
journalctl -u autopilot-metrics.service

# Follow logs in real-time
journalctl -u autopilot-metrics.service -f

# Manually trigger a run (for testing)
sudo systemctl start autopilot-metrics.service
```

**Configuration:**

Edit `/etc/systemd/system/autopilot-metrics.service` to customize:

- `User`: Change the user that runs the script
- `WorkingDirectory`: Change the project directory
- `Environment`: Set custom paths or webhook URLs
- `OnCalendar`: Change schedule (see `man systemd.time`)

After editing:

```bash
sudo systemctl daemon-reload
sudo systemctl restart autopilot-metrics.timer
```

**Schedule Syntax:**

```
OnCalendar=Mon *-*-* 09:00:00     # Every Monday at 9 AM
OnCalendar=*-*-* 00:00:00         # Daily at midnight
OnCalendar=weekly                  # Weekly (Sunday at midnight)
OnCalendar=*-*-01 09:00:00        # First of every month at 9 AM
```

See `man systemd.time` for full syntax documentation.

**Troubleshooting:**

If timer doesn't run:

1. Check timer is active: `systemctl is-active autopilot-metrics.timer`
2. Check service status: `systemctl status autopilot-metrics.service`
3. Check logs: `journalctl -u autopilot-metrics.service --since today`
4. Verify paths in service file are correct
5. Test script manually: `./scripts/weekly-metrics-report.sh`

Common issues:

- **Permission denied**: Check `User` field matches file owner
- **Command not found**: Add Cargo bin to `PATH` in service file
- **Database not found**: Set `AUTOPILOT_DB` environment variable
- **Timer not triggering**: Check `systemctl list-timers` for next run

## Alternative: Cron Setup

If you prefer cron over systemd timers:

```bash
# Edit crontab
crontab -e

# Add line (runs every Monday at 9 AM)
0 9 * * MON cd /home/christopherdavid/code/openagents && ./scripts/weekly-metrics-report.sh
```

## Future Services

Additional services that could be added:

- **Autopilot Daemon**: Background autopilot supervision
- **Metrics Dashboard**: Web server for metrics visualization
- **Database Backup**: Automated daily backups of autopilot.db
- **Log Rotation**: Automated cleanup of old trajectory logs
- **Health Checks**: Periodic system health monitoring

## Security Considerations

The service files include basic security hardening:

- `PrivateTmp=true`: Isolated /tmp directory
- `NoNewPrivileges=true`: Prevents privilege escalation
- `StandardOutput=journal`: Logs to systemd journal (not world-readable)

For production use, consider additional hardening:

- `ProtectSystem=strict`: Read-only system directories
- `ProtectHome=read-only`: Limit home directory access
- `PrivateDevices=true`: No device access
- `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`: Limit network protocols

See `man systemd.exec` for full hardening options.

## Related Documentation

- [Weekly Metrics Reports](../docs/metrics/README.md)
- [d-004: Continual Constant Improvement](.openagents/directives/d-004.md)
- [Autopilot Daemon Guide](../docs/autopilot/DAEMON.md)
