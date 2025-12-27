# Demo Gallery Quick Start

## What is This?

The OpenAgents Autopilot Demo Gallery is a web-based showcase of autonomous coding sessions. Visitors can watch replay videos of the AI agent working on real issues, see code diffs, and understand the value proposition before trying it themselves.

## For Viewers (Public)

### View Existing Demos

1. Visit the demo gallery (locally or deployed)
2. Click any "LIVE" demo card
3. Use playback controls:
   - **Space**: Play/Pause
   - **Left/Right arrows**: Rewind/Fast-forward 5 seconds
   - **Home/End**: Jump to start/end
   - **Click timeline**: Scrub to specific point

### Upload Your Own Replay

1. Get a replay bundle JSON file from someone
2. Click "Choose Replay Bundle" in upload section
3. Select the JSON file
4. Viewer loads automatically

## For Contributors (Team)

### Generate a New Demo in 3 Steps

**Step 1: Run Autopilot**
```bash
# Pick an issue on the OpenAgents repo
# Start autopilot (replace <issue_url> with real GitHub issue)
cargo run -p autopilot -- run <issue_url>

# Wait for completion
# Session log will be at: ~/.openagents/sessions/YYYYMMDD/HHMMSS-sessionid.jsonl
```

**Step 2: Convert to Replay Bundle**
```bash
# Find the session log
SESSION_LOG=$(ls -t ~/.openagents/sessions/*/*.jsonl | head -1)

# Convert to publishable bundle
cargo run -p autopilot --example replay_demo \
  $SESSION_LOG \
  demo/my-demo-name.json

# This automatically redacts secrets and optimizes for demo
```

**Step 3: Add to Gallery**
```bash
# 1. Edit demo/index.html
# 2. Add entry to the `demos` array (see examples in file)
# 3. Commit and push

git add demo/my-demo-name.json demo/index.html
git commit -m "Add demo: <short description>"
git push origin main
```

That's it! The demo is now live.

## Testing Locally

```bash
# Start local server
cd demo
python3 -m http.server 8000

# Open in browser
# Gallery: http://localhost:8000/
# Viewer: http://localhost:8000/replay-viewer.html
```

## Quality Guidelines

Before publishing a demo, ensure:

### Must Have
- ✅ Session completed successfully (no crashes)
- ✅ Changes work (tests pass, code compiles)
- ✅ PR would be mergeable (or was merged)
- ✅ No secrets leaked (check bundle for API keys, tokens)

### Should Have
- ✅ Clear issue description (viewers understand context)
- ✅ Reasonable duration (2-15 minutes at 2x speed)
- ✅ Interesting narrative (shows planning, implementation, testing)
- ✅ Clean diff (no massive file dumps or generated code)

### Nice to Have
- ✅ Tests included in the change
- ✅ Documentation updated
- ✅ Demonstrates a pattern (useful for learning)

## Demo Selection Strategy

**Good Demo Candidates:**
- Feature additions (clear before/after)
- Bug fixes with reproduction (shows debugging)
- Refactoring (demonstrates code quality)
- Test suite creation (shows thoroughness)
- Documentation improvements (showcases communication)

**Avoid:**
- Pure scaffolding (boring, no insight)
- Trivial typo fixes (too simple)
- Failed runs (crashes, infinite loops)
- Extremely long sessions (> 30 min real time)
- Sessions with leaked secrets

## Dogfooding Workflow

### Create Issues for Autopilot

```bash
# Create a small, well-scoped issue
gh issue create \
  --title "Add validation for replay bundle format" \
  --body "The replay viewer should validate JSON structure before loading.

Acceptance criteria:
- Check required fields (version, metadata, timeline, receipts)
- Display clear error message if validation fails
- Add test for validation function"
```

### Run Autopilot Continuously

```bash
# Start autopilot daemon (runs in background)
cargo run -p autopilot --bin autopilotd -- \
  --workdir /home/user/code/openagents \
  --project openagents

# Check status
autopilotd status

# View logs
tail -f docs/logs/$(date +%Y%m%d)/*.rlog
```

### Review and Promote

```bash
# After autopilot creates PR:
# 1. Review the PR on GitHub
# 2. If good, merge it
# 3. If GREAT, convert to demo:

SESSION_ID=<session_id_from_pr_or_logs>
SESSION_LOG=~/.openagents/sessions/$(date +%Y%m%d)/${SESSION_ID}.jsonl

cargo run -p autopilot --example replay_demo \
  $SESSION_LOG \
  demo/demo-$(date +%Y%m%d)-${SESSION_ID}.json

# Add to gallery and publish
```

## Customization

### Change Playback Speed

Edit `crates/autopilot/src/replay.rs`:

```rust
// Default is 2x (compress 10min session to 5min demo)
let playback_speed = 2.0;

// For complex sessions, use slower playback:
let playback_speed = 1.5;

// For simple sessions, use faster:
let playback_speed = 3.0;
```

### Add Custom Metadata

When creating replay bundle, you can manually edit the JSON:

```json
{
  "metadata": {
    "issue_url": "https://github.com/OpenAgentsInc/openagents/issues/123",
    "pr_url": "https://github.com/OpenAgentsInc/openagents/pull/456",
    "tags": ["featured", "beginner-friendly"]
  }
}
```

Then reference these in the demo gallery:

```javascript
tags: demo.metadata.tags || ['rust', 'feature']
```

## Troubleshooting

### "Invalid replay bundle format"
- Run `jq . demo/your-file.json` to validate JSON
- Check that `version`, `metadata`, `timeline`, `receipts` fields exist
- Ensure timeline is an array

### Demo loads but nothing plays
- Check `metadata.playback_speed` is set (should be 2.0)
- Verify timeline events have `t` timestamps
- Check browser console for JavaScript errors

### Diff display not working
- Ensure Edit tool calls have `input.old_string` and `input.new_string`
- Check `input.file_path` is set
- Verify strings are not empty

### Session log not found
- Check `~/.openagents/sessions/` directory exists
- Verify autopilot completed successfully
- Check date folder matches (YYYYMMDD format)

## Next Steps

After creating your first demo:
1. Share it with the team for feedback
2. Iterate on issue selection (find better demo candidates)
3. Experiment with different session lengths
4. A/B test which demos convert best

## Resources

- [Full README](README.md) - Detailed feature documentation
- [Deployment Guide](DEPLOYMENT.md) - How to deploy to production
- [Replay Format](../crates/autopilot/src/replay.rs) - Technical specification
- [d-027 Progress](../.openagents/d-027-progress.md) - Implementation roadmap

## FAQ

**Q: How long should a demo be?**
A: 2-5 minutes (at 2x speed) is ideal. 30 seconds is too short to show value, 15 minutes is too long to hold attention.

**Q: Can I edit the replay bundle manually?**
A: Yes! It's just JSON. You can remove boring events, adjust timestamps, or add metadata. Just ensure the JSON remains valid.

**Q: What if autopilot crashes mid-session?**
A: Partial sessions can still be converted to replays. You might want to edit the timeline to remove the crash or add a note in the demo description.

**Q: Can I create demos for other projects?**
A: Absolutely! The replay format is project-agnostic. Just run autopilot on any repo and convert the session log.

**Q: How do I rotate demos (show different ones each visit)?**
A: Edit `demo/index.html` and modify the `demos` array order. Or implement random selection in JavaScript.
