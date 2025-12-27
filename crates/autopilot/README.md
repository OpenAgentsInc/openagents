# Autopilot

Core logic for Autopilot.

Idea: You run Autopilot via `cargo autopilot` and everything else is taken care of.

## Preflight

It first checks:

- Reads everything in your .openagents folder
- If you're authed with claude/codex/opencode etc.
- The current folder
- Does it have a git repo it can access
- Any projects / how are you doing issue tracking
- What you use for inference: any local models, cloud API key, swarm providers
- Your usage

This puts a config file in your ~/.openagents/folders/filepath-smoething/ (not committed to git) with all that info
