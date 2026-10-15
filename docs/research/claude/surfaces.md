# Surfaces and deep links

The desktop registers the `claude` URL scheme. One function,
`claudeURLHandler`, decides what a URL may do. It logs
`claudeURLHandler handling` at silly level and returns false when the URL
does not parse or the target window is gone. The application was not launched,
so this is the function's control flow, not a trace of a click.

## When a link is dropped

If managed configuration sets `authentication.disableDeepLinks`, every host is
dropped except `login` and the `claude.ai` paths `magic-link` and
`sso-callback`. The log is `dropping deep link (disableDeepLinkRegistration)`.
The managed-setting copy for that switch is "Disable claude:// deep-link
handling", with `availableInVersion` `1.6889.0` in the setting record.

If the protocol is not in the allowed set, the handler warns
`invalid protocol` and returns false. Preview (`preview`) and debug handoff
(`debug-handoff`) are parsed and then rejected in this build: both log that
they are only available in nest builds. The embedded build JSON says
`isNestBuild` is false.

`/login` and `/gift/redeem` on the web origin are treated as pages that must
not be interrupted by a late login callback. A callback that arrives while the
view is on one of those paths, or while the view is already gone, is evicted.

## Hosts

The host enum and the path enum are adjacent string tables in the main chunk.

| Host | Path values the handler branches on |
| --- | --- |
| `hotkey` | Accepted and ignored. The handler returns true and does not navigate. |
| `preview` | Rejected outside nest builds. |
| `debug-handoff` | Rejected outside nest builds. |
| `sable-otter` | Loads `index.chunk-DXSnxnIZ.js` and calls `composeFromOs` with the query parameter `k`. Does not navigate the main view. |
| `cowork` | `/shared-artifact?uuid=`, or `/new`. |
| `code` | A code route, `/continue`, `/needs-input`, `/search`, or `/new`. |
| `resume` | `?session=` must match the uuid pattern used for shared artifacts. |
| `login` | `/google-auth`. |
| `claude.ai` | The web path table below. |

`cowork /shared-artifact` requires a `uuid` query that matches that same
pattern. It navigates the main view to `/cowork-artifact?shared=<uuid>`.
`cowork /new` reads `q` (truncated), and repeats `folder` and `file`. When
the caller says the argument vector was capped, folder and file are
discarded. Otherwise they are copied into `/task/new` and `src=external` is
set. Absolute files that survive the path checks can be handed to a separate
external-file intake when there is no query and no folder. The telemetry verb
is `cowork_new`.

`code /continue` and `code /needs-input` read `session`. Continue requires the
session check used for "last" and for concrete ids. Needs-input allows a
missing session or an id matching `local_[A-Za-z0-9-]{1,64}`. Search navigates
to the code surface's `/search`. New reads `q` or `prompt`, repeats `folder`
and `file`, and also reads an ssh payload. If that payload is not `kind:
"none"`, the handler either refuses it as `truncated` when the argument
vector was capped, or starts a session with the ssh target. Folder values
pass through a normalizer before they are appended.

`resume?session=<uuid>` dynamically imports the session manager and calls
`importCliSession`. Success navigates to the imported session's route.
Failure is categorized and logged as `Failed to import CLI session`. A bad
id is `missing or invalid session`.

## claude.ai paths

The path table's string values, in source order, are `magic-link`, `new`,
`sso-callback`, `mcp-auth-callback`, `chat`, `project`, `settings`,
`admin-settings`, `customize`, `directory`, `create`, `tasks`, `task`,
`space`, `claude-code-desktop`, `code`, `epitaxy`, `design`, `resume`,
`cowork`, and `local_sessions`. The handler does not switch on every one of
those spellings directly. Several share a helper that forwards the path into
the web view after a normalization check.

| Path | Derived behavior |
| --- | --- |
| `magic-link` | The hash must split into exactly two colon-separated parts. Those parts are copied onto `magic-link` on the web origin. `anon_id` is stored as the cookie `_cross_domain_anonymous_id` on that origin, path `/`, secure, `SameSite=Lax`, not httpOnly, expiring one year out. |
| `new` | Query `q` is turned into a composer URL with `surface` and `composer`. The verb is `cowork_new` when `surface=cowork`, otherwise `chat_new`. |
| `sso-callback` | A platform-suffixed path is ignored. Otherwise the query is copied onto `/sso-callback` on the web origin, with the same anonymous-id cookie. |
| `mcp-auth-callback` | The path must stay under `/mcp-auth-callback/` after normalization. The dispatcher gets `dispatchHandleDeepLink`. |
| `chat/<uuid>` | Navigates to `/chat/<uuid>`. Any other shape navigates to `/recents`. |
| `project/<uuid>` | Navigates to `/project/<uuid>`. Any other shape navigates to `/projects`. |
| `epitaxy` | Must stay `/epitaxy` or `/epitaxy/…` after normalization, then the web view navigates there. |
| `customize` | `…/plugins/new` with `marketplace` (and optional `plugin`) is rewritten to `/<head>/plugins/new`. Other customize paths use the shared forwarder. |
| `code` | A recognized code route is dispatched. A session id matching `cse_` or `session_` plus the allowed character set can open that session. A malformed session id is rejected. Anything else falls through to the generic forwarder. |
| `cowork`, `local_sessions`, `directory` | Shared forwarder, and the cold-start flag is passed through. |
| `tasks`, `task`, `space`, `settings`, `admin-settings`, `create`, `claude-code-desktop` | Shared forwarder. |

