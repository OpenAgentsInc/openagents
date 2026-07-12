# CUT-14 mobile authenticated catalog receipt

- Date: 2026-07-11
- Issue: [#8694](https://github.com/OpenAgentsInc/openagents/issues/8694)
- Final disposition: **closed 2026-07-12.** Authenticated physical-iOS and
  Android-emulator directory,
  exact session activation, deep-link resolution, and process-death restore pass
- Chronology: earlier body statements retain intermediate simulator/emulator,
  auth, catalog, and device rungs; they do not override this disposition.
- Contract: `openagents_mobile.coding.authenticated_navigation.v1`

## Greenfield ownership

CUT-14 lands in `apps/openagents-mobile`. The older
`clients/khala-mobile` Expo app is frozen migration source and receives no new
product feature.

The mobile Sync host now creates the CUT-13 confirmed coding-catalog reader for
the exact server-verified personal scope. The host exposes one stable mobile
navigation service while replacing its hosted catalog and owner authority on
connect, disconnect, denial, and unlink. The service lists only available,
grant-eligible repositories and non-archived sessions from a live confirmed
snapshot, sorted by recent activity. Signed-out or non-live phases return no
repository/session rows and name whether cache authority is withheld or purged
after denial.

## Stable target and recovery contract

Deep links use one bounded form:

`openagents://coding/session/{sessionRef}?repository={repositoryRef}&thread={threadRef}`

Notification inputs use the same three refs under
`openagents.mobile.coding_target.v1`. Both paths decode with Effect Schema and
then resolve exact refs against the current live owner scope. Cross-owner,
stale repository/session/thread, unavailable authority, invalid catalog, and
revoked/unprojected grant states fail closed.

After a target has live authority and its thread lease opens, mobile persists
only its owner/repository/session/thread refs, source, and timestamp in the
device-local Sync SQLite store. The record contains no path, token, body, host,
or runtime handle. A real store close/reopen proves the same target restores
after process death only when the authenticated owner and confirmed catalog
still authorize it. Relinking a different owner rejects the retained target.

## Subscription fencing

Each activation receives a monotonically increasing in-process generation.
The prior thread lease closes before the next one becomes current; a lease
that opens after it was superseded closes immediately, and its late update
callback cannot enter the current projection. This is the deterministic
boundary for switching repositories/sessions without cross-scope content
leakage. OpenAgents Mobile now binds that lease to the shared no-poll live
conversation subscription. The Effect Native drawer groups confirmed recent
sessions under their repositories and dispatches one typed
`CodingSessionSelected` intent. Selection opens the exact session thread,
starts its closeable lease, and updates only while its generation is current.
New chat and ordinary thread navigation close the coding lease before
switching.

After verified process restart, app composition resolves the device-local refs
against the live catalog and passes the exact thread ref into conversation
selection. It does not infer the first chat row. The restored thread then binds
the same live lease and parent/mobile view updates remain scoped to that exact
thread.

## Verification

- Focused mobile coding + Sync host: 12 pass, 0 fail, 56 expectations.
- Full `@openagentsinc/openagents-mobile`: 71 pass, 0 fail, 316 expectations.
- Mobile typecheck: pass.
- Real SQLite close/reopen: pass with stable refs and no path/token material.
- Deep-link/notification, owner mismatch, revocation, stale target, non-live
  cache withholding, and concurrent activation fault cases: pass.
- Visible directory/conversation/Sync focus: 27 pass, 0 fail, 106
  expectations.
- Full greenfield mobile after visible integration: 74 pass, 0 fail, 327
  expectations; typecheck passes.
- Native delivery focus proves bounded offline retry, serial activation, stale
  rejection, queue overflow, initial/live source ownership, and teardown.
- Full greenfield mobile after native delivery: 77 pass, 0 fail, 345
  expectations; typecheck and `expo config --type public` pass.

## Production publication counterexample and repair

The first authenticated-device pass disproved the earlier assumption that the
Desktop-local CUT-13 catalog was already available through Sync. The owner
scope contained zero coding post-images even though Desktop held three local
sessions. Mobile therefore had nothing authoritative to list; this was a
missing production publisher, not a device-rendering failure.

Commit `6ea8f2508f` adds the bounded `coding.publishCatalog` mutation, enforces
the authenticated personal-scope match on the server, and makes Desktop publish
only stable catalog refs and metadata after connect and catalog changes. Raw
paths remain in the mode-`0600` Desktop binding and never enter Sync. Production
revision `openagents-monolith-00088-t24` serves the mutation at 100 percent.
The confirmed production owner scope subsequently held three projects, three
repositories, three worktrees, three sessions, and one navigation row, with the
publisher mutation acknowledged.

## iOS simulator debug receipt

On 2026-07-12 a native debug build was installed on an iPhone 17 Pro iOS 26.5
simulator and loaded the exact current `cut-16-exact-target-selector` bundle
from Metro. The first run exposed two emulator-only integration defects: drawer
Settings had a dead handler, and Expo's split-bundle async loader expected an
HMR client that this plain React Native debug host does not install. Settings
now opens the Effect Native OpenAgents account/Sync surface, and Metro uses its
native async-require implementation so lazy native imports remain test-safe
without split-bundle HMR registration. The current app cold-launches without a
redbox or unhandled rejection. Missing React Native lowerings for History,
Branch, and InfoCircle were completed in the same pass.

The focused account/session suite passes 26 tests with 115 expectations and
mobile typecheck passes. The simulator has no saved GitHub session, so the
authenticated directory, exact deep link, and process-death restoration pass
must follow an owner sign-in; credentials are not handled by the agent.

## Android emulator Release receipt

The existing `khala_test` AVD was restored with an Android 15/API 35 ARM image
and received a clean embedded Release APK. The exact bundle opened the Khala
home, drawer, repaired Settings route, and OpenAgents account surface. The
first account-link attempt exposed that native auth, notification, fetch, and
secure-store modules were still split through dynamic imports; an embedded
Release host could silently return home instead of opening authorization. Those
native modules now load from the main bundle. After a clean reinstall, account
linking opened Chrome Custom Tabs at GitHub's real “continue to OpenAgents”
sign-in form. Local OTA loading was disabled only in the ignored generated host
so the acceptance run could not substitute a published bundle for the source
under test.

## Android emulator debug receipt

An API 35 `khala_test` Pixel emulator now has the current native debug APK.
The first launch proved the native client requested Metro lazy bundles even
after the async-require override, reproducing the same HMR setup rejection.
Commit `a533b4ccf2` forces `lazy=false` at Metro's bundle request boundary for
the plain debug host. A cold app restart then loaded the full current bundle,
logged `Running "main"`, rendered the Khala home, opened drawer Settings, and
rendered `Local device ready` with the `Link OpenAgents account` action. The
post-fix log contains no HMR, fatal, or React Native JS error.

Authenticated catalog/deep-link/process-death acceptance remains pending the
same one-time owner OAuth sign-in. The Android emulator and Metro remain
available for that handoff; no physical Android is required.

On 2026-07-12 the persisted `khala_test` AVD was booted without loading or
saving a snapshot and upgraded in place to a fresh debug build from current
main. The pre-update and candidate APK signing-certificate SHA-256 digests
matched exactly, so `adb install -r` preserved app data; no uninstall, package
clear, or credential injection occurred. The cold-launched current bundle logs
`Running "main"` and renders the signed-out Khala surface. This removes the
stale pre-physical-fix APK as an acceptance risk, but does not satisfy the
authenticated criterion: the emulator still needs its one owner GitHub OAuth.

## Physical iOS authenticated acceptance

On 2026-07-12 build 117 on the paired iPhone exposed and closed three live
integration gaps that deterministic tests had not revealed:

- the post-auth experience could become Sync-current while retaining a
  pre-live withheld coding-directory snapshot;
- 48 ordinary chat rows preceded coding navigation in a non-scrollable drawer,
  making valid coding groups unreachable below the viewport; and
- published coding sessions named thread refs for which no hosted
  `chat_thread` existed, while a newly opened dedicated thread scope could be
  misclassified as stale during its short catch-up window.

Commits `d97e14fc1d`, `a3af489cc5`, `8612aca5b2`, and `a6afa94f74` repair those
boundaries. Desktop now materializes each published coding thread through the
authenticated conversation service before catalog publication. Mobile refreshes
the full authenticated experience after catch-up, places coding groups before
ordinary history, and waits boundedly for the live thread lease to become
confirmed.

Physical receipts:

- the drawer renders `Coding sessions` with three `codex-smoke` repository /
  session groups before ordinary chat history;
- device SQLite contains three coding sessions and a matching confirmed
  `chat_thread` for every published `threadRef`;
- selecting a directory row persists the exact owner/repository/session/thread
  tuple as `mobile_coding_selection` under device-local scope;
- a CoreDevice process replacement restores the same exact coding thread and
  coding composer context without inferring the first conversation; and
- a cold-launch `openagents://coding/session/...` payload resolves and persists
  the requested second session after authenticated catch-up.

Focused verification after the repairs: mobile 29 pass / 138 expectations,
Desktop 98 pass / 546 expectations, both app typechecks pass.

## Authenticated Android acceptance and closure

On 2026-07-12 the owner completed GitHub OAuth in the persisted API 35
`khala_test` emulator. The authenticated drawer rendered all three confirmed
coding repository/session groups. Selecting the explicit `Active` control for
a session opened and durably stored its exact repository/session/thread tuple;
an `am force-stop` process replacement restored the same coding context and
target selector.

The cold deep-link pass found one real source-attribution defect: native target
delivery resolved the correct session but discarded whether activation came
from a deep link or notification, so persistence mislabeled it as `directory`.
Native delivery now carries the decoded source into the same authoritative
activation binding. Focused tests prove both `deep_link` and `notification`
propagation. The emulator then cold-opened the exact third-session deep link;
device SQLite recorded its exact refs with `source: deep_link`. A subsequent
ordinary cold launch retained the refs and advanced the durable source to
`restore`.

Final verification is 21 focused tests / 121 assertions and the complete
mobile suite at 123 tests / 643 assertions, all passing; mobile typecheck also
passes. The AVD hardware-keyboard bridge is enabled, so owner input works from
the macOS keyboard without using device mirroring. CUT-14 therefore has both
required authenticated platform receipts and no residual acceptance gate.
