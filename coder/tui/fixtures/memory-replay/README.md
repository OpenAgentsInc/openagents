# Memory recall replay fixture

Three responses recorded from Vertex `gemini-embedding-001` (256
dimensions) on 2026-11-02 with `GW_RECORD`, for one `coder recall`:

1. `1.http`: the document embedding of
   `deploy: Ship openagents.com only through coax/web/release.sh.`
2. `2.http`: the document embedding of `build: Build with coder/tui/coder.sh.`
3. `3.http`: the query embedding of `how do I put the website live?`

Each file is what curl printed with `-i`: the status line, the headers and
the body. The smoke remembers the same two facts in a fresh HOME, replays
these with `GW_REPLAY` and no credentials, and checks that `deploy` ranks
first. See Coder book chapter 19.

To record again:

```sh
coder/tui/coder.sh remember deploy "Ship openagents.com only through coax/web/release.sh." --project
coder/tui/coder.sh remember build "Build with coder/tui/coder.sh."
GW_RECORD=/tmp/rec coder/tui/coder.sh recall "how do I put the website live?"
```
