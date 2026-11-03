# Coder terminal dictation (#12668), 2026-11-03

Phase D1 of the [Hex proposal](../../voice/2026-11-02-coder-voice-from-hex.md):
Coder's terminal composer takes local Parakeet dictation as an editable draft
and never sends it. The [guide](../../voice/coder-dictation.md#in-the-terminal)
says how to use it.

All checks ran on Linux (RTX 4080) in a headless pseudo-terminal. No
microphone was opened, no sound was played and no window was shown.

## What was built

| Piece | File | Role |
| --- | --- | --- |
| Rules | `coder/tui/dictation.coax` (`#authority`) | The gesture, what each key means during a request, phases from the helper's status lines, result kinds, fixed error text, the overdue limits, the helper's extra arguments, and the guard |
| Join rule | `apps/shared/dictation-join.coax` (`#authority`) | One space between words, never next to whitespace, an opening bracket or closing punctuation. Moved out of Coder for Mac's guard, which now calls it, so both composers join the same way |
| Effects | `coder/tui/dictate.coax` | Starts `openagents dictate --events` as a helper, writes Enter to stop and Esc to cancel, reads its lines, stops it, draws the status row, writes the trace |
| Composer | `coax/lib/tui/tui.coax` | Ctrl+Space and the other free control keys as `TUI_K_CTRL_KEY`, Undo (Ctrl+Z, Ctrl+_), focus reports, an editor generation, and one undoable insertion |
| Wiring | `coder/tui/coder.coax` | `--dictation`, `--dictation-key`, `--dictation-args`, `/dictation on\|off`, the send counters, navigation count, the hint |
| Helper | `apps/openagents-cli/dictate.coax` | Each JSON line flushed as written; the recorded fixture can play in real time (`OPENAGENTS_DICTATE_FIXTURE_PACE=1`); the stand-in text holds up to 8200 bytes |

Capture, the shared controller (`apps/shared/dictation.coax`) and the
resident engine are `openagents dictate`'s, unchanged in behaviour; Coder
holds neither the microphone nor the model.

## The guard

A final transcript goes into the composer only when all of these are what
they were when the request began: the draft's bytes, the cursor, the editor's
generation (it counts every key and change, so typing then deleting still
counts), the send count (`cd_submit` calls), navigation (agent switch,
`/clear`, `/resume`), focus and consent. The text must be 1 to 8000 bytes of
valid UTF-8 (shortest forms, no surrogates, at most U+10FFFF) with no control
character but the line feed: no NUL, CR, tab, Esc, DEL or C1 control. It goes
in with `tui_ed_insert_undoable`, one edit that Ctrl+Z takes back while
nothing else changed. A transcript that doesn't fit leaves the draft whole.
No code on this path calls `cd_submit`, `cd_start` or `tui_ed_key`.

## Checks

`coder/tui/dictation.coax`: 13 checks, plus the 14 of `tui.coax` (two new:
the new keys, and the undoable insertion) and the join rule's 1 it loads:
28/28 passing.

Mutation, root-only, zero survivors:

| File | Mutants |
| --- | --- |
| `coder/tui/dictation.coax` | 443/443 killed |
| `apps/shared/dictation-join.coax` | 48/48 killed |
| `apps/openagents-desktop/zeron/dictation.coax` (now calling the shared rule) | 170/170 killed; 52/52 checks |

The first run of the rules left 10 survivors. Seven came from redundant code
(a capacity pre-check that `tui_ed_insert_undoable` repeats, an argument count
bound that parity already implied, a UTF-8 lead bound the code-point bound
repeats); that code was removed. Three were missing cases (a two-byte lead with
bit 4 set, U+DFFF, U+009F), which were added.

### Through the real command route

`coax/web/tool.sh coder/tui/dictation_check.coax model` runs Coder
(`coder/tui/coder.sh --engine echo`) in a 100x30 pty, types keys, reads the
screen through `coax/lib/tui/vt.coax`, and starts the real
`apps/openagents-cli/build/openagents dictate --events` with the recorded
backend (`verse/voice-host/fixtures/wait-stop.wav`, paced in real time). Coder's
own Enter stops the helper and its own Esc cancels it. `pw-record`, `parec`
and `arecord` stand-ins that fail at once come first on `PATH`.

Every case reads `CODER_DICTATION_TRACE`, which records, per request event,
`submits` (`cd_submit` calls), `enters` (Enter keys that asked to send),
`starts` (turns and commands started) and `turns`, never words.

| Case | What it shows | No-submit counters |
| --- | --- | --- |
| insert | "Please inspect" + Ctrl+Space, 3 s, Ctrl+Space: the draft becomes "Please inspect the code." / "send" / "press enter" (41 bytes, three lines); Ctrl+Z takes it back | 0/0/0/0 at insertion and at exit |
| gesture | `--dictation-key ctrl-g` starts and stops; Ctrl+Space then does nothing | 0 |
| consent | Without `--dictation` the gesture says how to turn it on and starts no helper | 0 |
| enter | Enter while listening cancels and sends nothing; the next Enter sends the typed draft | 0 at the cancel; 1/1/1/1 after the deliberate send |
| esc | Esc cancels; 3 s later still no insertion | 0 |
| typing | A key while listening cancels the request and is typed | 0 |
| stale | Stop and an edit in one read: the helper's answer arrives and is dropped | 0 |
| undo | Undo during a second request ends it and takes back the first insertion | 0 |
| navigate | Shift-Tab to another agent cancels the request | 0 |
| focus | A focus-out report (CSI O) cancels it | 0 |
| silence | 4 s of digital silence: "No speech detected", draft kept | 0 |
| failure | Device fault at open, recognition fault after the stop: draft kept | 0 |
| overflow | 9000 pasted bytes plus an 8000-byte transcript: refused, draft 9000 bytes; an 8001-byte transcript: the helper's `too_long` | 0 |
| model | Pinned Parakeet v3 on CUDA recognizes the recording: the draft becomes "Heard: Wait, stop." | 0 |

Result: 14 passed, 0 failed, in 82 s, including the GPU case.

The CLI's own `apps/openagents-cli/tests/dictate-check.coax` (no model):
25/25. `python3 coder/tui/smoke.py`, the existing offline harness: all 26
terminal UI checks pass (composer, Shift-Enter, paste, history, Shift-Tab,
themes, resize, Ctrl-C). Three of its non-TUI checks failed (plugin
capabilities, `--on ssh`, and `credential`, where it stopped). None of them
touches the files changed here.

## Limits

- Final text only. Partial text while speaking waits for streaming (#12673).
- Focus loss is seen only where the terminal sends focus reports.
- Some terminals send nothing for Ctrl+Space; `--dictation-key` picks another.
- No real microphone, no macOS build (`dictate-mac.m` is still uncompiled
  here). Both are in `NEEDS_OWNER.md`.
