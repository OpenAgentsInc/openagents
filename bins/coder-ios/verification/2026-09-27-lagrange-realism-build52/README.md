# Build 52: physically based Lagrange 1

Coder **0.5.0 (52)** is uploaded and available in internal TestFlight.
[App Store Connect](testflight-build52.json) confirms `VALID` and
`IN_BETA_TESTING`. The signed archive comes from clean source commit
`ca54eab66d` on `main` ([upload receipt](upload-build52.json)), which
includes the Lagrange 1 realism roadmap
([#9803](https://github.com/OpenAgentsInc/openagents/issues/9803)): a
physical renderer path with real-unit sunlight, shadows with a true
penumbra, bounce light, measured materials, the Sun, Earth, Moon, and
catalogue stars at infinity, bloom and metered exposure, rope tethers,
flexing arrays, plume and ice-flake glints, and a 30° station pitch. A new
**Art** control switches to a brighter camera preset.

Before archiving, the optimized simulator app passed
`ProductionLaunchUITests` and `ZoneUITests/testLagrangePortalEntersStationFliesAndReturns`
on a dedicated iPhone 17 Pro simulator ([log](native-tests.log)); the physical
path compiled and rendered on iOS Metal
([station](l1-station-simulator.jpg), [flight](l1-flight-simulator.jpg)).
The build was archived and uploaded on the owner's Mac with App Store Connect
credentials read from the protected environment file and never printed. Push
stays off. Physical-device acceptance is not recorded here. Desktop captures
are in [`docs/verse/captures/lagrange-1-realism/`](../../../../docs/verse/captures/lagrange-1-realism/README.md).
