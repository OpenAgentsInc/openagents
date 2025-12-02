# MechaCoder Operations Guide

A guide for humans and future agents on controlling and monitoring the MechaCoder autonomous coding agent.

---

## Where Logs Are

### Per-Run Logs (Most Important)
```
~/code/openagents/docs/logs/YYYYMMDD/HHMMSS-bead-run.md
```
- One file per agent run
- Contains: start time, bead claimed, all tool calls, tool results, final message
- Example: `docs/logs/20251202/093350-bead-run.md`

### System Logs
```
~/code/openagents/logs/mechacoder-stdout.log  # Agent console output
~/code/openagents/logs/mechacoder-stderr.log  # Agent errors
```

### View Latest Log
```bash
cat $(ls -t ~/code/openagents/docs/logs/$(date +%Y%m%d)/*.md | head -1)
```

### Watch Log in Real-Time
```bash
tail -f $(ls -t ~/code/openagents/docs/logs/$(date +%Y%m%d)/*.md | head -1)
```

---

## How to Control the Agent

### Check if Running
```bash
launchctl list | grep mechacoder
# Shows PID if running, "-" if not
```

### Stop the Agent
```bash
launchctl unload ~/Library/LaunchAgents/com.openagents.mechacoder.plist
```

### Start the Agent
```bash
cd ~/code/openagents && ./scripts/start-mechacoder.sh
```

### Restart the Agent
```bash
launchctl unload ~/Library/LaunchAgents/com.openagents.mechacoder.plist
cd ~/code/openagents && ./scripts/start-mechacoder.sh
```

### Run Once Manually (for testing)
```bash
cd ~/code/nostr-effect && bun ~/code/openagents/src/agent/do-one-bead.ts --dir ~/code/nostr-effect
```

---

## Configuration

### launchd Plist Location
```
~/Library/LaunchAgents/com.openagents.mechacoder.plist
```

### Source Plist (edit this one)
```
~/code/openagents/scripts/com.openagents.mechacoder.plist
```

### Key Settings in Plist
- `StartInterval`: 300 (runs every 5 minutes)
- `WorkingDirectory`: /Users/christopherdavid/code/nostr-effect
- `PATH`: Includes ~/.local/bin for bd command

### Agent Code
```
~/code/openagents/src/agent/do-one-bead.ts   # Main entry point
~/code/openagents/src/agent/loop.ts          # Agent loop
~/code/openagents/src/agent/prompts.ts       # System prompts
~/code/openagents/src/llm/openrouter.ts      # LLM client
```

---

## Bead Management

### Check Ready Beads
```bash
cd ~/code/nostr-effect && $HOME/.local/bin/bd ready --json
```

### Check All Beads
```bash
cd ~/code/nostr-effect && $HOME/.local/bin/bd list --json | jq '.[] | "\(.id) | \(.status) | \(.title)"'
```

### Reset Stuck Bead
```bash
$HOME/.local/bin/bd update <bead-id> --status open
```

### Close Bead Manually
```bash
$HOME/.local/bin/bd close <bead-id> --reason "Manual close: <reason>"
```

---

## Troubleshooting

### Agent Not Running
```bash
# Check launchd status
launchctl list | grep mechacoder

# Check for errors
cat ~/code/openagents/logs/mechacoder-stderr.log | tail -20

# Restart
./scripts/start-mechacoder.sh
```

### Agent Running But Not Making Progress
1. Check latest log for errors
2. Check if beads are stuck in_progress
3. Reset stuck beads: `bd update <id> --status open`

### API Errors
- Check `OPENROUTER_API_KEY` is set
- Must use model `x-ai/grok-4.1-fast` (it's free)
- Raw fetch is used (not SDK) to avoid validation issues

### Type Errors on Push
- Agent should run `bun run typecheck` before committing
- If push fails, agent should fix types and retry
- May need to manually fix and push if agent gets stuck

### bd Command Not Found
- Agent must use full path: `$HOME/.local/bin/bd`
- PATH in plist should include `~/.local/bin`

---

## Model Configuration

**CRITICAL: Never change the model from `x-ai/grok-4.1-fast`**

This is the only free model on OpenRouter. The agent prompt and AGENTS.md both specify this requirement.

---

## Log Format Example

```markdown
# Bead Run Log

Started: 2025-12-02T15:04:07.329Z

[timestamp] DO ONE BEAD - Starting
[timestamp] Work directory: /Users/christopherdavid/code/nostr-effect
[timestamp] Changed to: /Users/christopherdavid/code/nostr-effect

## Agent Turns

### Tool Call: bash
{"command":"$HOME/.local/bin/bd ready --json"}

### Tool Result: bash ✅ SUCCESS
[... output ...]

### Assistant
I'll claim bead nostr-effect-997.1...

[... more turns ...]

## Final Message
BEAD_COMPLETED: nostr-effect-997.1

[timestamp] SUCCESS - Bead completed!
```

---

## For Future Agents

If you're an AI agent reading this:

1. **Don't modify this file** unless asked
2. **Check logs** before assuming agent state
3. **Use bd commands** for bead management, not markdown TODOs
4. **Reset stuck beads** if agent appears hung
5. **Run typecheck** before any commit
6. **Never change the model** from grok-4.1-fast

---

## Quick Reference

| Task | Command |
|------|---------|
| Check if running | `launchctl list \| grep mechacoder` |
| Stop agent | `launchctl unload ~/Library/LaunchAgents/com.openagents.mechacoder.plist` |
| Start agent | `./scripts/start-mechacoder.sh` |
| View latest log | `cat $(ls -t ~/code/openagents/docs/logs/$(date +%Y%m%d)/*.md \| head -1)` |
| Watch log | `tail -f $(ls -t ~/code/openagents/docs/logs/$(date +%Y%m%d)/*.md \| head -1)` |
| Check beads | `bd list --json` |
| Reset bead | `bd update <id> --status open` |
| Check commits | `git log --oneline -5` |
