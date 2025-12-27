# OpenAgents Autopilot Replay Viewer

Web-based viewer for Autopilot session replays.

## Quick Start

### View a Replay

1. Open `replay-viewer.html` in your browser
2. Click "Choose Replay Bundle"
3. Select a replay JSON file (e.g., `sample-replay.json`)
4. Click Play to watch the autopilot session

### Create a Replay Bundle

From a session log (JSONL format):

```bash
cargo run -p autopilot --example replay_demo \
  ~/.openagents/sessions/20251226/153045-abc123.jsonl \
  my-demo.json
```

The replay bundle will be created with secrets automatically redacted.

## Features

- **Timeline Playback:** Watch events unfold in real-time
- **Scrubbing:** Click timeline to jump to any point
- **Speed Control:** Configurable playback speed (default 2x)
- **Event Types:**
  - Tool calls (Read, Edit, Bash, etc.)
  - Tool results with outputs
  - Phase transitions
  - Assistant messages
- **Receipts Panel:** Test results, CI status, file changes
- **Metadata Display:** Model, duration, costs

## File Format

Replay bundles are JSON files with this structure:

```json
{
  "version": "1.0",
  "id": "replay_abc123",
  "created_at": "2025-12-26T12:00:00Z",
  "metadata": {
    "duration_seconds": 480,
    "playback_speed": 2.0,
    "model": "claude-sonnet-4-5-20250929"
  },
  "timeline": [
    {
      "t": 0,
      "type": "tool_call",
      "tool": "Read",
      "data": {...}
    }
  ],
  "receipts": {
    "tests_run": 42,
    "tests_passed": 42,
    "ci_status": "success",
    "files_changed": 3
  }
}
```

## Usage Examples

### Local Development

```bash
# Open viewer in browser
open demo/replay-viewer.html

# Or serve via HTTP
python3 -m http.server 8000
# Then visit http://localhost:8000/replay-viewer.html
```

### Create Demo Content

1. Run autopilot on an issue
2. Wait for completion
3. Find session log in `~/.openagents/sessions/YYYYMMDD/`
4. Convert to replay bundle
5. Load in viewer

### Publish a Demo

1. Review replay for quality (no crashes, tests passed)
2. Ensure secrets are redacted
3. Upload bundle JSON to CDN
4. Embed viewer on website
5. Link to bundle in viewer

## Keyboard Shortcuts

- **Space:** Play/Pause
- **Left Arrow:** Rewind 5 seconds
- **Right Arrow:** Fast-forward 5 seconds
- **Home:** Jump to start
- **End:** Jump to end

## Browser Compatibility

- Chrome/Edge: ✅ Fully supported
- Firefox: ✅ Fully supported
- Safari: ✅ Fully supported
- Mobile: ⚠️ Works but not optimized

## Next Steps

- [ ] Add keyboard shortcuts
- [ ] Implement diff view for file changes
- [ ] Add search/filter for events
- [ ] Export as video/GIF
- [ ] Dark/light theme toggle
- [ ] Mobile responsive layout
- [ ] Auto-play on scroll (for homepage)

## Related Files

- `crates/autopilot/src/replay.rs` - Replay bundle format
- `crates/autopilot/examples/replay_demo.rs` - CLI conversion tool
- `crates/autopilot/src/logger.rs` - Session logging
- `.openagents/d-027-progress.md` - Implementation progress

## Demo Session Location

Session logs are stored at:
```
~/.openagents/sessions/YYYYMMDD/HHMMSS-sessionid.jsonl
```

Reports are stored at:
```
~/.openagents/reports/YYYYMMDD/HHMMSS-sessionid.md
```

## License

Part of OpenAgents - see repository root for license information.
