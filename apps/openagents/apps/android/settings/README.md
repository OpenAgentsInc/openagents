# Android settings source slice (#12091)

This directory has no app entry point or Gradle wiring. It mounts Rust Native
settings forms through the landed `SemanticViewAdapter`. Kotlin forwards view
identity and raw field values; Rust owns confirmations, preferences, SDK calls,
and account fencing. Generated bindings and executable checks are pending.

`open_settings` requires the shared `CoreAccount`, its matching `CoreClient`, and
capabilities for callbacks the host has actually registered. Construct a fresh
`SettingsHost` per account generation, use a UI coroutine scope, forward
`SettingsListener` notifications onto that scope, and dispose it on teardown.
A closed account surface must be replaced, not restored from saved JSON.

Register pairing, archive, and project-sharing navigation before enabling their
capabilities. Project sharing opens the existing `CoreSharing` Rust Native form;
archive uses the shared history owner. Cloud controls stay hidden until the
shared SDK returns an authorized status. Appearance and wallpaper controls stay
hidden until a native preferences applier is connected. On restart, read
`CoreSettings.preferences()` and apply those values before displaying the app.
The Rust store writes only versioned presentation preferences to
`phone-settings.json`; it never saves deletion consent or account credentials.
Use the existing Rust wallpaper renderer for pixel effects.

`AndroidSettingsActions` uses the Android document picker and reports the
persisted read-permission result with Rust's single-use picker ticket. Forward
only its reserved activity-result request code and close the actions object on
account teardown. Retain the URI permission for restart. The host must handle
native chooser/picker launch failures without applying a preference change.

Account deletion uses the existing SDK contract and shared account controller.
A mounted warning permits the first request. Only `PaidCredit` permits the
separate give-up-credit confirmation. `SharedTeam` and `Failed` show the shared
messages. After a pending deletion completes, an atomic generation check keeps
an account selected in the meantime signed in with its saved credentials.
Synthetic fixtures cover all four outcomes during that account switch, ordinary
refusals, successful deletion, and stale confirmation/sign-out requests.

## Remaining contracts and integration

- Project-invitation sharing uses `CoreAccount::project_invitation_share` with
  the original URL. Rust validates the canonical bounded link and returns a
  `ProjectInvitationShare` with `title` and `text`; the URL appears only inside
  `text`. Pass those fields to `AndroidSettingsActions.shareText`. Do not expose
  the invitation code separately or build invitation text in Kotlin. The About
  Share action uses the separate Rust `ShareText` effect.
- Android push-token registration belongs to F1. The existing APNs-shaped
  `CoreClient.register_push_target` API is not an Android registration contract.
  This slice provides only the OS notification-settings entry.
- B1 must register the source/test directories, navigation, lifecycle observer,
  native preference applier, picker result forwarding, and chooser handling.
  Account, pairing, sharing, and archive capabilities must reflect integrated
  implementations, including the pending #12082/#12087 dependencies.
- The new mobile dependency on `coder-ui` requires an app-workspace lockfile
  update and shared-core binding regeneration. Neither was run below the disk
  floor. Kotlin imports describe the expected UniFFI API, not a compiled result.

## Queued checks

No Cargo, Gradle, Kotlin, emulator, or device check has run for this slice. After
fresh machine status meets the 100 GiB floor and the serial build slot is clear,
use a full checkout and the pinned app toolchain. Run each Cargo command
separately from `apps/openagents`, with the shared app target and root helper:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-1-app"
export CARGO_INCREMENTAL=0 OPENAGENTS_KEEP_TARGET_DIR=1
. "$(git rev-parse --show-toplevel)/scripts/dev/cargo-build-env.sh"
cargo test -p openagents-account --test account deletion_
cargo test -p openagents-mobile --lib settings::
cargo test -p openagents-mobile --lib the_warning_keeps_the_wallet_words_in_view
```

The first Cargo resolution must update and review `apps/openagents/Cargo.lock`;
do not claim a locked build before that update. Regenerate Kotlin and Swift
bindings using the existing Android/iOS `scripts/*/build-core.sh` procedures
before their consumers compile. Run `SettingsPresenterTest` (five synthetic
fixtures) and compile this source with the integrated B1 app. The exact Gradle
command depends on B1's source-set wiring and is not runnable in this slice.

Then check synthetic device state for restart preferences, back/teardown,
wallpaper permission refusal, chooser launch, account switch during deletion,
paid-credit retry, shared-team refusal, and hidden unavailable actions. The
Rust copy guard is authored but unrun. No production account or live deletion
was used. Physical accessibility, push delivery, and store checks remain
separate from these focused source checks.
