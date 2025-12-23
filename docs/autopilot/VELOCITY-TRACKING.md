# Velocity Tracking

Velocity tracking monitors the rate of improvement in autopilot's performance metrics over time. This document explains how to use the velocity tracking feature to understand and celebrate autopilot improvements.

## What is Velocity?

Velocity is a measure of how quickly autopilot is improving. It's calculated as a score from -1.0 to 1.0:
- **Positive scores** (0 to 1.0): Metrics are improving
- **Zero score**: Metrics are stable
- **Negative scores** (-1.0 to 0): Metrics are degrading

The velocity score is computed by comparing key metrics between time periods and counting how many are improving vs degrading.

## Usage

### Basic Usage

Show velocity for the current week:

```bash
cargo autopilot metrics velocity
```

### Custom Time Periods

Analyze different time periods:

```bash
# Last 7 days
cargo autopilot metrics velocity --period 7d

# Last 30 days
cargo autopilot metrics velocity --period 30d

# This week
cargo autopilot metrics velocity --period this-week

# Last week
cargo autopilot metrics velocity --period last-week
```

### Historical Snapshots

View more historical velocity snapshots:

```bash
# Show last 20 snapshots
cargo autopilot metrics velocity --limit 20
```

### Custom Thresholds

Adjust celebration and warning thresholds:

```bash
# More sensitive celebration (lower threshold)
cargo autopilot metrics velocity --celebrate-threshold 0.3

# Custom progress threshold
cargo autopilot metrics velocity --progress-threshold 0.15

# Custom warning threshold
cargo autopilot metrics velocity --warning-threshold -0.2
```

## Output

The velocity command displays:

1. **Current Period**: The time period being analyzed
2. **Velocity Score**: Overall improvement score (-1.0 to 1.0)
3. **Issues Completed**: Number of issues completed in the period
4. **Metric Counts**:
   - Improving metrics (green)
   - Stable metrics
   - Degrading metrics (red)
5. **Celebration/Warning Messages**: Based on velocity score
6. **Key Metrics**: Individual metric changes with direction indicators
7. **Historical Velocity**: Recent velocity snapshots for trend analysis

### Example Output

```
================================================================================
🚀 Improvement Velocity
================================================================================

📊 Current Period: This Week
  Velocity Score:    0.65 (-1.0 to 1.0)
  Issues Completed:  12
  Improving Metrics: 5
  Stable Metrics:    2
  Degrading Metrics: 1

🎉 CELEBRATION: Great work! Autopilot is significantly improving!
  5 metrics are improving, showing strong upward momentum!

🔑 Key Metrics:
  tool_error_rate       📈   -15.3%
  completion_rate       📈    22.1%
  cost_per_issue        📈   -8.5%
  duration_per_issue    ➡️    -2.1%

📈 Historical Velocity:
  2025-12-23 14:30 | This Week | Score: 0.65
  2025-12-22 14:30 | This Week | Score: 0.43
  2025-12-21 14:30 | This Week | Score: 0.28
```

## Celebration Messages

The velocity command provides feedback based on the velocity score:

### 🎉 Celebration (score > 0.5 by default)
- Displayed when autopilot is significantly improving
- Indicates strong upward momentum
- Great time to review what's working well

### ✨ Progress (score > 0.2 by default)
- Displayed when autopilot is making steady progress
- Positive improvements detected
- Keep up the good work!

### ⚠️ Warning (score < -0.3 by default)
- Displayed when metrics are degrading
- Suggests investigating recent changes
- Time to run diagnostics and review updates

### 🔇 Neutral (between thresholds)
- No special message
- Metrics are relatively stable
- Normal operational state

## Key Metrics Tracked

The velocity calculation focuses on these critical metrics:

1. **tool_error_rate**: Percentage of tool calls that fail
2. **completion_rate**: Percentage of claimed issues completed
3. **cost_per_issue**: Average cost (in USD) per completed issue
4. **duration_per_issue**: Average time (in seconds) per completed issue

These metrics are selected because they directly measure autopilot's effectiveness and efficiency.

## Velocity Snapshots

Each time you run the velocity command, a snapshot is stored in the database with:
- Timestamp
- Period analyzed
- Velocity score
- Metric counts (improving/stable/degrading)
- Issues completed
- Individual metric trends

This historical data allows you to:
- Track improvement trends over time
- Compare performance across periods
- Identify regressions quickly
- Celebrate sustained improvements

## Best Practices

### 1. Regular Monitoring

Check velocity weekly to stay aware of trends:

```bash
# Weekly check
cargo autopilot metrics velocity --period this-week
```

### 2. Compare Periods

Use different periods to understand longer-term trends:

```bash
# Monthly view
cargo autopilot metrics velocity --period 30d --limit 4
```

### 3. Investigate Degradations

When velocity is negative:
1. Review recent code changes
2. Check for new failure patterns
3. Run the analyze command for detailed breakdown
4. Consider reverting problematic changes

### 4. Celebrate Wins

When velocity is strongly positive:
1. Document what improved in LEARNINGS.md
2. Share success with the team
3. Consider if improvements can be applied elsewhere

### 5. Customize Thresholds

Adjust thresholds based on your context:
- Stricter projects: Lower thresholds
- Experimental phases: Higher tolerance for degradation
- Production systems: Very low tolerance

## Integration with Other Tools

### With Analyze Command

For detailed breakdowns after seeing velocity trends:

```bash
# Check velocity
cargo autopilot metrics velocity

# If concerning, analyze in detail
cargo autopilot metrics analyze --period this-week
```

### With Metrics Dashboard

The velocity data feeds into the metrics dashboard for visual tracking.

### With Baseline Management

Velocity helps identify when to update baselines:

```bash
# After sustained improvement
cargo autopilot metrics baseline update
```

## Troubleshooting

### "No sessions found"

- No autopilot sessions in the selected period
- Try a different time period or run some autopilot sessions first

### Score always 0.0

- Not enough historical data to compare
- Metrics haven't changed significantly
- Try a longer time period

### Unexpected negative velocity

1. Check recent changes with `git log`
2. Run `cargo autopilot metrics analyze` for details
3. Review the detailed metrics to identify regressions
4. Check for infrastructure changes (new dependencies, environment changes)

## Related Documentation

- [IMPROVEMENT-DIMENSIONS.md](./IMPROVEMENT-DIMENSIONS.md) - All tracked metrics
- [LEARNINGS.md](./LEARNINGS.md) - Applied improvements
- [README.md](./README.md) - Autopilot overview
