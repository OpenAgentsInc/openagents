# Integrating with the iOS host

The iOS app runs the shared app core (`apps/shared/app/core/core.coax`) and
performs every effect it asks for with native services. Views, the login flow
and the environment picker never call the host: they read the core and
dispatch actions into it.

## What the host does

| Piece | File | What it does |
| --- | --- | --- |
| Root | `app.coax` | Loads the page, the core, the atlas tables and the cases adapter; `oa_root_guest` lets a guest result go when no page took it; `oa_drive_*` run coax drive's cases (debug builds) |
| Core in C | `core-host.c`, `core-host.h` | `oa_core_*`: each `app_host_*` entry point as plain C; a call that answers `APP_GUEST` hands the done event's text to the page's hook (`oa_core_on_guest`, set by `oc_attach` to `chat_guest`: the environment picker's reads) at once |
| Page in C | `host.c`, `host.h` | `oc_*`: the chat page's frame, pointer, rows and editor text; `oc_env_save` / `oc_env_restore` (the chips' choices, kept in app preferences by `App.swift`); `oc_drive_start` (`-OADrive`, debug builds) |
| Core in Swift | `AppCore.swift` | `AppCoreC` (the ABI), `AppServices` (launch with rejoin, services, lifecycle) |
| Bridge | `apps/shared/native/AppCoreBridge.swift` | Takes or refuses each queued request, tells cancellations, returns results with the service table's fields, one timer for `app_host_wake` |
| Services | `apps/shared/native/AppHTTP.swift`, `AppAccount.swift`, `AppWake.swift`, `AppDevice.swift`, `AppEffects.swift` | HTTP (one origin, the core's `up_http` policy, no cookies or redirects, per-request limit and timeout), the account (tokens and the email attempt stay native), IDs and clocks, clipboard, links, lifecycle |
| Editing | `apps/shared/native/KitEditor.swift` | The platform editors over GPU fields. Which field holds the keyboard is the page's (`uiframe_focus`, kit/frame.coax); the host shows that field's editor and hides the keyboard when there is none. Each frame the composer's editor takes the page's draft (`oc_text`), so a sent, chosen or driven draft shows |
| Cases | `apps/shared/host/apple/phone-drive.c` | Debug builds on `-OADrive SOCKET`: each operation runs through the shared driver (`apps/shared/app/cases`) on the main thread, then the host takes its turn (`oc_on_drive_op`: the keyboard a tap asked for); while the driver holds the clock a display frame gets only what it grants |

## What a screen gives the host

To be integrated, a screen module leaves an `INTEGRATION.md` that names:

1. The file the root loads.
2. Its surface procedures with `#export_c`: frame, pointer, the semantic rows,
   the focused field's box and kind, and the editor's text in and out (the
   shape of `chat_*` in `chat.coax` and `oc_*` in `host.h`).
3. The core actions a platform event becomes (`app_host_action` codes), if
   any. A screen dispatches its own taps.
4. For a guest (the environment picker): the procedure that takes the done
   event's text on `APP_GUEST` (the chat page's is `chat_guest`; with none,
   the root calls `app_guest_done`).

The host does not change a screen's state. It reads `app_host_dirty_take` and
redraws.

## Checks

```sh
coax/remote/remote.sh run --local -- bash apps/shared/native/app-services-check.sh
apps/openagents-ios/dev.sh
apps/openagents-ios/check.sh
```
