# Microcoder journal AJR1

Microcoder's worker keeps a durable journal of every model request, reply,
command and decision: `~/.openagents/coder/sessions/worker-US.ajr`. Replay
reads it back. One file writes and reads a frame:
[`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
(`aj_prepare`, `aj_read`, `aj_commit`).

- Profile and goldens: [`proto/coder/microcoder-journal-ajr1.json`](../../proto/coder/microcoder-journal-ajr1.json)
- Check: `coax/web/tool.sh coax/lib/agent/journal-profile.coax proto/coder/microcoder-journal-ajr1.json`
- Registry row: `microcoder-journal-ajr1` (step C11, [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250))

## A frame

The file is frames, one after another, with nothing between them. All
numbers are little-endian.

| Bytes | Field | Meaning |
| --- | --- | --- |
| 0 to 3 | magic | `AJR1` |
| 4 to 11 | sequence | `u64`. 0 for the first frame, then one more for each frame |
| 12 to 19 | group | `u64`. The sequence of the frame this one belongs with (a reply's request, an output's command). Never past its own sequence |
| 20 | kind | `u8`. See the table below |
| 21 to 24 | length | `u32`. The payload's bytes: at most 32,768 |
| 25 to 56 | previous | The digest of the frame before. 32 zero bytes for the first frame |
| 57 on | payload | `length` bytes. Any bytes, NUL included |
| last 32 | digest | SHA-256 of every byte of the frame before it |

A frame is 89 bytes plus its payload.

| Kind | Name | Payload |
| --- | --- | --- |
| 0 | `context` | The session's context |
| 1 | `model_chunk` | Part of a model request. A large request is ordered chunks |
| 2 | `model_intent` | The model about to be asked, written before the request is sent |
| 3 | `model_reply` | Part of the model's reply |
| 4 | `model_end` | The reply ended |
| 5 | `command_intent` | A command about to run |
| 6 | `view_intent` | A file about to be read |
| 7 | `tool_output` | Output of a command or a view |
| 8 | `tool_end` | The command or view ended |
| 9 | `stop` | The session stopped |
| 10 | `decision` | A decision model's answer |

No payload holds a credential.

## Rules

- A writer prepares a frame, the host appends and syncs it, and only then
  does the chain move on (`aj_commit`). A failed append poisons the writer:
  it prepares nothing more.
- A reader refuses a frame whose magic, sequence, group, kind, length,
  previous digest or digest is wrong, and a frame cut short. It never reads
  a truncated tail as a record.
- The format has no version field other than the magic. A change to the
  layout is a new magic and a new profile.

## What the check holds

- The profile's 11 frames are one chain, one of each kind. They were made
  with Python's `hashlib` and `struct`, not with the codec. `aj_prepare`
  gives each frame's exact bytes, and `aj_read` gives its values and the
  next digest back.
- `aj_read` refuses each of the nine `refused` frames.
- The layout numbers (57, 89, 32,768) and the kinds are the codec's.

To check a journal that a worker wrote:

```sh
coax/web/tool.sh coax/lib/agent/journal-profile.coax --file ~/.openagents/coder/sessions/worker-US.ajr
```

It reads every frame, checks the chain, writes each frame back with
`aj_prepare`, and compares the bytes. On 2026-11-13 the five journals with
frames on the owner's Mac (128 frames, 75,433 bytes, nine of the eleven
kinds) all read and were written back to the same bytes.
