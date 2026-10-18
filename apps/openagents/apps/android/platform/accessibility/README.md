# Android accessibility

Rust derives names, roles, states, available actions, focus order, modal scope,
and focus restoration from validated Rust Native views. It does not export typed
intents or field values. `CoreAccessibilitySurface` fences callbacks by account
generation and applied view revision. Focus bookmarks contain only the Rust
account/screen scope and a node key. Editors keep their private input; this
bridge does not save drafts, credentials, grants, or field values.

Pass an owned `NativeSemantics(CoreSemanticsPort(surface))` as the optional last
argument to `SemanticViewAdapter`. Use the same `CoreAccount` and screen scope as
the Rust view owner. Mount acknowledges the view owner before committing focus.
Dispose closes the semantics object. The adapter maps headings to spoken labels
on API 26–27 and uses the heading API on API 28 and later. It uses state
descriptions on API 30 and later. Existing native controls keep their activation,
editing, IME, and keyboard behavior. Transcript pages carry measured Rust row
bounds in layout points; the adapter does not estimate row geometry.

`CoreTranscriptAccessibility` reads the current measured Rust frame. Pages have
at most 32 rows and each text part has at most 2,048 UTF-8 bytes. Continuations
use Rust byte positions; adapters must not truncate or split text themselves.
`TranscriptNavigation` supplies earlier/later actions for the existing C2 virtual
node delegate. Forward its returned actions and `perform` calls from that delegate;
`scrollTo` receives a Rust row identity and measured bounds. Call `retire` before
replacing or closing a borrowed frame. Catch the generated `CoreError` at the
host boundary and refresh the current frame when a revision expires. This module
installs no second virtual node provider and owns no viewport.

`AccessibilityLifecycle` forwards font scale, density, and the platform animation
setting to the existing Rust layout owner. The host must request a new frame;
Kotlin must not rewrap Rust text. Lifecycle methods return Rust requests to refresh
existing account/session/permission owners or pause existing media owners. Save
focus before rotation or teardown, restore the bookmark on the new screen, and
mount a fresh Rust view before accepting callbacks. These requests do not implement
a second reconnect, permission, upload, call, or draft state machine.

## Pending integration checks

This Q1 source has not been compiled or run. The B1 checkpoint previously ran its
two `SemanticViewAdapter` tests on offline API 35; those results predate this
change. The B1 source sets already include `platform` in the app and
`platform/tests` in Android instrumentation tests, so these sources need no
build-file changes. Regenerate the UniFFI binding from the production-surface
mobile library before compiling the new types. Connect transcript actions to the
integrated C2 virtual-row delegate. The fixtures are authored synthetic input,
not Android output or a captured Rust frame.

After the coordinator assigns a build slot, run the Rust semantics and mobile
accessibility fixtures, regenerate bindings, then run focused Kotlin and Android
instrumentation checks on API 26 and 35. Physical TalkBack, switch/keyboard
navigation, system and predictive back, contrast, light/dark modes, large fonts,
reduced motion, permission revocation, and interrupted upload/call recovery
remain device checks listed in `NEEDS_OWNER.md`. No device pass is claimed by
this source.
