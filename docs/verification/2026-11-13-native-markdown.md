# Native Markdown and message presentation, 1072

The phone follows T3 Code `f313d0a6cb`'s native text/message approach using
OpenAgents' existing shared Coax parser and components. The source reference
and exact scope are in [the native spec](../../coax/native/composer.md#native-messages-build-1072).
No external Markdown runtime or second parser was added.

## Verified behavior

* Shared message articles carry heading levels, combined inline marks,
  list nesting/task metadata, table alignment, quotes, code and opt-in rules.
  The component projection reserves tree capacity and falls back to complete
  source when the rich representation cannot fit.
* The native projection uses TextKit selection/layout, distinct code/table
  views, user bubbles and assistant prose. Message views and selections
  survive draft changes and copy acknowledgements.
* Copy message emits the original source; Copy code emits the unfenced code.
  The installed clipboard service never reads clipboard contents. Text effects
  can finish alongside a model request without replacing its turn state.
* Links are explicit tapped component actions, admitted in Coax again by the
  installed host. The existing shared URI parser is reused. Only HTTP(S)
  document links open, with fragments supported and no account headers.
* A non-cancelling outside-tap recognizer dismisses the editor keyboard while
  preserving controls and long-press text selection. The composer keeps its
  normal send/focus behavior. Shared end-follow geometry yields when the
  reader scrolls up. Header toolbar nodes remain outside the transcript.

## Checks

The remote targeted component suite job passed, including the shared parser,
tree, message kit, transcript and applicable scroll/projection checks. The
app's 236 Coax checks passed. The link authority killed all 29 root mutants,
with zero survivors or equivalents. Native TextKit tests passed for heading
size, bold/italic/strike/link attributes, tasks/code, immutable selection,
original-message/raw-code copy, and admitted link requests.

The chat UI flow passed email login → send → formatted reply → copy → outside
tap and keyboard dismissal → relaunch → saved conversation reopen → multiline
composer. The two Extras/balance UI cases passed after the toolbar projection
change, using fixture observations and no wallet seed or payment operations.
Screenshot inspection found a toolbar ordering and copy-icon sizing issue;
both were corrected before release.

The portable artifact oracle now exercises rich Markdown and both copy
operations against real WebAssembly; frames and action bodies match exactly.
Wire-check, layer-check, components-check and tools-check passed. The first
implementation push's pre-push checks passed in 18 seconds.

These tests use a fake mailbox/API/model with real native Keychain, text and
HTTP services. Production model/storage routes were not changed or redeployed;
the live account chat path verified for build 1071 remains in use. This release
changes message presentation and platform text effects, retaining the current
whole-response model transport. It does not add token streaming, a work/tool
log, math/Mermaid, image fetching or new syntax lexers.

## Release

The release is build 1072, version 1.0.0, `com.openagents.app`. The new installed
semantic text and platform-effect contract requires this TestFlight build;
1071 cannot acquire these native views through OTA. Coax application and
message code remain deliverable through signed Updates in the new runtime.

The archive, signed package and screenshots are retained under
`~/.openagents/releases/openagents-ios-1072/`. Owner device acceptance is
recorded in `NEEDS_OWNER.md`.

Runtime: `f2e09cdfd651f8e9cca9b34a9b2d37198a5dc8e273fb5ea45c4371e2c2d03060`.
Signed OTA generation **1** serves the same five resources as the exported
archive. The public domain and pinned Cloud Run fallback returned the exact
signed envelope and all five file hashes/bytes. The final native chat UI test
passed again after the toolbar order and icon-size correction.

Apple accepted the archive and reports **VALID / IN_BETA_TESTING**.
Build 1072 is verified in the existing **Owner** group, with its release
notes assigned. No new testers or groups were created.
