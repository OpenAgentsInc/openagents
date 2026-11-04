# Local streaming dictation events v1

`proto/openagents/cli/dictation/v1/stream.proto` defines the semantic fields
of `openagents dictate --stream --events`. Each complete JSON line has
`schema: "openagents.dictation.stream.v1"`. This is a local JSONL projection,
not protobuf's JSON encoding: integer fields are bounded JSON numbers. The
existing `openagents.dictation.v1` final-only output is unchanged. Default
terminal-composer helpers still request the existing final-only mode.

Every event carries `event` (starting at zero), `ticket` (zero before capture),
`sequence` (model pushes submitted), `samples` (model audio submitted, at most
960,000), `profile`, `model`, `revision` and `capture`. The current explicit
profile is `psionic-nemotron-streaming-en-v1`, model
`nemotron-speech-streaming-en-0.6b`, revision
`ebe59e5a817142986528bbbee5dba8db7b38ed50`. It is English RNNT on Linux CUDA.

- `status` adds `state`: loading_model, opening_input, ready, listening,
  finishing or recognizing. Status never carries transcript text.
- `partial` adds a full current `text` snapshot, only after an acknowledged
  current-ticket push. It is provisional display text and must never submit
  a message, trigger a command or become a final/composer insertion.
- `final` adds `text`, `exit: 0` and `limited`. It is eligible once, after
  capture's existing minimum-hold/speech admission, all captured samples,
  explicit model finalization and the existing controller's ticketed delivery.
- `no_speech`, `cancelled` and `error` terminate without transcript text.
  They add `exit`; no_speech adds `reason`, error adds `error` and `message`.

Only complete newline-terminated events count. Cancellation, capture/model
failure and revoked tickets cannot publish a subsequent final. Previously
displayed partials remain provisional and cannot be retracted from an external
reader. A failed/interrupted output may leave an incomplete last line and a
nonzero process exit; discard that line. Cancellation after a final has already
been completely published cannot retract it. Output retries are bounded and
poll control keys/signals, so a reader that stops draining cannot indefinitely
block cancellation or grow an output queue.

Plain streaming mode shows partials on stderr and keeps final text alone on
stdout. `--json`, `--terms`, `--format code`, CPU and macOS streaming are refused
before capture; use `--events` for structured streaming output. No option
silently changes the multilingual offline provider or its vocabulary profile.
Audio consent, input selection, resampling, pre-roll/tail and 60-second capture
limits remain the shared dictation controller's. Partials may precede its final
speech-admission decision; no_speech/cancel/error supersedes them.
