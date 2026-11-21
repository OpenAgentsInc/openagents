# Hosting the real Everglade: what was tried, and why it was removed

2026-11-16. For one afternoon GameBuilder ran the real Everglade (the
`everglade2` realm from the pinned openagents tree) through the engine's page
host, on the Mac and in the iOS simulator. It was removed the same day, along
with Everglade's blockout: the owner is making Everglade from scratch, the way
any new world is made here. The code is in git history: `b30f4cb`, reverted by
`863929d`. This is what it showed.

## What worked

- The realm builds from the pinned tree as a library (`metaverse/native/build.sh
  everglade2`), with its eight shader pairs emitted for Metal and its pack's
  layers as GPU blocks (BC7 on the Mac, ASTC on iOS), for the Mac, the
  simulator and a device.
- The app compiled the page host (`metaverse-engine/host/apple/PageHost.m`),
  the audio host and the deep-stack thread from the pinned tree, and drew the
  realm at 60 frames a second on both platforms, with sound, the site's room
  and touch controls.
- It added about 130 MB to the app.

## What went wrong

The owner found movement on the Mac slow. It was not found why.

Measured, with a debug script that pressed keys through the same calls a real
key makes and read the position from the realm's driver:

- Running covered 4.2 m/s and walking 1.5 m/s, which are Everglade's own
  numbers. Shift walks; running is the default.
- The frame rate held 60, at a small window and a large one.
- The page host skipped 1 to 10 percent of frames, and the distance covered
  was the same whether the skipped time was kept or lost.
- After Character Create, one W press moved the character for about three
  seconds and then it stood still with W down. Repeated presses, as a keyboard
  sends, moved at full speed. Opened straight in the glade, one press held.
- The quality governor fell to LOW at 60 frames a second, where the Everglade
  app sits at MED. Not explained.

The slowness the owner felt was never reproduced by the script. The part the
script could not exercise is pointer capture: the host held the pointer on a
key press and could let it go on the next frame, and the scripts ran with no
mouse and the window in the background. That is the likeliest place, and it is
a guess.

## What it says about the engine

The realm, the engine and the room are the ones the Everglade app runs
correctly, so the fault was in the embedding.

1. Every app writes the same glue by hand: keys, pointer capture, focus and
   frame time are about 200 lines each in the Everglade Mac app, the Everglade
   iOS app, and were again here. The engine should ship one view that owns it.
2. `PageHost.m` returns early from a frame while the GPU still has the last
   one, and the time of that frame is lost. It was small here; on a machine
   that falls behind, the game runs slow by the share of frames skipped.
