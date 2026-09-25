# Inspect files from a transcript

Click an underlined file path in a Gym transcript to open its retained evidence.
This works in the ordinary Runs transcript and on either side of head-to-head
replay. Paths in commands, reads, and edits are clickable. A click outside a
path keeps the existing step selection and expansion behavior.

The viewer reads saved evidence. It does not execute a recorded command, call a
model, or substitute a file from your current checkout. Head-to-head playback
pauses while the file is open. Press Esc to return to the same transcript
position; press Space to resume replay.

| Key | Action |
| --- | --- |
| Up/down arrows or `j`/`k` | Scroll one row. |
| Mouse wheel | Scroll three rows. |
| Page Up/Down | Scroll one page. |
| Home/End or `g`/`G` | Go to the beginning/end. |
| Tab, right arrow, or `]` | Show the next retained observation or snapshot. |
| Left arrow or `[` | Show the previous observation or snapshot. |
| Esc | Close the viewer and return to the transcript. |

File text wraps to the available width and keeps line numbers. Continuation
rows preserve the source line. Code, Markdown source, read output, and patches
stay literal so that source syntax is not mistaken for display formatting.
Terminal control characters are escaped. The raw-record replay view (`d`) keeps
its existing literal display; file links appear in the readable transcript.

## What the displayed version means

The viewer first shows the latest matching observation at or before the clicked
step. Its heading identifies the source step and evidence type:

- **Read output** is what the agent received. It can be a region or a truncated
  response, rather than the whole file.
- **File-read command output** comes from a successful single-file `cat`, `head`,
  `tail`, or numeric `sed -n` print command. Compound commands are not treated as
  one file because their combined output can have several sources.
- **Write requests** preserve the requested contents. They do not by themselves
  prove that the write succeeded.
- **Edits and patches** preserve the recorded change or failure. They are not
  reconstructed complete files.

An earlier observation is evidence from that earlier step. It does not prove
that no intervening command changed the file. Later transcript observations
are excluded from the viewer opened at an earlier step.

The viewer also finds files in the episode's artifact inventory, retained
Microluna sequential candidates, and Harbor's final artifact export. Each
version names its source file. A recorded SHA-256 must match before the file
is displayed. Relative workspace paths are resolved only against the run's
recorded working directory; absolute container paths are mapped through the
retained candidate or artifact metadata.

Candidate selection records do not record an exact snapshot capture time.
Final exports describe the completed run. These versions therefore require an
explicit Tab selection and say that they may be later than the clicked step.
If no earlier observation exists, the initial view explains that gap instead
of showing a later snapshot as historical state. Public Fable transcripts can
supply recorded reads and edits, but this feature does not invent unpublished
workspace snapshots.

If the run retained no matching evidence, the viewer says so. It also reports
missing files, digest mismatches, binary or non-UTF-8 files, and oversized files.
Reads are limited to 2 MiB per file; metadata is limited to 16 MiB. It refuses
parent traversal and symlinks in retained references. The viewer offers at most
64 transcript observations and 128 total evidence versions, plus a missing-at-step
notice when needed. Ambiguous retained artifact suffixes remain separate,
explicitly named versions.

## Verification

Issue [#9594](https://github.com/OpenAgentsInc/openagents/issues/9594) tracks this
feature. Tests cover both transcript views, pause and return behavior, historical
reads, snapshot selection, hashes, path bounds, Unicode hitboxes, long paths,
spaces, wrapping, scrolling, and resizing.

Run the retained TB4 acceptance explicitly from the repository root:

```sh
GYM_FILE_VIEW_AUDIT_DIR=/tmp/gym-file-viewer \
  cargo test -p gym --features tui file_viewer_retained_acceptance -- \
  --ignored --nocapture
```

It reads `session-window-debug__3KVqBUz`, opens `/app/app/gc.py`, and checks both
candidate copies and the final export against their recorded hashes. It saves
the initial missing-at-step screen, the explicitly selected candidate screen,
and a JSON inventory. No benchmark or model is rerun.

The [September 24 verification record](measurements/2026-09-24-file-viewer/README.md)
retains the scoped Rust gate, real TB4 artifact checks, and native terminal
click-through screens.
