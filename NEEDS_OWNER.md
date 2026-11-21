# Owner actions

> Historical entries from 2026-10-29 through 2026-11-02 (95 sections) were archived verbatim to [docs/archive/needs-owner-2026-11.md](docs/archive/needs-owner-2026-11.md) (#13845).

## Relay fallback retirement and optional admin role (#13382, 2026-11-13)

The file-backed signing key and private-kind writes were approved in this
chat and are deployed with the existing relay identity. The Coax relay's
public WSS/NIP-11 checks and eleven extensions pass. See
[the rollout record](docs/verification/2026-11-13-relay-followups/README.md).

**Retirement completed 2026-11-13; reply timing clarified.** The agent
interpreted the owner's “Approved” reply as authorization for Cloud Run
`openagents-nostr-relay` in `us-central1`, database `nostr_relay_v2` in
`khala-sync-pg`, and cutover backups `1794510495518`, `1794519112832`,
`1794523583705`. Deletion completed at 22:57 UTC; API read-back found none
of these resources and the shared SQL instance remained RUNNABLE. The final
metadata check found 602,125 events, sequence 780366, last receipt
2026-11-12 21:47:46.065713 UTC, and zero rows after the cutover backup.

The structured “Keep the fallback and backups” answer arrived after the
deletions. The agent disclosed this timing conflict. The owner then declined
recovery work; no restoration was attempted. The earlier
bare “Approved” reply and the later structured answer are distinct events;
the retirement record must retain that distinction.

**Admin role decided: disabled.** The owner explicitly selected “Leave the
admin endpoint disabled”. NIP-86/98 is implemented and tested with a
configured admin role, but production has no admin key, rejects admin
requests and omits 86/98 from NIP-11. This is a resolved owner decision,
not a pending request for a key.


## iOS 1082 history navigation (2026-11-14)

- [ ] Install TestFlight **1082** and check the history button at the upper left.
      Select a real Mac chat, then a CoderOS chat, and confirm the expected
      history loads. Try the edge swipe and return to a draft. Settings and
      Worlds are in the history footer. Simulator checks cover the installed
      navigation and send/reopen flow with independent local account/computer
      fixtures; this final check uses the owner's real phone and computers.

## iOS 1085 shorter interface (2026-11-14)

- [ ] On TestFlight **1085**, reopen the app and apply the OTA, or use
      **Settings → Updates**. Generation **2** contains the shorter interface
      and icon actions. Confirm the appearance on the physical phone; the
      twelve native/simulator checks passed.

## iOS transcript visibility (2026-11-14)

- [ ] Reopen TestFlight **1085** and apply Updates generation **3**, or use
      **Settings → Updates**. Open the CoderOS conversation that showed blank
      bubbles: text should be readable, with compact expandable tool rows and
      no empty copy-only messages. Both computer services are updated and the
      exact conversation passes service read-back. Generation 3 also includes
      generation 2's shorter interface; no new TestFlight install is needed.

## iOS 1087 Arcade components (2026-11-14)

- [ ] Install TestFlight **1087**. Check the white/grayscale login, Chat drawer,
      composer, tool rows, Projects, Usage and floating world menu on the phone.
      Settings → Components has the animated specimens. Check reduced motion,
      larger text and VoiceOver with the real account, plus keyboard/email-code
      behavior. Installed simulator flows pass; physical-phone accessibility
      and power/frame-time measurements are not claimed. See
      [the release evidence and pictures](docs/verification/2026-11-14-arcade-mobile/README.md).

## iOS 1088 account/settings (2026-11-14)

- [ ] Install TestFlight **1088**. Open the rail’s Settings, add a passkey,
      verify the account and open its masked Backup on the actual iPhone.
      Check Face ID/cancellation, capture/background clearing, security-push
      permission/receipt and VoiceOver/large text. Simulator fixtures cover
      account screens and server admission; real Apple ceremonies and push
      delivery remain physical-device checks. Wallet keys remain separate.

## iOS 1095 Microduck workbench (2026-11-15)

- [ ] After TestFlight 1095 is available in Owner, open Home → Microduck on
      the actual iPhone. Tap a part, select head_yaw or a leg joint from the
      floating menu, adjust Pose, toggle Explode and Reset. Check pinch/orbit,
      VoiceOver's named pickers and reopening after the first download while
      offline. Native simulator/Metal and public-cache checks pass; physical
      device accessibility, power and frame time remain unclaimed. See
      [the source, pictures and release record](docs/verification/microduck-workbench-13639/README.md).

## iOS 1097 illustrative Microduck servo (2026-11-15)

- [ ] In Home → Microduck → menu → Illustrative servo, compare Baseline,
      Delayed and Limited; pause, change target/effort, try Reject negative
      inertia, then Reset. Check readable real plots, source-joint selection,
      VoiceOver and background/resume on the physical iPhone. This is an
      authored illustrative fixed-base scalar servo, without hardware control.
      Native Metal and WebGPU observations are recorded in
      [the qualification](docs/verification/microduck-servo-13640/README.md).

## iOS 1301 Microduck saved runs (2026-11-15)

- [ ] Install **1301** from the Owner TestFlight group. Home → Microduck:
      run an illustrative servo, Save, Open latest, scrub, then Branch + continue.
      After the first model download, repeat in airplane mode. Check Open file,
      Share file, cancellation and VoiceOver on the physical iPhone. Native
      Metal, browser cross-host numerical replay and cache tests pass; these
      phone ceremonies and accessibility checks remain unclaimed. See
      [the delivery and evidence](docs/verification/microduck-replay-13641/README.md).

## GPU identity settings on iPhone (#13645, 2026-11-16)

Install **1400** from TestFlight: log in by email, open sidebar → Settings,
copy the public key, and verify system passkey registration/step-up and
local device authentication when showing Backup. Test Rotate, then Restore
with a disposable identity only if you intend to replace the current key.
Never send the private key in chat or a screenshot. Check VoiceOver, largest
text and native momentum scrolling on the phone. Simulator/native checks
cover the state/crypto/transport paths; these system ceremonies and physical
appearance cannot be certified by those checks.

Build 1400 is VALID, IN_BETA_TESTING and verified in the Owner TestFlight group. It fixes the
native sidebar tap, its accessibility entry while open, and Back from
Settings → Log in; the actual UIKit tap test passes. Check each paired computer's sessions, opening
one, loading More, and reopening the drawer during a running turn. The identity API is
`oa-api-00171-b4j` after staging and production migration/smoke checks. It
includes the auto-synced Account chats drawer and main's reconnect/rejoin fix. Check opening a saved
chat, paging, returning to Latest, and scrolling at large text sizes. The
compiled fixtures exercise 60 chats and assert fixed widths during opening.

## GPU Projects on iPhone (#13645, 2026-11-16)

Build **1401** is VALID, IN_BETA_TESTING and verified in Owner
TestFlight; install it from that group. Sidebar → Projects: create a disposable Project, edit Details
and Instructions, add/read/remove reference text, then open its saved chats
or start a new Project chat. Check keyboard dismissal, large text, native
scrolling and returning to the sidebar. Settings retains automatic account
Nostr identity, public-key Copy and the passkey-protected Backup/Rotate/Restore
flow; use a disposable identity for replacement tests. These physical-phone
and system passkey ceremonies remain owner checks. The broader GPU epic is
still open for the remaining screens, composition and desktop adoption.

## GPU Usage on iPhone (#13645, build 1402)

- [ ] After 1402 is available in Owner TestFlight, open Settings → Usage and
      check the connected computers and provider-reported weekly/5h windows.
      Connect a disposable additional Claude account, complete its browser/code
      flow, then set Default and verify a new chat uses it. Existing chats keep
      their bound account. Check background/resume and a connection interruption;
      Check account change must retain the original operation.
- [ ] Verify Nostr public-key Copy and system passkey/biometric Backup on the
      physical phone. Rotate/Restore only a disposable identity if you intend
      to replace it. Never send a private key or authorization code in chat or
      a screenshot. SDK/compiled-page tests pass; these physical ceremonies,
      VoiceOver and phone appearance remain owner checks.

## GPU composition on iPhone (#13645, build 1403)

Apple accepted **1403** with no upload errors. The immediate status is
`NOT_YET_LISTED`; Owner assignment is pending processing, so install
availability is not yet verified. The handoff records the assignment command.

- [ ] When **1403** becomes available in Owner TestFlight, open
      **Settings → Renderer**. Check the image, moving translucent group,
      transformed Tap button, blur with a sharp button and source blur.
      Test Back, background/resume, VoiceOver, Reduce Motion and Reduce
      Transparency. Metal, WebGPU, Linux Lavapipe and actual UIKit tap tests
      pass; physical-phone appearance remains an owner check.
- [ ] Check existing login, chats, Projects and Usage. Verify Nostr public-key
      Copy and passkey/biometric Backup. Rotate/Restore only a disposable
      identity when you intend to replace it; never share its private key.
      [Delivery and evidence](docs/audits/2026-11-16-gpu-composition-1403.md).

## Claude-reference GPU chat on iPhone (build 1404)

Build **1404** is VALID, IN_BETA_TESTING and verified in the Owner TestFlight
group. Install it from that group; the handoff records the delivery and checks.

- [ ] In 1404, open a computer chat and check the
      centered title, collapsed activity groups, individual tool details,
      reply/code Copy and Send/Stop. Conversation options → Recent chats:
      search loaded history, then return to the draft. Verify that tapping
      outside the input dismisses the keyboard and choosing a computer works.
- [ ] Check large text, VoiceOver, small-screen menu scrolling and background/
      resume. Model/effort, attachments and microphone remain hidden by choice.
      Existing Nostr key custody and account services remain; test system
      Backup/Rotate/Restore only with an identity you intend to use or replace.
      Never share a private key. [Screenshots and qualification](docs/audits/2026-11-16-claude-gpu-chat-1404.md).


## Claude conversation library on iPhone (build 1406)

**1406 is VALID, IN_BETA_TESTING and verified in Owner TestFlight.**
1405 is superseded.
[Release and qualification](docs/audits/2026-11-16-claude-chat-library-1405.md).

- [ ] Install 1406 from Owner TestFlight. In a connected chat,
      check Pin, Rename, Archive and Restore, including Save above the keyboard.
      Preferences are local to this installation and separated by account.
- [ ] Recents → All/Pinned/Archived; search for an older chat title.
- [ ] Workspace → Branches prepares a draft. Review and Send to use the
      selected computer; existing Claude permissions and GitHub auth apply.
- [ ] Share excerpt → verify the chooser and Cancel; check VoiceOver and
      largest text. Model/effort, attachments and mic remain hidden.

## Durable asset bucket for externalizing realm `.pack` files and `psionic/fixtures/` (#13844, 2026-11-20)

- [ ] Provision or designate a permanent (no-TTL) content-addressed bucket (GCS or Cloudflare R2) with public read URL + upload credentials for realm `.pack`/`.pack.gz` bundles (`apps/openagents-web/realms/`, 358 MB) and large `psionic/fixtures/` binaries (235 MB).
- Context: the existing `gs://openagentsgemini-coax-build-cache` bucket has a 14-day lifecycle TTL (ephemeral build cache only), and local `gcloud` auth requires interactive re-login (`gcloud auth login`). Until a durable bucket is named and writable, deleting these 593 MB of assets from git would break fresh checkouts and remote build hosts.
