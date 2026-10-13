# Film review: notes at timestamps

`scripts/film/review.py` is a local page for reviewing video cuts. The owner
plays a video, jumps to a moment, and leaves a note: they type it, or speak
it with their own dictation tool into the note box. Every note keeps the exact
frame, so agents can see what a note means without guessing.

## Run it

```
scripts/film/review.py                       # opens ~/Downloads/episode-290-draft.mp4
scripts/film/review.py ~/Downloads/x.mp4     # opens another video first
```

Then open http://127.0.0.1:8791/. It only listens on this Mac. The video
picker lists recent videos in `~/Downloads` and `~/work/film-out`.

| Key | Does |
| --- | --- |
| Space | Play or pause |
| ← → | Back or forward 1 s (Shift: 5 s) |
| , . | Back or forward one frame |
| N | Pause and open a note at the current time |
| Enter | Save the note (Shift+Enter for a new line); Esc cancels |
| [ ] | Jump to the previous or next note |

Notes show as yellow marks on the bar under the video and in the list on the
right. Clicking a note or a mark jumps to it. Notes can be edited or deleted.

## Where notes go (for agents)

`~/work/film-out/reviews/<video>-<hash>/`, which is outside git:

- `notes.md`: each note in time order, with its frame paths. **Read this before
  changing a video the owner reviewed.**
- `notes.json`: the same for scripts. `t` is in seconds into that exact file.
- `frames/<t>.png` is the exact frame; `frames/<t>-strip.png` shows one second either
  side at 4 fps.

A note's time only matches the file it was made on. When a new cut moves
things, map each note to the new cut by its content, and say which notes the
new cut addresses.
