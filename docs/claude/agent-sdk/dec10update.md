@claudeai: Dec 10, 2025, 1:01pm - https://x.com/claudeai/status/1998830338735485239

We're releasing more upgrades to Claude Code CLI:

- Async subagents
- Instant compact
- Customer session names
- Usage stats

Tasks can now spawn async subagents that move to the background and continue working independently, even if the main agent finishes its task and becomes inactive.

This lets subagents handle long-running tasks, great for monitoring logs or waiting for builds.

Claude now compacts context exponentially faster.

Compacting takes only seconds so you don’t get interrupted.

You can also now rename sessions to make them easier to find & resume later.

Type /rename to give any previous session a custom name.

We’ve also added keyboard shortcuts to the /resume screen. Hit ‘R’ to rename a session or ‘P’ to preview a session.

Lastly, we’ve shipped a new command: /stats

/stats generates a visualization of your daily Claude Code usage and provides data on your sessions, usage streaks, and favorite models.

All features are available in today’s Claude Code build. Run `claude update` for the latest.
