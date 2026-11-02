# 37. A terminal inside the agent

> **Status: planned.** Nothing in this chapter is written yet.

The other direction: Coder runs programs that expect a terminal (test
runners with progress bars, interactive prompts, editors) and needs to read
what they drew. That takes a VT emulator: a parser for escape sequences, a
grid of cells with scrollback, and a snapshot the model can read as text.

## What this chapter will cover

- A VT parser state machine in Coax, and the subset of sequences real tools
  emit.
- A cell grid, alternate screen, scroll regions and wide characters.
- Snapshots as text for the model, and keystrokes sent back as tool calls.
- Checking the emulator against libghostty as a test oracle on recorded
  sessions, without linking it into Coder.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
