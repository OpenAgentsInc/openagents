# Activity groups: Coder Desktop beside Zeron (#13731)

Coder Desktop draws an agent's work as Zeron's activity group does. The
component is `components/app/activity.coax`; Coder's transcript uses it in
`cp_group` (`apps/coder-desktop/page/layout/conversation.coax`).

## Captures

All captures are 2x Retina, dark, at Coder's usual text size (zoom 1.0).

| State | Zeron | Coder Desktop |
| --- | --- | --- |
| Live: a thought, one command done, one running | `zeron-live.png` | `coder-live.png` |
| Settled, folded | `zeron-settled.png` | `coder-settled.png` |
| Settled, opened by a press on the summary | (the fixture does not take the press) | `coder-settled-open.png` |

Zeron ran its release `zeron-zgx` with a fixture (`ZERON_ZGX_UI=<copy of
apps/zgx/ui> zeron-zgx fixture=...`). Its window is translucent, so the
backdrop shows through it. Coder ran from a local build with the Tier 1 mock
worker in place of `mcworker` and its own home folder.

## What matches (measured in points)

- Row pitch: 28 in both (Coder 27.5, Zeron 28, from the text centers).
- Glyph 15.5 right of the trunk (Zeron's 24 minus 8.5); the verb 40 right of
  it (Zeron's 48 minus 8.5, plus the R's side bearing).
- Header to the thought's first line: 27 in Coder, 27 in Zeron.
- Thought line height 20, folded past 4 lines under "Show all N lines".
- Summary words and order ("Thought process · ran 2 commands"), the working
  word and seconds ("Pondering… 9s"), elbows of radius 6 to x 20, trunk 1 point
  at 0.12 of the ink.

## What differs

- Type: Coder has no 12-point face. The summary and verbs use its 13.2-point
  face (Zeron 12), and thoughts its 14.4-point body (Zeron 14). Text runs about
  10% wider, so the thought wraps sooner in the same column.
- Summary word: Zeron's fixture says "Thought"; Coder says "Thought process",
  as Zeron's transcript (`chat.rs`) does.
- Loader: Coder draws Zeron's eight dashes on a ring; the dash stretch is an
  approximation of Zeron's lead sweep. Zeron's fixture shows no loader because
  it has no running tool.
- Fade: Coder fades a long target per character, not with a gradient mask.
- An opened command shows its whole command and output; Coder keeps no exit
  code number, only failed or not.
- A sub-agent's transcript pane draws its work with the same group (#13744); its tab is Zeron's panel chip (28 tall, the label at most 160, fading).
