# Build 51: whole-screen touch look and bottom movement stick

Coder **0.5.0 (51)** is uploaded and available in internal TestFlight.
[App Store Connect](testflight-build51.json) confirms `VALID` and
`IN_BETA_TESTING`. The signed archive comes from clean source commit
`63fe7dbd5ab516433edda2a06f96e385e8e5eb4d` on `main`, which includes
[#9751](https://github.com/OpenAgentsInc/openagents/pull/9751):
touch look uses the whole surface, a translucent stick above the
bottom-left safe area moves the player, and pinch still zooms.
Bundle verification passed (`status: passed`, retained in
`~/coder-ios-build51/archive-bundle-verification.json` on the Mac) and the
[upload receipt](upload-build51.json) records the export with destination
`upload`.

The build was archived and uploaded on the owner's Mac through
`openagents computer exec`, with App Store Connect credentials read from the
Mac's protected environment file and never printed. Push stays off, as in
build 50. Physical-device acceptance of the new controls is not recorded here;
`cargo test -p coder-mobile` (83 tests) covers the stick geometry and look
behavior.
