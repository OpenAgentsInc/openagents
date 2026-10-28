# Haunted House port verification — 2026-10-28

Source: aethyr-cpp `69848ca`. App: OpenAgents 1.0.0 (1052).

- Current content: all 120 original objects and their exact transforms,
  21 door IDs, 13 rooms plus three outdoor areas, plain roofs and the
  current pine/dead-tree forest. Official protobuf reader compared these
  to the preserved source document and seeded placement capture.
- Resource: 1,645 instances, 87 meshes, 26 lights, four conversation
  characters and all nine zones. Only the referenced texture slices ship.
- Coax: scene checks, reusable HUD and lighting checks, 44 player checks,
  29 Grid checks. GPU-buffer admission: 109 mutants killed, zero survivors.
- ASan/UBSan: real geometry and generated codecs; invalid geometry refused;
  every scare fired, including delayed thunder and sheet/brew transformations;
  dialogue/teaching, character creation/behaviours, object editing/placement,
  zone changes, grouped frame packets and save/restore exercised.
- Independent oracle: Coax-written saves read by official protobuf; the
  official writer changes title/identity/feedback and Coax restores them.
- iOS simulator 26.5: actual Metal pipelines, Grid portal entry, Mortimer
  conversation, placement, persistence across world changes, all camera modes,
  map, portrait/landscape, reset and return. Existing three Home/Grid UI tests
  also pass. Full-screen device captures verify the landscape viewport;
  cropped XCUIApplication snapshots do not represent a rotated Metal surface.

Local copies and file invitations are implemented. Retired web account,
owner/public-publication, paid change-offer and cloud NPC services are not
restored or simulated. A local save never claims a public publication.
