# Everglade II rest controls, clicked in headless Chrome

Partial #13046 work. Two headless Chromes take the real journey (Character
Create, USE IN EVERGLADE, seated in one local Everglade II room) and each
presses its rest controls with the mouse. The room's answer (`z SEQ CODE DICE`)
decides every step.

## Run

```
coax/web/room/build.sh                       # the room binary, all 1049 room checks
coax/web/wasm.sh apps/openagents-web/everglade2.coax apps/openagents-web/everglade2-face.c \
  apps/openagents-web/build/everglade2.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh coax/web/room/local.coax --room 8957 --page 8958 --static --no-build
coax/web/tool.sh everglade2-crowd 'http://localhost:8958/worlds/character-creator?relay=ws://localhost:8957' \
  --players 2 --seconds 3 --rest --shot DIR
```

`--rest` is new in `coax/web/everglade2-crowd.coax`. It clicks at the control
centres that the new `e2_probe` fields 30 to 39 report (`everglade2.coax`),
then reads the room's rest code back. Each page, in turn:

| Press | Room's answer | Held |
| --- | --- | --- |
| (none) | z 0, SHORT REST and LONG REST show | yes |
| LONG REST | z 3, asleep | yes |
| W for 1.5 s while asleep | still z 3, moved 0 cm | yes |
| WATCH | z 4, on watch | yes |
| END REST | z 0, both controls show again | yes |
| SHORT REST | z 1 | yes |
| W for 1.5 s | z 0, Short Rest broken; walked about 6.3 m | yes |

The whole crowd check holds too (`crowd-rest.log`). `rest-asleep-0.jpg` shows
WATCH and END REST over the bar, with HP 9/9 · ASLEEP.
`rest-short-1.jpg` shows END REST in a Short Rest.

Everglade II Wasm: 643,835 bytes, under the unchanged 660,000 budget
(`wasm.log`).

## Limits

- The page server served a copy of another worktree's `apps/openagents-web/build`
  with this tree's `everglade2.wasm` and `everglade2.vpk3` rebuilt. The site
  wasn't fully rebuilt.
- END REST from Watch ended the rest outright (z 0) here. The check also accepts
  REST BROKEN (z 6) followed by a second END REST.
- No Hit Die press: that needs a finished Short Rest, which is an hour of room
  time. The room checks (`e2 rest`) cover it.
