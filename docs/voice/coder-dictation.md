# Local dictation in Coder for Mac

**Integration status:** the owner-approved bounded 2,097,152-work source
profile builds the current Mac app with the recorded focused development
compiler. Recorded-audio keyboard/mouse hold, exact draft insertion, Undo and
zero-send checks pass. The [adoption record](../verification/coder-dictation-2026-11-02/adoption/README.md)
identifies the current producers; the [historical handoff](../verification/coder-dictation-2026-11-02/app/README.md)
retains the earlier isolated diagnostic. Real microphone and macOS permission
checks remain pending. This is not a normal compiler bootstrap or full release
qualification.

Coder's composer in OpenAgents for Mac accepts speech through Psionic's own
original-weight Parakeet v3 engine. Speech becomes an editable draft at the
caret. It never presses Enter, submits a message or runs a command.

## Use it

1. Install the pinned model using [Psionic's setup guide](../../psionic/docs/parakeet.md).
   The default is `~/.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99`;
   `OPENAGENTS_PARAKEET_MODEL` selects another copy of those exact files.
2. Use the app bundle and current-source compiler identified in the adoption
   record. To rebuild, run `apps/openagents-desktop/build.sh` with `COAX`
   selecting that compiler. Open
   `apps/openagents-desktop/build/OpenAgents.app` when ready.
   In the **OpenAgents** menu, turn on
   **Enable Local Dictation — Parakeet**, then grant macOS microphone access.
   The setting defaults off and is saved locally.
3. Choose **OpenAgents → Dictation Input…** to use the system default or a
   specific input device. The choice is saved locally by stable CoreAudio UID.
   An unavailable selected input is an error; it never silently switches to
   another microphone.
4. Wait for the initial model load. Hold **⌘D**, speak, then release it.
   Holding the microphone below the composer does the same thing.
5. Review the inserted draft and send it yourself. Undo removes the insertion
   as one edit. Escape cancels a pending dictation.

The system's microphone indicator stays on while dictation is enabled and
this app's composer window is active: a short in-memory pre-roll protects the
first word. Switching apps or windows stops capture, clears audio and cancels
the request. The loaded model stays resident across focus changes; disabling
dictation or quitting closes it. Permission denial, missing model data, an
unsupported input format or inference failure is shown by the composer.

## What the implementation guarantees

Coax owns consent, focus admission, tickets, the timeline, PCM conversion,
Verse's resampler and speech/level measurements, segment bounds and text
joining. Swift handles AppKit events, microphone permission/capture and worker
scheduling. Psionic owns recognition, with its Metal encoder and exact
row-parallel CPU decoder. Audio never enters a hosted provider, an audio file,
telemetry or a transcript store.

The timeline keeps up to 450 ms before the press, ignores holds shorter than
300 ms, includes up to 200 ms after release and caps the complete captured clip
at 60 seconds. A level gate and at least 160 ms of speech must pass before
inference. Empty ASR text is also “No speech detected.” The controller reuses
Verse's Coax resampling at exactly 16, 24, 44.1 and 48 kHz; the host transports
channel zero of the selected input (planar or interleaved float32, 1–32
channels). Other rates, including fractional near-matches, refuse explicitly.
Device/configuration changes stop capture, clear old audio and invalidate the
request. Choose an available input again to rearm; returning to **System
default input** is an explicit choice. Late old-device callbacks and in-flight
recognition cannot publish a draft after the switch.

Parakeet's per-call bound remains 327,520 samples. Longer clips are divided
into at most three independent model requests, with exact coverage and no
omitted or repeated samples; their text is joined only after all succeed.
This is bounded offline segmentation. Cross-boundary word quality is not a
full-context or streaming equivalence claim.

A result belongs to its original chat, ticket, text, selection and send count.
Editing, moving the selection, using IME, resetting or leaving the chat, losing
focus, disabling dictation and cancellation invalidate it. A stale completion
cannot consume a newer request. The whole joined insertion must fit; a refusal
leaves the draft and undo history untouched. Newlines are literal text, and
neither a successful transcript nor an error reaches the send path.

## Verification and limits

[Device selection and route-change checks](../verification/coder-dictation-devices-2026-11-02/README.md)
cover injected devices and actual PCM buffer layouts. Physical device removal
and permissions remain owner acceptance.

[The dated record](../verification/coder-dictation-2026-11-02/README.md)
keeps focused policy mutations, native O0/O2 and sanitizer checks, asynchronous
host-effect cases, the real resident Metal recording and offscreen app checks.
Tests use public recordings or injected capture/permission effects; they never
open the owner's microphone or a visible window.

Repository vocabulary rescoring and spoken formatting exist for
`openagents dictate` (`--terms FILE`, `--format code`; [record](../verification/dictation-vocab-2026-11-03/README.md)).
The Mac app does not use them yet. The [Hex proposal](2026-11-02-coder-voice-from-hex.md)
still has later work: partial
text while speaking, streaming and measured key-release latency. The terminal
UI's toggle gesture (below) and the standalone `openagents dictate` command are separate.
No 300 ms dictation-latency or representative multilingual-quality claim is made.

## In the terminal

Coder's terminal UI (`coder/tui/coder.sh`) has the same final-only dictation
(GitHub #12668). Terminals don't reliably report when a key is released, so
it is a toggle, not a hold.

1. Build the CLI: `apps/openagents-cli/build.sh`. Coder runs
   `apps/openagents-cli/build/openagents dictate --events` as a helper
   (`CODER_DICTATE` names another copy). The microphone and the model live in
   that helper, never in Coder.
2. Turn dictation on: start Coder with `--dictation`, set
   `CODER_DICTATION=1`, or type `/dictation on`. It is off by default.
3. Press **Ctrl+Space**. The row above the composer shows the model loading,
   then **Listening**. Speak, then press **Ctrl+Space** again. **Esc**
   cancels.
4. The words go into the draft at the cursor, with a space to join them to
   the words around it. A line feed stays text in the draft. Read the draft,
   then press Enter to send it yourself. **Ctrl+Z** (or Ctrl+_) takes the
   dictated words back while nothing else has changed.

The gesture is configurable: `--dictation-key` or `CODER_DICTATION_KEY` takes
`ctrl-space` (default), `ctrl-g`, `ctrl-o`, `ctrl-t` or `ctrl-x`. Use another
key if your terminal sends nothing for Ctrl+Space. `--dictation-args` or
`CODER_DICTATION_ARGS` passes the helper's `--device`, `--capture`, `--input`,
`--model-dir`, `--format` and `--terms`, as `FLAG VALUE` pairs.

While a request is open:

- Enter, Esc, Ctrl-C, Ctrl-D and a focus-out report cancel it and do nothing
  else. Enter doesn't send the draft; press it again to send.
- Any other key cancels it and then does what it always does. This includes
  typing, moving the cursor, pasting, Undo and Shift-Tab to another agent.
- Coder asks the terminal for focus reports. Where the terminal sends them,
  switching away cancels the request.
- A result that arrives after the draft, the cursor, the send count, the
  agent or the session changed is dropped.

No speech, an error (permission, model, device, recorder, capture, format,
recognition), a transcript over 8000 bytes or one that doesn't fit the draft
leaves the draft as it was and sends nothing. The composer row says which.
Error text is Coder's own fixed wording; nothing the helper prints reaches the
terminal.

The rules are `#authority` in `coder/tui/dictation.coax`: the gesture, what
each key means, the phases, the result kinds and the guard. The guard admits
only valid UTF-8 whose one control byte is a line feed, and joins words with
the Mac's rule (`apps/shared/dictation-join.coax`). The effects are in
`coder/tui/dictate.coax`. [Record](../verification/coder-terminal-dictation-2026-11-03/README.md).
Partial text while speaking waits for streaming (#12673).
