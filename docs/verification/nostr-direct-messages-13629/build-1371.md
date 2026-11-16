# Build 1371 artifact record

- Bundle: `com.openagents.app`, version `1.0.0 (1371)`.
- Installed runtime: `50192fe9dcbba0417c9a8bc8d492c50b57112fceffd4d501ffaab1865ce2a635`.
- Exported IPA SHA-256: `04105feb6f421fd2a7f427e95577a668fe2bfdfd513eb51842bb53df5014588f`.
- Retains main’s 1370 composer/transcript styling; supersedes the unuploaded 1361 archive.
- All 21 update resources match the exported IPA byte for byte.

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `index.html` | 688 | `89a6576e44c00991d6ebd3c479209efdf10c387bfa0b3fda2c707ddc879d1364` |
| `runtime.js` | 2527 | `283a2a048e66019b2ffc13503c8e49f124f112f4e27316f2bc19ea6ccba6fcb4` |
| `tree-commands.js` | 1657 | `fe971cb9633f9fd9581dfdbc4c119361701a23948e228b5da5f95a3e56fd6f45` |
| `coaxial/shell.js` | 7184 | `f9d1838e4029f1cf198eec535571585f9f9baf283dd69957c5798398f8b2b72c` |
| `build/app.js` | 8388608 | `e88718e138624c58273f1f176416238bba7a4563ee35d9d1e56896c7cd1a2ca7` |
| `build/app.tail.js` | 2460802 | `40e03ce4960fd7e0882cd8be0a4252372fe9293db46664f9149815e800e56754` |
| `build/extras.js` | 539779 | `758393944f55c95a76405e3ee48bb638f2321beac8ccd05afeaff666b920321b` |
| `build/extras.tail.js` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `build/editor.js` | 3658464 | `13f21e0d80d370f4f7d7dd39338b2e764675e5013af276aff6a8cdb8a496b883` |
| `build/editor.tail.js` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `build/worlds.js` | 663418 | `7e2d65bbbd5953b6dc6c8b68e2d22aee741cab35264657bf6e66355f01dba3a6` |
| `build/worlds.tail.js` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `build/zeron/Geist.ttf` | 126048 | `5c8968eafb98a4c4f47033daf29e38e284a6f2a82eb017d171ab040fe7c4b615` |
| `build/zeron/Geist-Medium.ttf` | 127660 | `0090e004725f6f64b841715b4167920580f883fcf9b67fc6d744089103fec101` |
| `build/zeron/Geist-SemiBold.ttf` | 127872 | `612ec98df33935354f39e81e54101656961ab6e5549f64b63eb57868ba7bab8d` |
| `build/zeron/GeistMono.ttf` | 148516 | `42d8ad2e610238e64e8abfcde3037c63f7850a73928742b7ab7229d897bcb155` |
| `worlds/splice-bots.json` | 4456 | `a2165b90706eeb9c7b9bbf33f8a032b78088b7ee292cf7908d884183ac057ee0` |
| `worlds/everglade.json` | 235297 | `437e45349b2e6f97bd331c1592d45411655ab077f6cfb29d745678dcc2431fdd` |
| `worlds/everglade2.json` | 235432 | `234d0d84deb8b4497b3c859b750b2344f4269aef9433fa35aec6019273f4c096` |
| `worlds/myrealm.json` | 235294 | `2ee1e81d1927fab295fc761df7760dfb6224cc81737d66c35fb5edfa0ead83ff` |
| `worlds/microduck.json` | 546 | `4c3d85ceb5253e3907ca63f5a1810cc45af668eb9ed7deca2a6ae877352fde44` |

The native transcript/program suite passed 16 tests; the merged messaging/account
suite passed 57, with one live-network opt-in skipped. Earlier real API/relay
exchange is recorded in the verification README. All five world modules link.
Crash symbols are retained by UUID. Apple upload and web promotion receipts
are recorded on issue #13629.

Upload accepted: `0d187a01-baad-4b1c-8b50-666603b48fa1` (21,750,958 bytes).
Apple status: `VALID`, internal `IN_BETA_TESTING`; existing Owner group verified.
The matching runtime-scoped generation-1 feed and all 21 resources verify at
both advertised HTTPS endpoints. The native binary was not rebuilt for the
browser-only isolated-asset loader change.
