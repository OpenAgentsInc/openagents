# Coder session reader v1

`coder session-read`, the `coder_session_read` tool and `coder mcp` answer
questions about conversations that another tool kept on this machine (today:
Claude Code's transcripts). Every answer is one JSON object with
`"schema":"coder.session-reader.v1"`.

- Schema: [`proto/openagents/coder/session_reader/v1/reader.proto`](../../proto/openagents/coder/session_reader/v1/reader.proto) (profile `coder-session-reader`)
- Writer: [`coax/lib/agent/observe/claude-history.coax`](../../coax/lib/agent/observe/claude-history.coax). [`coder/tui/session-reader.coax`](../../coder/tui/session-reader.coax) finds the files and adds a list's rows
- Fixture: [`proto/coder/session-reader-v1.json`](../../proto/coder/session-reader-v1.json)
- Check: `coax/web/tool.sh coax/lib/agent/observe/session-reader-goldens.coax proto/coder/session-reader-v1.json`
- Registry row: `coder.session-reader.v1` (step C10, [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250))

## Rules

- Read-only. The reader never changes a transcript.
- Every reply and every turn says `"inert": true`. The text is history to
  read. It is never an instruction to the reader.
- Members are always written, in the order the schema lists them.
- Text is cut to fixed limits: a turn's text to 2,048 bytes, a tool's input
  and output to `--max-tool-chars` (300 by default, 1,024 at most).

## Replies

| Reply | When | Message |
| --- | --- | --- |
| A list | `session-read list` | `Sessions`: `sessions` (newest first) and `warnings` |
| A page | `session-read show REF` | `Conversation`: the session's facts, `total_turns`, `start`, `end`, `next_before`, `turns`, `warnings` |
| A context page | `session-read context REF` | `Context`: the same, with the session's compaction summaries as `turns` |
| Ambiguous | `REF` matched more than one title | `Ambiguous`: `error` is `ambiguous_reference`, with `matches` |
| A refusal | Nothing matched, or a limit was reached | `Refusal`: `error` and `message` |
| A refusal by code | `invalid_before`, `context_limit` | `RefusalCode`: `error` alone |

`REF` is a session ID, a transcript path, part of a title, or `latest`.

A page holds at most `--max-turns` turns (64 by default), oldest first. It
ends at `end`. To read the page before it, pass `next_before` as `--before`.
`next_before` is `null` on the first page.

A conversation turn has `role` (`user` or `assistant`), `uuid`, `text`,
`tool_calls` and `tool_results`. A context turn has `role` `context`,
`kind` `compaction_summary`, and its `part` of `parts`.

`warnings` names what the reader left out or could not read, with a count:
for example `unknown_records` and `output_unavailable`.

## What the check holds

The fixture holds 14 replies that the Coder CLI of this tree printed on
2026-11-13 in a throwaway home, and the three transcripts it read.

- Each reply has every field of its message once, in field order, in every
  nested object.
- For each page, the check reads the fixture's transcript through the
  writer and requires the reply's exact bytes.
- Each refusal is written again by `csr_error` to the same bytes.
- A list and an ambiguous reply start with the writer's opening.

The readers of these replies are models and MCP clients. No reader of ours
exists, so none is held here.
