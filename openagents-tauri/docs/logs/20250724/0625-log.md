# Claude Code Permission Fix - 2025-07-24 06:25

## Issue
User reported that Claude Code was requesting permissions even when the project folder was moved, despite our expectation that the `--dangerously-skip-permissions` flag would bypass permission prompts. The error message shown was:
```
Claude requested permissions to use Bash, but you haven't granted it yet.
```

## Root Cause
The `--dangerously-skip-permissions` flag was not being passed to the Claude Code command execution in the `ClaudeSession::send_message_static` method.

## Solution

### 1. Added `--dangerously-skip-permissions` Flag
Modified the command construction in `src-tauri/src/claude_code/manager.rs` to include the flag for both command variants:
- With `--continue` flag (line 166)
- Without `--continue` flag (line 173)

### 2. Enhanced Error Handling
Added specific error detection and user-friendly messaging when permission errors occur:
- Detects "Claude requested permissions" error messages
- Provides actionable steps for users:
  1. Restart Claude Code
  2. Update Claude Code: `claude update`
  3. Check if Claude Code is running with correct permissions

### 3. Version Detection and Warning
- Added `version` field to `ClaudeSession` struct
- Captures Claude Code version during session initialization
- Warns users if they're running an old version (0.2.x)
- Displays version info in initialization messages

### 4. Improved User Feedback
- System messages now show Claude Code version when initialized
- Clear warning messages for old versions
- Better error messages with emoji indicators (⚠️) for visibility

## Files Modified
- `src-tauri/src/claude_code/manager.rs`:
  - Added `--dangerously-skip-permissions` to command execution
  - Added version field to ClaudeSession struct
  - Enhanced error handling for permission errors
  - Added version detection and warning system

## Testing
- Rust build: ✅ Passed with no errors or warnings
- TypeScript build: ✅ Passed successfully

## Expected Behavior
After this fix, Claude Code should no longer prompt for permissions when:
- Moving project folders
- Starting new sessions
- Using any Claude Code commands

The `--dangerously-skip-permissions` flag bypasses all permission prompts, allowing seamless operation regardless of project location changes.