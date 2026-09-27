# Build 53: the plaza as a neon stage

Coder **0.5.0 (53)** is uploaded and available in internal TestFlight.
[App Store Connect](testflight-build53.json) confirms `VALID` and
`IN_BETA_TESTING`. The signed archive comes from clean source commit
`f3bb5b36ab` on `main` ([upload receipt](upload-build53.json)), which draws
the plaza through the physical renderer in its own amber palette
([#9808](https://github.com/OpenAgentsInc/openagents/issues/9808)): glowing,
antialiased ladder-colored lines with bloom, a polished black floor that
mirrors the city with Fresnel reflectance, fog in the physical path, and a
hue-preserving tone curve. It keeps build 52's physically based Lagrange 1.

Before archiving, the optimized simulator app passed
`ProductionLaunchUITests` and the Lagrange zone UI test on a dedicated iPhone
17 Pro simulator ([log](native-tests.log), [plaza](plaza-simulator.jpg)). The
build was archived and uploaded on the owner's Mac with App Store Connect
credentials read from the protected environment file and never printed. Push
stays off. Physical-device acceptance is not recorded here.
