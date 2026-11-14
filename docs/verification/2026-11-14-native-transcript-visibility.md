# Native transcript visibility — 2026-11-14

The reported CoderOS transcript showed empty user bubbles and standalone copy
buttons. The native Claude reader returned text, tool calls and tool results;
the environment adapter discarded the two tool arrays and rendered every
source record as a plain message, including records with no text.

The shared environment projection now retains text and groups tool activity
under stable source-record IDs. Matching results join their calls in one
expandable tool row. Results whose call is on an earlier page remain visible.
Tool details remain literal, inert data. Source cursors, bounded omissions,
read-only control and admission checks remain owned by their existing codecs
and policies. The shared message component also skips empty Markdown documents
before creating either a bubble or a copy action.

## Delivery

- Signed Updates generation **3** for the TestFlight **1085** runtime
  `2d68783d8dcf339c1914ea28688a0baa0ce00c83a695288286aa93aeb3d5b750`.
- Only `build/app.js` and `build/app.wasm` differ from generation 2. The other
  eighteen resources retain their prior hashes. No native SDK change or new
  TestFlight upload is required.
- Public read-back from both `updates.openagents.com` and the Cloud Run
  fallback verified generation 3, its Ed25519 signature, and all twenty
  artifact byte lengths and SHA-256 hashes.
- The Mac LaunchAgent and CoderOS user service were updated in place after
  confirming neither service had child processes. Versioned executables and
  private rollback pointers were retained; identities and grants were kept.

## Evidence

- Five native Markdown/transcript XCTest cases passed, including twelve
  interleaved empty/whitespace source records. The native tree retains only
  the two real messages and two tool groups. Visible rich text has nonzero
  height and content. Simulator screenshots were exported and inspected.
- Shared message/frame checks: **57/57**. Coax-to-JavaScript/Wasm runtime
  oracle passed for the app and extras.
- The actual CoderOS conversation from the screenshot was read through the
  restarted service: **15 items, 10 tool rows, 5 text messages, zero empty
  messages, no error**. The Mac service returned its sixteen sessions without
  error after restarting.
- Projection suite: **119/119** checks passed, and all **178** changed-code
  mutants were killed. Checks exercise pairing, parallel calls, unpaired
  results, exact literal content, malformed source fields, buffer boundaries
  and atomic refusal of exhausted pages. The full host suite previously
  passed **857/857** before the final projection refinements; its unchanged
  units were not rerun as a separate full suite.

Private manifests, envelopes, service rollback files, RPC replies, screenshots
and executables are under
`~/.openagents/releases/native-transcript-visibility/`. The XCTest result is
`/private/tmp/oa-transcript-visibility.xcresult`.

The remaining physical-phone check is recorded in `NEEDS_OWNER.md`; simulator
coverage and real service read-back do not claim that a phone installed the OTA.
