# Coder live reply stdout v1

`openagents coder run|fork ... --stream-json` opts into live reply output and
implies JSON mode. It retains ordinary session, routing, tool, approval, model
and end records, and replaces stdout's whole `reply` records with:

```json
{"kind":"reply_delta","schema":"coder.reply-stream.v1","segment":0,"text":"Hello "}
{"kind":"reply_delta","schema":"coder.reply-stream.v1","segment":0,"text":"world."}
{"kind":"reply_end","schema":"coder.reply-stream.v1","segment":0}
```

The [schema](../../proto/coder/reply-stream-v1.schema.json) defines the new
record identity. Concatenate deltas in pipe order for one segment. End closes
that segment; a later tool/answer segment uses the next number. Delta text
contains at most 8,192 unescaped UTF-8 bytes, without splitting a code point.
Providers supply their native chunk sizes; a provider which buffers an answer
cannot gain token-level streaming through this adapter.

These events are ephemeral progress, not durable commits or tool effects.
Session files retain exactly the existing whole-reply format for forks,
history and replay. Ordinary `--json` output is unchanged. A disconnected or
crashed writer may leave a displayed partial reply with no durable whole
reply. The terminal `end` remains the turn outcome; `reply_end` alone never
means the task succeeded.

Before live JSON encoding, Coder withholds unfinished prefixes of its known
credential values and redacts complete matches with the existing scrubber.
Escaping happens after redaction. Secret discovery, tool consent and all
existing session permissions are unchanged.

Aethyrforge appends these chunks to its native transcript and to transparent
left-side text while a turn is active. It neither focuses nor opens the
composer in response to a chunk. Enter still opens the conversation on demand.

## Final local speech input

The separate [native IPC schema](../../proto/coder/voice-input-v1.schema.json)
binds a Parakeet final transcript to one native microphone-grant ticket.
Only the current local top-level page may send it. Native foreground/world
readiness, one-shot finality, cancellation and the 16 KiB Coder prompt bound
gate dispatch in `apps/aethyrforge-desktop/voice-coder.coax` (`#authority`).
Partials, stale/duplicate finals, canceled captures and blank input start no
turn. Busy Coder turns are retained, with a visible rejection message rather
than an implicit stop or a queue. Typed drafts are preserved. Accepted final
words call the same automatically routed Coder run/fork path as typed input.
The realm receives only voice status for a consumed native final, so the same
words cannot also trigger a world edit. Audio remains on the local Parakeet
path; only its finalized text goes to the selected coding provider.
