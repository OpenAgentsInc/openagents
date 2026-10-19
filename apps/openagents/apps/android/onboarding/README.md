# Android onboarding and recovery

`CoreAndroidRecovery` presents Rust Native tutorial, Your ID, backup, confirmation,
and restore screens over the existing `CoreOnboarding` and `CoreAccount` APIs.
Its constructor creates no identity. `start` uses the shared identity-first path
only when no identity is already loaded. That path owns provider-account linking,
proof of work, and reuse of an existing wallet seed. This module never creates a
wallet, imports provider credentials, or changes storage rules.

Create the flow after A3 supplies the shared account and identity stores. Mount
`RecoveryHost` in a shell-owned container using one bounded serial worker. B1 owns
the Activity, source sets, generated bindings, and the ActivityResult launcher.
Map only `RESULT_OK` to credential success; cancellation, unsupported credentials,
missing secure lock screen, and missing intent must return false. The bridge uses
[Android's device-credential prompt](https://developer.android.com/reference/android/app/KeyguardManager#createConfirmDeviceCredentialIntent(java.lang.CharSequence,java.lang.CharSequence))
on API 26 and later. Its authentication callback is trusted platform input, not a
remote assertion or replacement for A3's authenticated store.

Forward `onPause` to `background` before task previews/clipboard access are lost,
`onResume` to `foreground`, and screen dismissal/account replacement to `close`.
A credential Activity may pause this Activity. In that case, the host keeps only
the pending authentication token, drops the mounted views, and defers its result
until foreground. Rust still checks its original 60-second deadline and captured
account generation/public key. Ordinary backgrounding clears screen data and
invalidates callbacks. The screen sets `FLAG_SECURE` before rendering secrets,
disables autofill, input suggestions, saved editor state, and implicit text copy,
and clears editor/display references on close. It does not promise zeroization of
managed-runtime strings or change an existing window's secure flag.

Copy is available only through the Rust card's explicit action. The clipboard is
marked sensitive and receives a maximum 60-second cleanup timer. Background and
close request immediate cleanup, using a nonsecret marker to avoid clearing a
later clipboard item. See Android's [sensitive clipboard guidance](https://developer.android.com/privacy-and-security/risks/secure-clipboard-handling).
Process termination, clipboard restrictions, and OEM task previews require the
pending device checks; no physical security pass is claimed here.

## Integration blockers and pending checks

`AndroidRootPresentation` mounts `RecoveryHost` for the recovery route. The app's
main source set includes `onboarding`, its instrumentation source set includes
`onboarding/tests`, and `MainActivity` owns the device-credential result callback.
That callback accepts only `RESULT_OK`; the root forwards background, foreground,
and route dismissal to the recovery host. Rust `open_chat` uses D3's existing
default-chat owner. Do not add a Kotlin identity or session path.

The generated UniFFI binding still needs the centrally scheduled Android ABI pass
to expose `CoreAndroidRecovery` and its records. Do not edit the generated Kotlin
file by hand. Kotlin compilation and the focused recovery instrumentation tests
have not run yet.

`CoreAccount::restore_words_if_current` takes the authenticated generation and public
key. The shared controller holds its account-switch lock across existing identity
validation/write, the loaded-identity update, and reservation of the new sign-in
ticket. It passes `replace_unsaved=false`, preserving wallet/unsaved-word refusals.
The existing cancellable sign-in runs after releasing the lock. Its normal account
watcher retires old clients, so this path never closes a newer account's clients.
The legacy `restore_from_words` API remains compatible. Deterministic fixtures cover
switch-first rejection, switch-during-commit serialization, and failed writes; they
are authored but have not run. No A3 storage implementation changed.

Queued checks after an assigned build slot: changed-module Rust fixtures,
production-surface mobile build and binding generation, focused Kotlin compilation
and offline instrumentation on API 26/35. The fixtures use synthetic entropy,
a temporary in-memory vault plus scratch marker directory, fake authentication,
and no account, wallet, clipboard, or network services in host tests. Provider
continuity, existing-store migration, real device authentication, task previews,
clipboard deadline/process death, and restart qualification remain integration or
owner-device checks. No executable check has passed at this checkpoint.
