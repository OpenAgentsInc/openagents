# The Haunted House: the first-person shell and its tenant (#12474)

`haunt-web` draws the `haunt.v1` house in a browser with WebGL2, and
`icecream-live --game haunt` publishes it as the `haunt` livehost tenant.
Everything here ran on the owner's Mac in headless Chrome under
`openagents browser run`: a local build at `/` for the walk, and the local
livehost host (`127.0.0.1:8796`) for the live change and the first play.

## The recorded run

`run.mp4` (and its stills, `run-sheet.jpg`): the built-in walker
(`#walk`) arrives in the yard facing south, away from the house, down the
cobbled road between the trees. The creak and crow play behind at 2 s and
the bass line enters; it turns around to the lit house, then walks the
yard, the entry hall, the parlor, the long hall, the kitchen and the back
hall, and leaves by the back door (`ended.png`). The page reported:

- areas walked: yard, entry hall, parlor, long hall, kitchen, back hall;
- fired: the arrival cue, the entry hall's footsteps, the parlor's candle
  dip, the long hall's lightning and banging door, the back hall's chains
  and sheet ghost;
- about 21 s from arrival to the back door at walking speed (a person
  lingers; the spec's 3 to 5 minutes is a playtest target).

## Vignette, lantern, gentle

`gentle.jpg`: the same view with the default vignette (0.75, plum) and
with the gentle setting (G, or the menu), which softens the vignette and
brightens the lantern and the ambient light.

## A light change reaches a running page

`parlor-light-live.jpg`: a page standing in the parlor, before and after a
deploy that changed only the parlor's preset from `candle` to
`jack-o-glow`. The page acked the new version 81 ms after the deploy
started, applied in 16 ms, fetched 13 KB in 3 files (the parlor's chunk,
the index and the manifest), and kept the same walk with no reload.

## First play

The deploy's budget check (the host's pre-publish check): first play is
1103 KB uncompressed in 14 files of a 3417 KB budget (1462 KB in all): the
page, the module, the house settings, the yard and the entry hall, and
their meshes. A fresh browser opening `/t/haunt/` was playing at 330 ms,
having moved 382 KB; the other four areas streamed in after.

## Limits

- Local only; production serves no game tenants yet. The website also
  serves the build at `/games/the-haunted-house`, following each signed-in
  account's Game Lab copy (#12476).
- Headless Chrome; frame rates on phones are not measured here.
