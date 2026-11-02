# Microcoder replay fixture

Two responses recorded from Vertex Gemini Flash on 2026-11-02 with
`GW_RECORD`, for the task:

```text
Run `echo replay-check` and then tell me what it printed, in one short sentence.
```

`1.http` is the first step's `next_action` (run `echo replay-check`), and
`2.http` the finishing step ("It printed replay-check."). Each is what curl
printed with `-i`: the status line, the headers and the event stream. The
smoke replays them with `GW_REPLAY` and no credentials, twice, and checks
that the two session logs match byte for byte apart from the session's ID,
times and folder. See Coder book chapter 9.

To record again:

```sh
GW_RECORD=/tmp/rec coder/tui/coder.sh run "TASK" --engine micro --json
```
