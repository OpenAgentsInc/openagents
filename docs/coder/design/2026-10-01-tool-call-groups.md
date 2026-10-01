# Tool-call groups, as Grok Build draws them

2026-10-01, [#10117](https://github.com/OpenAgentsInc/openagents/issues/10117).

Coder runs show their tool calls the way Grok Build (xAI's Rust TUI) shows its
own: grouped, condensed by default, expandable. This note records what Grok
Build does, with captures of its real output, and how ours maps to it.

## What Grok Build does

Source: `~/work/projects/repos/grok-build` (read-only reference,
`SOURCE_REV` 559751fdcec0), and `grok` 1.0.46 run in a 110x45 tmux pane on
a scratch crate on 2026-10-01 (prompt: read three files, grep for `TODO`
and `fn`, list `src`, run `ls -la` and `git log --oneline`).

**Grouping.** `xai-grok-pager/src/scrollback/state/verb_group.rs` and
`state/groups.rs`.

- A tool call either looks or acts (`ToolCallBlock::verb_group_kind` in
  `scrollback/blocks/tool/mod.rs`). Looking calls (read, search, list, web
  fetch, web search, memory and skill reads) fold eagerly: a maximal run of
  consecutive ones becomes one header row. Acting calls (execute, edit, MCP
  `use_tool`, sent messages, other) never fold; each is its own row and
  breaks a run.
- Finished, collapsed thoughts inside a run are claimed by it (hidden) but
  never counted. A run folds when it has at least one member.
- The header label counts members by verb, buckets in first-appearance
  order: "Read 3 files, Searched 2 patterns, Listed 1 dir". The verb is
  present tense ("Reading") while any member runs; failures add
  " · N failed". Nouns: file(s), pattern(s), dir(s), website(s), and for
  the kinds that only appear in truncation labels "Ran N commands",
  "Edited N files", "Ran N tools".
- Truncation: a dense run of standalone rows longer than
  `group_max_visible + 1` (default 10, `xai-grok-pager-render/src/appearance/config.rs`)
  hides its oldest rows under one header labelled with the same vocabulary
  ("Ran 6 commands"), else "N more".
- The `group_tool_verbs` setting (default on, `settings/defs.rs`) turns the
  folding off.

**Per-call line shapes** (`scrollback/blocks/tool/*.rs`): a verb, a target,
and a result summary. `Read lib.rs` (basename collapsed, full path when the
row is open), `Search "TODO" (2 matches in 2 files)`, `List src (3 entries)`,
`Run <description or command>`, `Edit main.rs +1/-1`. Open, a read shows
numbered file lines and a command shows `$ command`, a blank line, and its
output.

**Glyphs and colors.** A group header is `◈ ` (U+25C8,
`glyphs::diamond_dotted`), a call `◆ `. From the captured ANSI: header
glyph rgb(108,108,108), header label bold rgb(120); a call's glyph green on
success, red on failure, animated while running; "Run " bold rgb(108) and
the target rgb(108); prose rgb(200).

**Keys** (`actions/defaults.rs`). Tab focuses the scrollback; `e` toggles
the selected row (`ToggleFold`), `E` toggles every row (`ToggleExpandAll`),
`l`/→ opens and `h`/← closes, Enter opens a group; Ctrl+E toggles
thinking. In minimal mode, Ctrl+E and `/expand` re-print the last folded
block. Ctrl+O is YOLO mode, not expansion.

Condensed (the turn as it finished):

```
     ◈ Read 3 files, Searched 2 patterns, Listed 1 dir
     ◆ Run List files and recent git commits
```

The group opened (Tab, select it, Enter):

```
   ◈ Read 3 files, Searched 2 patterns, Listed 1 dir
   › Read lib.rs
   ◆ Read main.rs
   ◆ Read Cargo.toml
   ◆ Search "TODO" (2 matches in 2 files)
   ◆ Search "fn" (3 matches in 2 files)
   ◆ List src (3 entries)
   ◆ Run List files and recent git commits
```

A member and the command opened (→):

```
     ◆ Read src/lib.rs

     1  pub fn add(a: i32, b: i32) -> i32 { a + b }
     2  // TODO: subtract
     3  pub fn mul(a: i32, b: i32) -> i32 { a * b }

     ◈ Read 2 files, Searched 2 patterns, Listed 1 dir
     ...
 │┃  ◆ Run List files and recent git commits
 │┃  $ ls -la && git log --oneline
 │┃
 │┃  total 16
 │┃  ...
 │┃  45f98b7 init
```

## How ours maps to it

**One shared module.** `openagents_chat::tool_groups` holds the grouping
every surface uses: `Stretch` (a run's consecutive commands, tool calls,
and thoughts), `Stretch::items` (groups, standalone calls, thoughts, and
the truncation fold, Grok Build's rules above with `MAX_VISIBLE` 10),
`Shown::line` (the per-call shape), `label` (the counted label), and
`Stream` (the same items for a log that only appends). No surface keeps a
copy.

**Typed input, never words.** A command or tool-call step now carries a
typed `Call` (`coder_events::Call`: `verb`, `target`, the agent's own
`about` for a command, `failed`), set by the event mapper from the same
argument fields #10113 reads for the step's line (`target_file`,
`filePath`, `path`, `target_directory`, `command`, `pattern`, `url`, the
ACP kind), and a failed result's status. A Microcoder command (Codex,
Claude Code) is `Verb::Run` with its command, and its `output` event
carries the exit. Grouping reads only these; a line written before `call`
existed reads as one standalone call.

| Grok Build | Ours |
| --- | --- |
| read, search, list, web fetch fold | `Verb::Read`, `Search`, `List`, `Fetch` fold (`tool_groups::looks`) |
| execute, edit, other stand alone | `Run`, `Edit`, `Delete`, `Move`, `Other` stand alone |
| thoughts claimed by a run | thoughts claimed by a run |
| "Read 3 files, Searched 2 patterns", present tense while running, " · N failed" | the same words |
| `group_max_visible` 10, "Ran 6 commands" | `MAX_VISIBLE` 10, the same label |
| `Search "TODO" (2 matches in 2 files)` | `Search "TODO"`; what the tool returned ("found 2 matches") shows open |
| `Run <description>` | `Run <about>`, else the command's first line |
| red diamond on failure | ` · exit 101`, ` · timed out`, ` · failed` after the line |
| colors | our white ladder (below) |

**OpenAgents Terminal** (`openagents-terminal` `Row::Tools`, drawn by
`coder-terminal` `RunRow::Tools`). Condensed: `◈ label` (mark Half, label
ThreeQuarters), `◆ line` (mark and line Half), `· thought` (Half); a
failure's result and the mark of a failed or running call at Full.
Expanded: the label, then each call with `$ command` and its output (Half),
clipped to the width. Ctrl+O (Grok Build has no free chord; its scrollback
`E` toggles everything) and `/expand` toggle every stretch at once. A reply
draws no row of its own in the terminal, so it never splits a stretch.

**Desktop and phone** (`openagents-chat-app` `coder_run`). A group is one
tool row named by its label; a click or tap opens it to each call's line
and what it returned. A call is one tool row named by its verb, detailed by
its target and any failure, opening to its command and output. A thought
stays a "Thinking" row. A row with nothing inside does not open.

**`openagents chat`** text prints each item once it is final: a group's
label when something other than a looking call comes, a call when it has
its result. `--json` is unchanged apart from the new `call` field.

## Captures

The terminal snapshots (`crates/openagents-terminal/snapshots/`,
`crates/coder-terminal/tests/snapshots/tools_*.txt`), the desktop outline
`crates/openagents-desktop/snapshots/dsk-11-coder-tools.txt`, and the
desktop PNGs
[`dsk-11-coder-tools-condensed.png`](../../../crates/openagents-desktop/screenshots/dsk-11-coder-tools-condensed.png)
and
[`dsk-11-coder-tools-expanded.png`](../../../crates/openagents-desktop/screenshots/dsk-11-coder-tools-expanded.png) (written by
`tool_calls_show_grouped_and_a_click_opens_a_group` under
`OPENAGENTS_CODER_CAPTURE_DIR`) are all drawn from
`crates/openagents-chat/fixtures/coder-events/tools-{grok,codex,claude}.ndjson`,
real runs recorded on 2026-10-01 (home paths replaced, `target/` build
output trimmed from the result's file list). Devin and OpenCode reach the
same ACP mapping as Grok Build; their argument names are covered by
`an_agents_tool_calls_are_typed`.

A real run of "do a test delegation to grok" in OpenAgents Terminal, in a
tmux pseudo-terminal:

```
   Coder · Grok Build (default) · worktree of grokdemo at
   /Users/me/.openagents/worktrees/grokdemo-e182680b791e
   You asked for Grok Build; it is signed in and has capacity.
   ◈ Listed 1 dir, Read 4 files
   Coder finished · 0 files changed · +0 -0
```

## License

Grok Build is Apache License 2.0 (`LICENSE`, "Copyright 2023-2026
SpaceXAI"). It allows use, modification, and redistribution, including
of derivative works, provided the license and notices travel with copied
code. Nothing here copies its code: the grouping is reimplemented from the
behavior described above, so no notice is owed; the glyphs and the label
wording are not copyrightable expression.