The generic forwarder strips leading slashes, rejects a path that fails the
normalization check, and asks the web-contents dispatcher to handle the
resulting claude.ai URL. A path under `/code` also notifies the code surface
that this web contents is navigating there.

Google sign-in (`login` host, `/google-auth`) requires a `code` that answers
a sign-in this process started. An unsolicited or cold-start code is evicted
and nothing is saved. The code is delivered to the renderer with
`send(googleAuthCode, {code})` only when the view exists, the start was not
cold, and the view is not sitting on `/login` or `/gift/redeem`. The same
anonymous-id cookie is set when `anon_id` is present. A missing code is
`desktop_login_failed` with reason `malformed_callback`.

The binding whitelist matches this: the page may register `googleAuthCode`
and `cmdK`, and no other binding name.

## Desktop actions

Four URLs are written into Linux `.desktop` files under the user data dir
(XDG `applications`, file name built from the app id). They are also the
template for the actions themselves:

| Name | URL |
| --- | --- |
| New Chat | `claude://claude.ai/new?surface=chat&source=desktop_action` |
| New Claude Code Session | `claude://code/new?source=desktop_action` |
| Continue Last Claude Code Session | `claude://code/continue?session=last&source=desktop_action` |
| Sessions Waiting for You | `claude://code/needs-input?source=desktop_action` |

Each entry sets `Exec` and `TryExec`. The source query `desktop_action` is
what `Dk()` records as `os_entry` on `desktop_os_surface_invoked`.

Sharing an artifact also builds
`claude://cowork/shared-artifact?uuid=<artifact_uuid>` when the share helper
checks whether shared content is still current.

## Shell behavior around the web view

The main shell and the main view are two preloads on one window. The shell
exposes the title-bar service `MainWindowTitleBar`: `titleBarReady`,
`updateTitleBar`, `showLoadError`, `hideLoadError`, `requestReloadMainView`,
`requestRestartApp`, `requestMainMenuPopup`. Menu commands from the web view
are a second service, `MenuEvents`: new and close window, reopen the last
closed session, sidebar, session cycling, command palette, keyboard
shortcuts, split view, find in files, and an edit-command claim so the page
can take or release undo and clipboard shortcuts.

`WindowControl` reports layout back to the shell: frame, sidebar, split view,
go-menu areas, which menu target should close, whether a modal scrim is up,
and whether the reopen-closed-session item should be enabled. It also
resizes, shows, focuses, closes, sets theme, and sets an incognito flag.
`WindowState` reports fullscreen, visibility, on-screen, kept-hidden, app
focus, zoom, computer-use dock state, host power, device link, and
compositing.

`QuickEntry` on the main view is `setRecentChats` and `onQuickEntrySubmit`.
The quick-entry window itself only dismisses and skooches. Recent chats are
pushed in; the submit comes back out.

Find in page is split. The overlay window's service is `FindInPage`
(`findInPage`, `stopFindInPage`, `endFindSession`, `focusInput`, `blurInput`,
`clearQuery`, `stepToMatch`, plus a density store). The page's service is
`FindInPageProvider` (`findRequest`, `findClear`, `reportFindResult`,
`setProviderActive`, and the popout variant). `Resources` adds the file
search that the same overlay can cover: project file list, content search,
and claim/obstruct/popout so two surfaces do not both own the bar.

`DesktopIntl` is `getInitialLocale`, `requestLocaleChange`, and
`onLocaleChanged`, exposed on every shell preload including Buddy.

Account changes are `Account.setAccountDetails` and `activeAccountChanged`.
`Auth` is `doAuthInBrowser` and `prepareForSignedIn`. `WebBuild` reports the
page's commit hash up to the shell with `reportCommitHash`. Plan usage is its
own service on the same world. The exact method list for `PlanUsage`,
`Navigation`, `OsOpenHandlers`, `DeepLink`, `ContentStampReporter`, `Toast`,
`ManagedConfig`, and `Dictation` was not extracted as a method array; the
names are on the world object.

## About, support, and update

`AboutWindow` is `getAppName`, `getBuildProps`, `getSupport`, `openHelp`, and
`ancientJournal`. Help is `https://support.anthropic.com`. The updater
service on the main view is `checkForUpdates`, `restartToUpdate`,
`restartToUpdateWhenIdle`, `cancelPendingRestart`, `getRunningLocalSessionCount`,
`getRunningLocalSessions`, `requestAncientJournal`, and the two state
subscriptions `updaterState_` and `ancientJournalState`. Restart-when-idle is
the same decision as the stealth-update countdown: do not quit while a local
session is still running.

`SupportBundle` is `submitAction` plus `supportBundleState_`.
`DesktopInfo` is `getSystemInfo`, `getDefaultBrowserName`, and
`showLogsInFileManager`. Log files the VM and native host write beside that
folder are listed in [the VM chapter](vm.md).
