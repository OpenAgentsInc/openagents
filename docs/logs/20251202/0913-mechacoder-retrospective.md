# MechaCoder Overnight Agent - Development Retrospective

**Date:** 2025-12-02
**Author:** Droid (with human guidance)
**Status:** In progress - agent running, iterating on reliability

---

## Executive Summary

We're building an autonomous coding agent ("MechaCoder") that runs every 5 minutes via macOS launchd, picks up beads (issues) from a project, implements them, runs tests, commits, pushes, and closes the bead. The goal is overnight autonomous coding where the user wakes up to a series of commits on main.

---

## What We Built

### 1. Core Agent Infrastructure

**Files created:**
- `src/agent/do-one-bead.ts` - Single bead runner (cron-compatible)
- `src/agent/overnight.ts` - Multi-bead loop runner
- `src/agent/run.ts` - Simple test runner
- `src/agent/loop.ts` - Core agent loop with tool execution
- `src/agent/session.ts` - JSONL session persistence
- `src/agent/prompts.ts` - System prompts and conventions

**Scripts:**
- `scripts/com.openagents.mechacoder.plist` - macOS launchd config
- `scripts/start-mechacoder.sh` - Setup script

### 2. OpenRouter Integration

Modified `src/llm/openrouter.ts` to:
- Use raw `fetch()` instead of SDK (avoids validation issues with Grok responses)
- Support tool calling with proper message format conversion
- Add debug logging for request/response tracking

### 3. Tool System

The agent has access to:
- `read` - Read file contents
- `edit` - Edit files with find/replace
- `write` - Create new files
- `bash` - Execute shell commands

---

## Problems Encountered & Solutions

### Problem 1: `bd` command not found in cron environment

**Symptom:** Agent ran `bd ready --json` but got "command not found"

**Root Cause:** launchd runs with minimal PATH, doesn't include `~/.local/bin`

**Solution:** 
1. Updated plist to include full PATH: `/Users/christopherdavid/.local/bin:/Users/christopherdavid/.bun/bin:/usr/local/bin:/usr/bin:/bin`
2. Updated system prompt to tell agent to use `$HOME/.local/bin/bd` explicitly

**Lesson:** Cron/launchd environments are NOT the same as interactive shells. Always use absolute paths or ensure PATH is set.

### Problem 2: OpenRouter SDK ResponseValidationError

**Symptom:** `ResponseValidationError: Response validation failed` when calling Grok

**Root Cause:** Grok returns extra fields (`reasoning_details`, `reasoning`) that the OpenRouter SDK's Zod schema doesn't expect, causing validation to fail.

**Solution:** Replaced SDK's `client.chat.send()` with raw `fetch()` call that doesn't validate response schema.

**Lesson:** SDKs with strict response validation can break when providers add new fields. Raw API calls are more resilient.

### Problem 3: Agent claims completion without doing work

**Symptom:** Agent says "BEAD_COMPLETED" after only reading files, without actually writing code, committing, or pushing.

**Root Cause:** The system prompt wasn't explicit enough. The model would read context, decide the task was "done" conceptually, and claim completion.

**Solution:** Rewrote system prompt with:
- Explicit phases (Find Work → Understand → Implement → Verify → Commit → Close)
- VALIDATION CHECKLIST that must ALL be true before claiming completion
- Emphasis on "You MUST call edit or write tool at least once"
- Clear statement: "If ANY of these are NO, you have NOT completed the bead"

**Lesson:** LLMs will take shortcuts if you let them. Be VERY explicit about required actions.

### Problem 4: Tool message format mismatch

**Symptom:** API errors when sending tool results back

**Root Cause:** OpenRouter SDK expects `toolCallId` but the API wants `tool_call_id`

**Solution:** Added conversion in `makeRequestBody()` to transform message format for tool responses.

**Lesson:** Different parts of the same API ecosystem may use different naming conventions (camelCase vs snake_case).

### Problem 5: Multiple agents claiming same bead

**Symptom:** Two beads showing `in_progress` simultaneously

**Root Cause:** Multiple agent runs (manual + launchd) picking up work concurrently

**Solution:** 
1. System prompt includes concurrency guard: "if a bead is already in_progress, do not take it"
2. Should probably add file-based locking in future

**Lesson:** Autonomous agents need coordination mechanisms when running in parallel.

### Problem 6: Agent writes HTML entities instead of characters

**Symptom:** Test file contains `&#39;` instead of `'`

**Root Cause:** Unknown - possibly model hallucination or encoding issue

**Solution:** Not fully solved yet. May need post-processing or better prompting.

**Lesson:** Always verify generated code compiles/runs before committing.

---

## What's Working Well

1. **The basic loop works** - Agent can:
   - Query beads with `bd ready --json`
   - Claim a bead with `bd update --status in_progress`
   - Read source files to understand context
   - Write new test files
   - (Sometimes) run tests and commit

2. **Logging is comprehensive** - Every run creates a detailed markdown log in `docs/logs/YYYYMMDD/HHMMSS-bead-run.md` with:
   - All tool calls and their arguments
   - All tool results
   - Final message from assistant
   - Timing information

3. **launchd integration works** - Agent runs every 5 minutes automatically

4. **Grok model is fast and free** - x-ai/grok-4.1-fast provides good results at no cost

---

## Current State

As of this writing:
- Agent is running in background
- It has created new test files: `Nip16Module.test.ts`, `Nip28Module.test.ts`, `Nip50Module.test.ts`
- It modified `Nip05Service.test.ts`
- Beads 997.1 and 997.2 are `in_progress`
- Agent finished a run but didn't complete commit/push (response got cut off)

