# Kiln's transcript

Kiln draws its agent run with the OpenAgents app's own transcript: the
desktop's `TranscriptView`, which the web apps already use through
`openagents_web_chat_view::desktop_transcript` (the chat, the demo, the
landers, and the agent card). Nothing is restyled: Kiln hands the run to a
small GPUI module, which maps it onto the desktop's message parts.

## Pieces

- **`apps/openagents/web/kiln-transcript`** (the GPUI web workspace): a
  `cdylib` that mounts the desktop transcript in the page's `#gpui-mount`
  box and exports `mount()` and `show(json)`. It maps each Kiln part onto
  `openagents_doc::MessagePart`:

  | Kiln part | Desktop part | Draws as |
  | --- | --- | --- |
  | `reasoning` | `Reasoning` | the collapsible **Thought process** block |
  | `text` | `Text` | answer text (markdown) |
  | `status` | `Text` with a `notice` id, in emphasis | a quiet line: which Pylon answered, and how long it took |
  | `tool` (`read`, `search`, `exec`, `write`, `edit`) | `Tool` with `ReadFile`, `Search`, `Exec`, `WriteFile`, `EditFile` | the desktop's tool rows, grouped as "Ran 4 commands · edited 3 files"; an edit or write carries a `ToolDiff`, drawn as the desktop's diff |

  A remote `cargo test` is an `Exec` row whose output is the runner's
  summary ("tests passed: 6 passed, 0 failed"), red when it fails. Output
  over 4,000 bytes is cut in the card; the raw log keeps all of it.
- **`kiln::transcript`** (`crates/kiln`): the run as Kiln records it (turns,
  status lines, thinking, tool cards with their text before and after),
  serialized as the JSON the module takes. It also turns a Pylon's notes
  into plain words and keeps toolchain noise (a sandboxed `xcrun`'s cache
  warnings) out of the cards; the raw log shows it muted.
- **The page** imports the module after Kiln starts and hands it the run on
  every change (`window.kilnTranscriptShow`). **Transcript** and **Raw
  log** switch between the two views. Without the module, the page shows
  the raw log.

## JSON

```json
{
  "working": "Asking a Pylon",
  "started_ms": 1792630000000,
  "entries": [
    {"id": "u1", "role": "user", "parts": [{"kind": "text", "text": "make fib"}]},
    {"id": "a2", "role": "assistant", "streaming": true, "parts": [
      {"kind": "status", "text": "DeepSeek V4.1 Flash · Experimental preview · answered in 24.5 s"},
      {"kind": "reasoning", "text": "Find the recency function first."},
      {"kind": "tool", "id": "c3", "tool": "search", "pattern": "fn recency", "path": "src", "command": "grep -n 'fn recency' src", "done": true, "output": "src/lib.rs:24:pub fn recency(hours: f64) -> f64 {"},
      {"kind": "tool", "id": "c4", "tool": "edit", "path": "src/lib.rs", "done": true, "old": "…", "new": "…"}
    ]}
  ]
}
```

## Thinking

Each step's reasoning goes in a thinking block:

- the pylon's own `reasoning` field on a result, when it sends one;
- otherwise a `<think>` block in the answer;
- otherwise the step's stated `thought` from Kiln's reply format.

Since 2026-10-21 a Pylon result carries the model's reasoning in an
optional `reasoning` field (NIP-CJ, NIP-PYLON), capped at 32 KiB and
trimmed from the start. The Psionic engine passes `reasoning_content`
through, and the DeepSeek V4.1 group head passes what it splits off at
`</think>`. Flint on GCP runs it (pylon `b316b703de`); a Kiln run pinned
to it (`?model=flint&pylon=NPUB`) shows Flint's own reasoning in each
step's **Thought process**. The DeepSeek group gets it with its next head
relaunch (head `e411ee8ce2`). Pylons answer a whole result at once, so the
block appears when the answer arrives rather than streaming.

DeepSeek also sometimes answers in its own tool-call markup
(`<｜DSML｜invoke name="view">…`) instead of Kiln's JSON. Kiln reads those
calls as commands.

## Inside the Grid

The Grid's **Kiln** control opens `/admin/kiln/?embed=grid` in the
`#kiln-panel` overlay. With `embed`, or whenever the page is framed, Kiln:

- lets the world show through;
- draws the run as a translucent panel on the right;
- puts its controls in a **Controls** drawer on the left.

The page works the same at `/admin/kiln/` and at `kiln-dev`.

## Size

| Module | Raw | Brotli |
| --- | --- | --- |
| Kiln's page module (`pkg/kiln_bg.wasm`) | 2.5 MB | 1.5 MB |
| The transcript (`transcript/openagents_web_kiln_transcript_bg.wasm`, WebGL2, `wasm-opt -Oz`) | 4.6 MB | 1.4 MB |
| The transcript's glue (`.js`) | 68 KB | 10 KB |

The transcript loads after Kiln starts, so it doesn't delay Kiln's
controls. It's one build, with the WebGL2 renderer; a WebGPU-only build,
like the agent card's, would be smaller.

## Build

- `apps/openagents/scripts/build-web-kiln-transcript.sh OUTDIR` builds the
  module (the GPUI web workspace's toolchain and target).
- `scripts/build-kiln.sh` builds it into the page's `transcript/`;
  `KILN_TRANSCRIPT=0` skips it.
- The web image builds it in the `gpuiweb-graph` stage and copies it to
  `/srv/kiln/transcript`. `openagents-web` serves
  `transcript/openagents_web_kiln_transcript.js` and
  `transcript/openagents_web_kiln_transcript_bg.wasm` to admins under
  `/admin/kiln/`.
