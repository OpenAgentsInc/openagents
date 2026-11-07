# Everglade arrival implementation

The local arrival slice uses `/worlds/everglade?arrival=1&raid=preview&offline=worker`.
It holds the room socket behind Everglade's existing CHARACTER picker, carries its
skin/cloak/trim choices in the existing Ranger look, and enters on the commons
facing the South Gate. The protected spawn and raider separation use that same
location. The first positive server-reported hit by the player on a cult enemy
shows a six-second acknowledgment once per page session. It does not grant XP
or fabricate a kill.

This is the existing Everglade Ranger picker. The standalone Character Create
Studio's CC2 modular wardrobe is not integrated into the Everglade renderer or
its multiplayer appearance contract by this slice. Do not present the local
picker recording as evidence of that integration.

The simulated session is a Coax tool using trusted CDP keyboard and mouse input:

```
coax/web/tool.sh everglade-arrival-session \
  'http://127.0.0.1:18777/worlds/everglade?arrival=1&raid=preview&offline=worker' \
  /tmp/everglade-arrival.mp4 /tmp/everglade-arrival-frames
```

It records two minutes at 1280×720 and 12 fps. It configures a small introductory force through the existing local admin controls
(size 4, cult/fanatic/warlock mix 10/0/0, no warlocks, leader aggression 0),
before its boot/start control messages are delivered, so the event snapshots that
mix from the beginning, with ordinary damage and progression rules. It selects appearance swatches,
accepts entry, walks toward the gate, and aims Ray of Frost at actual visible
cult enemies through read-only projection probes. Success requires the game's
room-confirmed contribution flag. The recorder never injects a hit or edits
combat state. The local room runs on the capture clock, and Chrome is headless
and muted. A failed run retains frames and reports failure.

The movement failure in the earlier clips came from using DOM-created keyboard
events. Trusted `Input.dispatchKeyEvent` with the actual key/code moves the
room's player and survives reconciliation; a diagnostic measured the player's
travel. No change to movement rules was required to make the simulated input
work.

## Verification

The world and offline-room Wasm builds passed with their embedded checks. Targeted
checks also covered the commons spawn/facing, raid clearance and obstacle routing,
character acceptance preserving the chosen Ranger look, and first-hit rejection
for misses and other players' hits.

A full two-minute player-perspective run passed the recorder's gameplay assertion:
the local room reported `e hit 2024 1000 6 3` (the player's 6 cold damage), the
contribution flag became 1, and a retained frame at 54 seconds showed
`YOU HELPED DEFEND EVERGLADE` and 50 XP. The run includes ordinary combat damage,
falling and the room's automatic return to standing; the recorder grants no damage,
health or XP. The player bot returns toward the village after confirmation.

The simulation starts with the local Ranger picker and uses a configured local
raid. It is evidence for that supported slice, not for the standalone CC2 Studio
handoff or a production deployment.
