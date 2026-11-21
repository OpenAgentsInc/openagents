# OpenAgents for iOS

The iOS app uses the compiled chat page, the shared transcript over the
composer (apps/shared/app/view/page.coax). The navigation button at the top left opens the sidebar,
which pushes the page right. It matches the desktop's sidebar
(`apps/openagents-desktop/page/layout/sidebar.coax`): New chat and Search at the top, Pinned and
Recents with every paired computer's chats together (each row names its computer when there are
several), and the "Local" row with its gear (Settings) at the foot.

**Experimental features** (Settings, off unless turned on, as on the desktop; UserDefaults
`experimentalFeatures`) are log in and the account, Projects, Foreman and the Nostr keys. Off, the app
has no login anywhere and no Default: the environment chip starts on the first paired computer, and a
Send with no paired computer says to pair one instead of asking for a login. On, the sidebar adds
Projects, Foreman and Log in or Log out, Log in pushes a full login page with a Back button, and a Send
on Default while logged out opens the same page. The page is compiled Coax drawn
by the shared GPU component kit in one Metal view. The app compiles nothing
to JavaScript or WebAssembly, has no web view and no Updates path. The plan
is in [components/gpu-components.md](../../components/gpu-components.md#the-ios-app-grown-screen-by-screen):
the app grows one screen at a time from this page.

Bundle ID `com.openagents.app`, team `HQWSG26L43`, version 1.0.0.


## Files

| File | What it is |
| --- | --- |
| `chat.coax` | The chat page: state, layout, paint and semantic rows, with its check |
| `app.coax` | The root that `generate.sh` compiles to C: the chat page, the shared app core and the atlas tables |
| `host.c`, `host.h` | The page's GPU carrier, included by `components/gallery/linux/main.c` |
| `ComponentsHost.m` | Compiles the shared Apple GPU host into the app |
| `App.swift` | App delegate and chat screen: display link, platform editor, VoiceOver; the editor for the field the page focuses (a tap outside it hides the keyboard, kit/frame.coax) |
| `DevGallery.swift` | The dev menu, debug builds only: the app set's gallery. Launch with `-OADevGallery YES` or double-tap with three fingers |
| `AppCore.swift` | The shared app core (`apps/shared/app/`) with the native services: HTTP under the core's policy, the account, the clipboard, links, lifecycle and rejoin |
| `core-host.c`, `core-host.h` | The core's `app_host_` entry points as plain C, included by `host.c` |
| `Bridge.h` | The C functions Swift sees |
| `project.yml` | The XcodeGen project: the `OpenAgents` app |
| `generate.sh` | Coax to C and Coax to Metal, in one remote job |
| `dev.sh` | The fast Simulator loop: generate and xcodegen only when their inputs changed, an incremental build, install, relaunch and one screenshot (`apps/shared/app/preview/`) |
| `build.sh` | Generate, `xcodegen generate`, then a simulator build |
| `check.sh` | Fails if the built app holds any JavaScript path |
| `release.sh` | Archive and upload to App Store Connect; `tools/app_store.py` is its API client |
| `OpenAgents/` | Icons, `Info.plist` and entitlements |

Shared code the app builds: `apps/shared/native/` (the platform editors, and the services `AppHTTP`, `AppAccount`, `AppWake`, `AppDevice`,
`AppEffects`, `AppCoreBridge`; checked by `apps/shared/native/app-services-check.sh`),
`apps/shared/host/apple/AppAuth.swift`, `DeviceKey.swift`,
`apps/shared/host/apple/ui-host.m` and `components/`.

## Build and run

```sh
apps/openagents-ios/build.sh -destination 'platform=iOS Simulator,name=Coax Native Starter'
xcrun simctl install booted apps/openagents-ios/build/Build/Products/Debug-iphonesimulator/OpenAgents.app
xcrun simctl launch booted com.openagents.app
apps/openagents-ios/check.sh
```

## Check

The conversation presentation follows the captured Claude iOS chat layouts
in `docs/research/claude-ios/`, using the shared GPU kit and OpenAgents'
dark-only Coder Noir grayscale skin (`UIUserInterfaceStyle = Dark` in
`Info.plist`, so UIKit keyboards and alerts match the dark GPU surface
even when iOS is in system Light Mode). Unsent composer drafts and
selected computer/repository chips persist in `UserDefaults` across
backgrounding and relaunch. Navigation, conversation menus and the computer/repository
picker are GPU components; the removed `GlassChrome.swift` projection is no
longer part of the product. UIKit supplies active text entry, accessibility,
scroll physics and system credential ceremonies.

On a paired Mac the second chip picks the project a new chat starts in. It
names the Mac's workspace ("openagents") and lists the projects that Coder's
chats work in inside it (a worktree counts as its repository), never the
workspace's subfolders. A Mac that predates the list shows "Workspace" and
takes no press.

Conversation options provide Share excerpt, Copy messages, Pin, Rename,
Archive/Restore and Workspace. Recents has All/Pinned/Archived filters and
paged title search across account and connected-computer history. Library
pin/rename/archive metadata syncs through the signed-in account, independently
of source transcripts. Changes merge per field across native clients and exact
pending writes survive relaunch and lost acknowledgements. Guest preferences
stay local. Workspace opens a GPU page with changes, branches, worktrees and
pull requests on the selected connected computer. Create/switch branches,
create/remove worktrees, push and create/edit PRs execute typed Git/GitHub
operations after review and confirmation; they never generate Claude prompts.
The resident service must explicitly enable this new profile locally with
`openagents-environment workspace-enable` (and `--grant ID` for another device).
GitHub uses that computer's existing `gh` login. No hosted runner is provisioned.
See [the protocol and safety contract](../../docs/protocol/native-workspaces-and-chat-library-v1.md).

Empty-chat options open Recents or the computer picker; the drawer owns
navigation and New chat. The centered session pill opens details and provider Usage. A turn's
thoughts and tool calls are one activity group, drawn by the same component as
Coder Desktop (`components/app/activity.coax`): a summary such as "Thought process ·
ran 2 commands", open while the turn works and folded once it is done, with rows
that open onto their call and output. A turn that is working shows the working
line with its seconds. Calls on a connected computer take their kind from the
tool's own name (`tk_of_name`). Replies expose Copy alongside the existing fenced-code Copy.
Observed native sessions have a fixed read-only notice instead of a composer.
Model/effort, attachments and microphone controls remain hidden by owner choice.

`apps/shared/app/preview/conversation-fixture.coax` is a synthetic visual
qualification root, absent from the installed app. Its optional carrier macro
is `OA_CONVERSATION_FIXTURE`; it exercises the real page with public fixture
messages rather than the owner's account data.

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/openagents-ios/chat.coax
coax/remote/remote.sh run -- coax/remote/check.sh only coax/native/gpu-editor-check.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/openagents-ios/editor.coax --library --run-steps 250000000 --steps 200000000
```

`OpenAgentsSidebar` runs `UITests/SidebarTests.swift` against the actual
UIKit control and GPU drawer: repeated open/close, regular Projects, Settings,
login Back and Usage navigation.
Use an isolated simulator and override `OA_IOS_BUNDLE_ID` for the test app,
so its Keychain and preferences stay separate from an installed account.
The test uses a local unreachable API origin and sends no login request.

The explicit `gpu-editor-check.coax` suite loads the shared editor; the next
command tests its compiled-page adapter and the app. `components/` and the iOS folder
are outside the web suite discovery roots, so selecting those file paths
directly with `check.sh only` would run no suites. The Apple app continues
to use its platform text views; the canvas editor is the Linux adapter.

## Release

```sh
apps/openagents-ios/release.sh /absolute/fresh/output/directory
python3 apps/openagents-ios/tools/app_store.py status --build BUILD
```

The version and build number are `MARKETING_VERSION` and
`CURRENT_PROJECT_VERSION` in `project.yml`. The script reads App Store
Connect credentials from `~/work/.secrets/appstoreconnect.env`.

## Nostr identity

Email login ensures an account-managed encrypted Nostr key. Sidebar →
Settings shows its public key, Copy, Backup, Rotate and Restore. Private
operations request system passkey verification; Backup also requests local
device authentication. Restore accepts nsec or hex through the shared Coax
codec. Wallet and independent login keys stay separate.

`apps/shared/app/settings/` holds public state, the private SDK codec and
native screen checks. `apps/shared/native/AppIdentity.swift` holds only
platform effects and protected pending-operation storage. Unknown changes
must be retried exactly or explicitly discarded; a finished change stays.
Run SDK checks with the `OpenAgentsIdentity` Xcode scheme on an isolated
simulator. Never put real private keys in a drive case or screenshot.

The history drawer reads automatically while visible and sleeps between
reads. It realizes visible one-line rows only, keeps the full panel width
through animation, and opens the chosen saved conversation through the
shared core. More pages older chats; the up-arrow returns to Latest.
Account history is separate from sessions running on a paired computer.

## GPU Projects

Sidebar → Projects opens a regular page on the same GPU surface. The list,
creation, details, instructions, knowledge and saved account chats use the
shared components and the existing private Projects API. Lists sync while
visible; forms retain edits. Saves retain one command through an unknown
result; Check save resends its exact bytes. New chats retain the Project ID
and read revision through the first submission.

Single-line fields use the existing native field; instructions and reference
text use the same platform text view as the composer. The shared Coax editor
is used on Linux. All hosts admit up to 16 KiB of editable reference/instruction
text; longer saved instructions are kept intact and refused for editing.
Complete downloaded source text is read through the shared virtualized reader,
with visible-only semantic rows and no second line wrapper. File attachments
remain a separate migration item.

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/projects/view.coax --library
coax/remote/remote.sh run -- coax/web/tool.sh apps/shared/app/projects/qualify.coax debug
coax/remote/remote.sh run -- coax/web/tool.sh apps/shared/app/projects/qualify.coax release
```

The page qualifiers run normal and ASan/UBSan builds against the actual compiled
page. They check drawer navigation, single/multiline fields, native editor
rectangles, pinned save ownership, large type, keyboard reveal and settled
motion. These fixtures do not replace a physical-phone ceremony.

## GPU Usage (1402)

Settings → Usage shows the provider-reported windows for connected Claude
accounts. Connect starts Claude Code's existing browser/code flow on the chosen
computer; the native secure field holds the pasted authorization code. Default
chooses the account for new conversations. Existing conversations keep their
bound account. Reads sync automatically; Check account change is an explicit
replay of an unresolved command, not a refresh control.

The shared flow and view are `apps/shared/app/usage/`; `AppProvider.swift` owns
protected SDK storage and carriage, using the existing account API/device
service. Tokens remain on the computer. Direct paired/local account control
retains explicit provider enablement and its exact existing grant scope.
