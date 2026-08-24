---
name: testing-cli-delegation
description: Run and verify the OpenAgents CLI coder TUI and its `/delegate` child-agent fleet end to end on a Linux box, including a recordable GUI terminal, a child harness with provider credentials, and the process checks that catch orphaned children.
---

# Testing the coder TUI and delegation fleet

## Build and entry point

```bash
source ~/.nvm/nvm.sh && nvm use 24
cd ~/repos/openagents/packages/openagents-cli && npx tsc -p tsconfig.build.json
export OA=~/repos/openagents/packages/openagents-cli/dist/main.js   # entry is dist/main.js
```

Always rebuild before testing: `dist/` is committed-independent and can lag `src/`.

## Child harness (delegation) setup

Delegation is off unless a child model is named, so a session started without
`--child-model` (or `OPENAGENTS_DELEGATE_MODEL`) answers `/delegate` with
"This session cannot delegate."

```bash
npm install --global opencode-ai      # provides the `opencode` harness on PATH
# Provider credential lives in a private harness config, never in the repo:
#   ~/.oa-delegate/opencode.json  (chmod 600) -> passed via --child-config
```

Never print, copy, or screenshot that config. Pass it per invocation:

```bash
COMMON="--child-model vertex-express/gemini-3.7-flash \
        --child-config $HOME/.oa-delegate/opencode.json --child-approve"
node $OA delegate "create a file x.txt containing kiwi then say done" \
  --agents 3 --concurrency 2 $COMMON      # headless; exit 1 if any child failed
node $OA coder --offline $COMMON          # interactive TUI; --offline uses a stand-in chat model
```

`--child-approve` is required for any child that must write files or run commands.

## Recordable GUI terminal

The TUI must be driven in a real PTY. A GUI terminal makes the recording useful:

```bash
pkill konsole; DISPLAY=:0 setsid konsole --hide-menubar --hide-tabbar \
  -p "Font=Monospace,18" -e /bin/bash &
DISPLAY=:0 wmctrl -a Konsole; DISPLAY=:0 wmctrl -r :ACTIVE: -b add,maximized_vert,maximized_horz
```

Konsole's `ctrl+shift+plus` font shortcut does not work under xdotool typing — set the
font with `-p "Font=Monospace,18"` at launch instead. Resize the terminal grid without
touching the window with an escape sequence, which is how you exercise a small terminal:

```bash
printf '\033[8;24;80t'    # 80x24
printf '\033[8;37;112t'   # back to a large grid
```

## Designing prompts that make the fleet observable

Children on a fast model finish in ~5 s and their rows are pruned once their finish
notice is read, so a naive prompt gives you nothing to look at:

- To watch tool/token counters advance: ask for several tool calls, e.g.
  `/delegate 2x create five files n1.txt..n5.txt each containing kiwi, one write call per file, then say done`.
- To hold rows on screen long enough for `ctrl+x`, resizes, or queue behaviour:
  `/delegate 2x run the shell command \`sleep 120\` then say done`.
- To exercise the row cap and the "+N more" line: 12 children with the default
  `--concurrency 4` shows 4 running + 4 queued rows and `+4 more` (cap is 8 rows).

## Verification checks that catch real bugs

- `pgrep -af opencode` after `ctrl+x` and after `ctrl+d`.
- Also check **grandchildren**: `pgrep -af "sleep 120"` and `ps -o pid,ppid …`. `opencode`
  puts its own tool processes in groups of their own, so a stop that signals only the
  agent leaves shell commands it spawned reparented to PID 1. Any orphan is a defect, not
  a flake. Match on `pgrep -af "^sleep 120"` and note the pids before the stop: a bare
  `pgrep -af "sleep 120"` also matches the shell running your own test command, which
  reads as a false orphan.
- Also stop the parent rather than the child: `kill -INT` the `delegate` process and
  re-check. A handler that only runs after the runtime tears the process down never
  reaches the children.
- Zoom on the bottom four rows: the key-hint row and the composer row are both
  width-truncated, so verify `ctrl+x to stop agents` survives while children run and that
  a composer line longer than the terminal leaves no stale text on the rule above the
  hints. Screenshot the region rather than trusting the transcript.
- The hint row and the right-hand counter compete for the same row, so test the hint
  **twice**: once on a fresh transcript and once after it scrolls (send a chat prompt so
  `↑N above` appears). At 80 columns the unscrolled case can pass while the scrolled case
  drops the hint. `hints()` in `coder-ui.ts` now takes hint objects with a pinned flag:
  `ctrl+x to stop agents` is pinned while any child runs, and the row drops the
  conveniences first (expect `ctrl+d to quit` / `pgup/pgdn to scroll` to vanish at 80
  columns, which is intended), then the counter, and clips only as a last resort. When
  verifying a similar row, assert the pinned hint's presence rather than the full hint
  list, and record the exact left/right row text in the report.
- Composer clipping: type a line longer than the terminal width. Expect one row with a
  leading `…` showing the *tail* of what was typed, and confirm the full text was still
  sent by reading the echoed reply. A wrapped line is a defect.
- A running row that has no usage yet should read `Initializing…` with no trailing `· 0`.
- `Stopped N children.` counts only running children; queued ones still each get a
  `stopped.` notice, so N is smaller than the number of rows. That is expected.

## Devin Secrets Needed

None for delegation itself — the provider credential is a local file at
`~/.oa-delegate/opencode.json`. Real (non-`--offline`) chat replies would need an
account token via `openagents auth login --scope chat:account`.