---

## Suggested Improvements

### High Priority

1. **Add file-based locking** - Prevent multiple agents from running simultaneously
   ```typescript
   const lockFile = '/tmp/mechacoder.lock';
   if (fs.existsSync(lockFile)) {
     const pid = fs.readFileSync(lockFile, 'utf8');
     // Check if process still running
     // Exit if so
   }
   fs.writeFileSync(lockFile, process.pid.toString());
   ```

2. **Add timeout per LLM call** - Currently no timeout on fetch(), so a slow response hangs forever
   ```typescript
   const controller = new AbortController();
   const timeout = setTimeout(() => controller.abort(), 60000);
   const res = await fetch(url, { signal: controller.signal });
   ```

3. **Validate code before committing** - Run typecheck and tests, only commit if they pass
   ```typescript
   // After edit, before commit:
   // 1. bun run typecheck
   // 2. bun test <affected files>
   // 3. If either fails, don't commit
   ```

4. **Stream responses for better UX** - Currently we wait for full response. Streaming would show progress.

5. **Add retry logic** - If a tool call fails, retry with backoff instead of giving up

### Medium Priority

6. **Better error handling in tools** - Some tool errors are cryptic. Improve error messages.

7. **Add a "review" step** - Before pushing, show a diff summary and wait for confirmation (optional flag)

8. **Track token usage** - Log prompt_tokens and completion_tokens to monitor costs

9. **Add memory/context management** - For long sessions, summarize earlier turns to stay within context limits

10. **Support multiple repos** - Currently hardcoded to nostr-effect. Make configurable.

### Low Priority

11. **Web dashboard** - Show agent status, recent runs, success rate

12. **Slack/Discord notifications** - Notify on completion or error

13. **Automatic PR creation** - Instead of pushing to main, create a PR for review

---

## Questions / Things That Would Help

### Technical Questions

1. **Is there a way to get Grok without the `reasoning_details`?** - This caused SDK validation issues. Maybe an API flag?

2. **Should we use a different model for different tasks?** - Grok is fast but sometimes hallucinates. Would a slower model be more reliable for code generation?

3. **How do we handle merge conflicts?** - If someone else pushes while agent is working, git push will fail. Need conflict resolution strategy.

4. **What's the best way to handle long-running tasks?** - Some beads might take 30+ minutes. Should we have a max runtime? Resume capability?

### Process Questions

5. **How should we prioritize beads?** - Currently just "highest priority task". Should we consider:
   - Estimated complexity?
   - Dependencies?
   - Time since created?

6. **When should agent ask for human help?** - Currently it just gives up or closes with blocking reason. Should it create a "needs human" tag?

7. **How do we measure success?** - Metrics to track:
   - Beads completed per day
   - Success rate (completed vs attempted)
   - Average time per bead
   - Test pass rate
   - Commits reverted

### Infrastructure Questions

8. **Should we run on a dedicated server?** - Currently on user's laptop. Server would be:
   - Always on
   - More resources
   - Better logging/monitoring
   - But: costs money, security concerns

9. **How to handle secrets?** - OPENROUTER_API_KEY is in plist file. Should use keychain or vault.

10. **Backup/recovery** - What if agent corrupts a file? Need rollback capability.

---

## Timeline of Session

1. **Start:** User asks to set up overnight autonomous agent
2. Created `do-one-bead.ts`, `overnight.ts`, launchd plist
3. First run: `bd` command not found - fixed PATH
4. Second run: OpenRouter SDK validation error - debugged, found Grok returns extra fields
5. Switched to Claude (works) but user said "MUST BE GROK - it's free"
6. Replaced SDK with raw fetch - works!
7. Agent runs, claims bead, reads files, but claims completion without doing work
8. Improved system prompt with validation checklist
9. Agent now writes code but sometimes times out before committing
10. Current: Agent running, has created test files, needs to commit/push

---

## Key Learnings for Future Agents

1. **Environment matters** - Cron/launchd ≠ interactive shell
2. **SDKs can be fragile** - Raw API calls more resilient
3. **Be explicit in prompts** - LLMs take shortcuts
4. **Validate before committing** - Don't trust generated code blindly
5. **Logging is essential** - Can't debug what you can't see
6. **Concurrency needs coordination** - Multiple agents = chaos
7. **Free tier has tradeoffs** - Grok is free but has quirks
8. **Incremental progress** - Better to complete small tasks reliably than big tasks flakily

---

## Next Steps

1. Wait for current agent run to complete
2. Check if it committed/pushed successfully
3. If not, debug why and iterate
4. Once reliable, let it run overnight
5. Review results in morning
6. Iterate based on what worked/failed

---

## Appendix: File Locations

```
~/code/openagents/
├── src/agent/
│   ├── do-one-bead.ts    # Main cron runner
│   ├── overnight.ts       # Multi-bead loop
│   ├── loop.ts           # Core agent loop
│   ├── session.ts        # Session persistence
│   └── prompts.ts        # System prompts
├── src/llm/
│   └── openrouter.ts     # LLM client (raw fetch)
├── src/tools/
│   ├── read.ts           # File reading
│   ├── edit.ts           # File editing
│   ├── write.ts          # File creation
│   └── bash.ts           # Shell execution
├── scripts/
│   ├── com.openagents.mechacoder.plist
│   └── start-mechacoder.sh
├── docs/logs/YYYYMMDD/
│   └── HHMMSS-bead-run.md  # Per-run logs
└── logs/
    ├── mechacoder-stdout.log
    └── mechacoder-stderr.log
```

---

*This document will be updated as we continue iterating.*
