# Android accessibility

Rust derives names, roles, states, focus order, modal scope, and focus restoration
from validated Rust Native views. `CoreAccessibilitySurface` fences callbacks by
account generation and applied view revision. Focus bookmarks contain only the
Rust account/screen scope and a node key. Editors keep their private input; this
bridge does not save drafts, credentials, grants, or field values.

Pass an owned `NativeSemantics(CoreSemanticsPort(surface))` as the optional last
argument to `SemanticViewAdapter`. Use the same `CoreAccount` and screen scope as
the Rust view owner. Mount acknowledges the view owner before committing focus.
Dispose closes the semantics object. The adapter maps headings on API 26–27 using
collection metadata and uses the heading API on API 28 and later. It uses state
descriptions on API 30 and later. Existing native controls keep their activation,
editing, IME, and keyboard behavior.

`CoreTranscriptAccessibility` reads the current measured Rust frame. Pages have
at most 32 rows and each text part has at most 2,048 UTF-8 bytes. Continuations
use Rust byte positions; adapters must not truncate or split text themselves.
`TranscriptNavigation` supplies earlier/later actions for the existing C2 virtual
node delegate. Forward its returned actions and `perform` calls from that delegate;
`scrollTo` receives a Rust row identity and measured y coordinate. Call `retire`
before replacing or closing a borrowed frame. Catch the generated `CoreError` at
the host boundary and refresh the current frame when a revision expires. This
module installs no second virtual node provider and owns no viewport.

`AccessibilityLifecycle` forwards font scale, density, and the platform animation
setting to the existing Rust layout owner. The host must request a new frame;
Kotlin must not rewrap Rust text. Lifecycle methods return Rust requests to refresh
existing account/session/permission owners or pause existing media owners. Save
focus before rotation or teardown, restore the bookmark on the new screen, and
mount a fresh Rust view before accepting callbacks. These requests do not implement
a second reconnect, permission, upload, call, or draft state machine.

## Pending integration checks

This source checkpoint has not been compiled or run: host capacity is below the
100 GiB floor. Regenerate the UniFFI binding from a fresh production-surface mobile
library before compiling these new types. Add this directory and its tests to the
B1 source sets; Activity/Gradle wiring belongs to B1. Connect transcript actions
after C1/C2 are integrated. The synthetic metadata in `tests` is authored fixture
input, not Android output or a captured Rust frame.

After capacity clears, run the Rust semantics and mobile accessibility fixtures,
regenerate bindings, then run the app's focused Kotlin/JUnit and API 26/35 offline
instrumentation checks. Physical TalkBack, switch/keyboard navigation, system and
predictive back, contrast, light/dark modes, large fonts, reduced motion, permission
revocation, and interrupted upload/call recovery remain device checks listed in
`NEEDS_OWNER.md`. No device pass is claimed by this checkpoint.
