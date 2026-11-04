# Audio Lab

A Verse page at https://openagents.com/worlds/audio-lab, not listed on the worlds page. It lists every Everglade sound by name and lets you hear each candidate for it, side by side.

## Controls

- Select a candidate's chip to play it: S is the synthesized sound Everglade used to play, E1 to E3 are generated with ElevenLabs, and K1 and K2 come from Kenney's CC0 packs.
- Select A/B to play the synthesized sound and then the first pick.
- The row shows the candidate you last played: its source, length, loudest 400 ms in LUFS, peak in dBFS, the judge's score and its waveform. A lit chip is one of the sound's picks.
- Select KEEP or DROP to put the shown candidate in or out of the sound's picks. Everglade picks one of its picks at random each time it plays the sound.
- Select COPY PICKS to copy the picks as lines for `aethyr/audio/everglade-sfx.picks`.
- Select RAID MIX to play a 20 second raid through Everglade's mixer: 30 raiders casting, hits, a house falling, your footsteps, the horn and the town's bed. The header shows how many voices play and how many the limits replaced.
- Select STOP, or press Escape, to stop everything.
- Select EVERGLADE to go to [Everglade](https://openagents.com/worlds/everglade).

## How it is made

- `apps/openagents-web/audiolab.coax` is the page, and `audiolab-face.c` binds it for the browser.
- `aethyr/audio/everglade-mix.coax` holds each sound's voice limit, level and ducking, and turns a cue into the host's line. Everglade and the raid mix both play through it.
- `aethyr/audio/mix.coax` is the mixer: voice limits that replace the quietest or farthest voice, pitch and gain variation, a priority for your own sounds, the bus limiter, ducking and the room's reverb.
- `aethyr/audio/sfx-tool.coax` generates, trims and normalizes the candidates, and `aethyr/audio/measure.coax` measures them. `aethyr/audio/SOURCES.md` lists where each sound comes from and its licence.
