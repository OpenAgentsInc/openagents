# The .cut format

`openagents labs cut FILE.cut OUT.mp4` turns a cut list into a video. A cut
list is a `.cut` file in the line format below, or Video Lab's `cut.v1`
JSON (for example `~/work/film-out/cut/cut5.json`). `labs cut` reads both.
New cuts are written as `.cut` lines. The parser and the checks are in
[labs-rules.coax](../../apps/openagents-cli/labs-rules.coax).

```text
cut EPISODE 291
size 1920x1080
fps 30
music audio/theme.m4a gain=-14 duck=-8
voice audio/vo.m4a gain=0
fade in=0.5 out=2

# the shots, in order
shot open   title="OPENAGENTS" title="Episode 291" dur=3
shot belt   src=clips/belt.mp4      in=0    dur=4.5  xfade=0.5
shot portal src=clips/portal.mp4    in=2.2  dur=6    xfade=0.75 title="THE PORTAL"
shot grid   src=https://openagents.com/realms/orbit-league?stage=highlight  dur=5
shot end    title="openagents.com" under=clips/belt.mp4 under_in=4 dur=3 xfade=0.5
```

Each line is one statement. Blank lines and lines that start with `#` are
skipped. A value with spaces goes in double quotes. Times are seconds, and
can have a fraction. A path is relative to the cut file's folder, and `~/`
is your home folder.

| Line | Meaning |
| --- | --- |
| `cut NAME` | The cut's name. Optional. |
| `size WxH` | The output size, two even numbers from 64 to 7680. The default is 1920x1080. |
| `fps N` | Frames per second, 1 to 120. The default is 30. |
| `music PATH [gain=DB] [duck=DB]` | The music bed. `gain` sets its level in dB. `duck` lowers it further while there is a voice line. |
| `voice PATH [gain=DB]` | The voice-over, mixed over the music. `--captions` transcribes this file. |
| `fade in=S out=S` | Fades the picture and the sound in from black and out to black. |
| `side PATH width=W [in=S] [vf="FILTERS"]` | A picture stacked to the right of the shots, W pixels wide, from `in` on, behind a 2 px rule; for a talking head beside the B-roll. The shots, titles and captions fill the rest of the width. |
| `captions PATH.srt [box=no]` | Captions from an SRT file instead of transcribing the voice. `box=no` draws them with a dark halo and no band. |
| `shot NAME ...` | One shot. See the next table. |

| Shot word | Meaning |
| --- | --- |
| `src=PATH` or `src=URL` | The picture. `labs cut` records a URL with `labs clip` first, into `FILE.clips/` beside the cut, and reuses that recording on later runs. |
| `in=S` | Where the shot starts in its source. The default is 0. |
| `dur=S` | How long the shot is on screen. Required. |
| `xfade=S` | A crossfade from the shot before it. On the first shot, a fade in from black. |
| `title="TEXT"` | A title line drawn in Filament. Repeat `title=` for more lines; each line after the first is half its size. Over a picture, the title sits in the lower third. A shot with a title and no `src` is a title card. |
| `under=PATH [under_in=S]` | The picture under a title card, darkened. Without it, the card is on the Grid's near-black. |
| `vf="FILTERS"` | ffmpeg filters run on the source first, such as a crop or a colour LUT. The shot is then fitted to the frame. |
| `over=PATH [over_in=S]` | A picture laid on the shot with a screen blend, from `over_in` on: its black stays clear, so a HUD drawn on black frames the footage under it. |

## Checks

`labs cut FILE --check` runs these checks and prints each problem as
`problem LINE REASON`. Every render runs the same checks first and makes
nothing when one fails.

- Every source exists, or is a URL.
- `in + dur`, plus the next shot's crossfade, fits inside the source.
- A crossfade is shorter than both shots it joins.
- Every shot has a source or a title, and a duration.
- A title fits the title-safe area, the middle 80% of the width, at the
  cut's size.

## cut.v1 JSON

`labs cut` reads `fps`, `size`, `music`, `voice`, `fade_in`, `fade_out`, and
each shot's `name`, `src`, `in`, `dur`, `xfade` and `title`. A title's
`lines` are drawn one under the other, each at its `size`. Each key that
`labs cut` does not draw yet is printed as `note LINE KEY is not drawn by
labs cut yet`, for example `over`, `push`, a title's colour and timing, and
the script's `captions` lines. Shots are numbered from 1 in place of
line numbers.
