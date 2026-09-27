# Build 50: Ruins and Lagrange 1

Coder **0.5.0 (50)** is uploaded and available in internal TestFlight.
[App Store Connect](testflight-build50.json) confirms `VALID` and
`IN_BETA_TESTING`. The signed archive comes from clean source commit
`c7451dfd71763f8dde65903b3102acd1a9cad2ae`; [bundle verification](archive-bundle-verification.json)
passes.

- The Atlantis zone is renamed **Ruins** in the game and its identifiers
  (`ruins`, `ruins-v1`, `ruins.wizard-woods.v1`, `verse-ruins`). Its pack bytes
  are unchanged and now load from `assets/verse/ruins/`.
- A second plaza arch leads to **[Lagrange 1](../../../../docs/verse/lagrange-1.md)**,
  a construction station on a controlled Lissajous orbit about Sun–Earth L1.

## Evidence

- [Native zone tests](native-zone-tests.log): on an iPhone 17 Pro simulator,
  `ZoneUITests` passes Lagrange 1 entry, EVA autopilot flight, and return;
  Ruins download from the new path, real-time combat, and return; and the
  notices check.
- Rust: 212 Verse tests, 68 mobile tests, 12 Ruins adapter tests, and 11
  `verse-lagrange` physics tests pass. The retained-source check verifies 198
  files.
- Offline renders of the station from the shared renderer:
  [spawn](l1-spawn.png), [keel jig](l1-jig.png), [the Sun](l1-sun.png), and
  [the Earth with its locator](l1-earth.png).

These are simulator and offline observations, not physical-device acceptance.
