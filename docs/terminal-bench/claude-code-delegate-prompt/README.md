# What Claude Code sends as the Opus delegate

[`request.json`](request.json) is the first request Claude Code 2.1.280
sends when Coder One delegates to lean Opus 5.5 at low effort, the
fastest arm in [the winning-runs analysis](../winning-runs-analysis.md). It
has the system prompt, the six tool definitions, the environment block, and
the request settings. The only thing missing is the briefing itself, which
is marked by a placeholder.

## How it was captured

Claude Code doesn't print its system prompt, so the request was captured
on the wire. [`tools/capture_claude_request.py`](../../../bench/terminal-bench/tools/capture_claude_request.py)
is a local server that records request bodies and answers every call with an
error. The CLI ran against it with the delegate's own flags from
`crates/coder-one/src/delegate.rs`:

```sh
claude -p --output-format stream-json --verbose --model claude-opus-5-5 \
  --permission-mode bypassPermissions \
  --tools Bash,Read,Edit,Write,Glob,Grep --effort low < briefing.md
```

It used a dummy subscription token, so no real credential reached the
server and no request reached Anthropic. Two edits make the file match a
trial: the working directory and memory path are rewritten to `/app`, and
`metadata.user_id` is removed. The host's own values still appear in the
environment block (`Shell: unknown` because the capture cleared the
environment, and the host's kernel version), so a trial's copy differs
there.

## What's in it

| Part | Size | Cache marker |
| --- | --- | --- |
| Billing header line | 74 characters | none |
| Identity line: "You are a Claude agent, built on Anthropic's Claude Agent SDK." | 62 characters | `ephemeral`, `ttl: 1h` |
| Main system prompt | about 5,500 characters | `ephemeral`, `ttl: 1h` |
| Six tool definitions (`Bash`, `Edit`, `Glob`, `Grep`, `Read`, `Write`) | about 10,000 characters of JSON | covered by the next marker |
| First user message: the commit-attribution reminder | about 580 characters | none |
| Second user message: the briefing | 2,500 to 12,000 characters in the panel runs | none |
| Environment block, sent as a trailing system message | about 950 characters | `ephemeral`, `ttl: 1h` |

A cache marker covers everything before it, so the marker on the
environment block, which comes after the briefing, caches the briefing too.
That's why the first call's cache write includes both the system prompt and
the briefing.

Request settings: `max_tokens` 128,000, adaptive thinking,
`output_config.effort` `low`, and a context edit that keeps thinking blocks.

## Can it be changed

Yes, through documented CLI flags:

- `--system-prompt` or `--system-prompt-file` replaces the main system prompt.
- `--append-system-prompt` or `--append-system-prompt-file` adds to it.
- `--exclude-dynamic-system-prompt-sections` moves the per-machine sections
  (working directory, environment, memory paths, git status) into the first
  user message, which helps cache reuse. It has no effect with
  `--system-prompt`.
- `--tools` sets the tool list. The lean arms already use it to cut the
  tool definitions to six.

The cache lifetime can be changed too. A subscription token gets a one-hour
cache, and an API key gets five minutes. The `CLAUDE_CODE_PROMPT_CACHE_TTL`
environment variable overrides both. Setting it to `5m` removes the `ttl`
from every marker, so the request uses the five-minute cache. The capture
confirmed this; the value `300` didn't change anything. A one-hour write
costs twice the input rate, and a five-minute write costs 1.25 times, so
this setting is the lever behind the "five-minute cache" item in the
analysis. Coder One doesn't set it yet.
