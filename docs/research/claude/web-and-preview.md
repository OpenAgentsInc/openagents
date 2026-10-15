# Web UI, frames, and previews

The Electron shell is small. The product UI is a second tree,
`Contents/Resources/ion-dist`, loaded in the main view. Artifact pages are a
third document, `frame-shell.html`, which the desktop hosts in a preview
partition and connects to the main process with a `MessagePort`. A preview of
a local dev server is a fourth partition family. The browser shim is how a
CLI on the machine asks the desktop to open a URL in that preview instead of
the system browser.

The `ion-dist` script graph was not fully read. What follows is the HTML
those documents ship, the preload that connects a frame, and the partition
names in the main process.

## The shipped claude.ai page

`ion-dist/index.html` is the app entry. Its own meta says `data-build-id="spa-dev"`,
`data-git-hash="local"`, `data-version="1.0.0"`, and
`data-build-timestamp="1791900498"`. Those fields do not identify the desktop
commit `ba51ab57…`. The desktop commit is on the Electron bundles. The page
and the shell are pinned separately.

The head reads `desktopBootFeatures.bootPlaceholder`. When the status is
`supported`, the document sets `data-desktop-boot-placeholder` and waits for
`data-boot-ui-ready`. Until then the boot surface is transparent, a
ten-second timeout reveals it anyway, and a fixed drag strip
(`#desktop-boot-drag-strip`, `app-region: drag`, 48px tall) is the only hit
target. `desktopPreferredLanguages` from the same boot JSON overrides
`navigator.languages` for CJK font ordering.

A stale-entry script reads `data-ion-served-at` and `data-ion-entry-cache`.
If the entry was served more than a day ago and the navigation is a reload
candidate, it writes `claude:stale_entry_reload` to session storage and
reloads once. That is the page protecting itself from a cached shell. It is
not the Electron compile cache.

On `/`, `/new`, and `/login`, if a session-marker cookie is present, the
page preloads a chat draft from IndexedDB database `keyval-store`, object
store `keyval`, key `store:chat-draft:chorus-unified-composer`, and the
react-query cache under `react-query-cache`. On `/chat/<uuid>` it preloads
`claude-conversation-store`, object store `trees`. On
`/cowork/…`, `/local_sessions/…`, or `/cowork_sessions/…` the id is
`cowork:cse_<id>` where the path id matches `session_` or `cse_`. The cowork
preload only runs when the user agent looks like the desktop app and
`claudeAppBindings` exists, and when the retention record's `products` array
includes `cowork`. A pending `conversation-store:clear-pending` key skips the
read. The cookies and the database were not opened. They are names in the
HTML.

The static composer is in the HTML, disabled, with the placeholder "How can
I help you today?". Layout variants in the same markup are a classic
sidebar, a rail sidebar, and a composer arm named `marlin`. A jurisdiction
disclosure block is present and hidden. The module entry is
`/assets/v1/index-DthvVvF0.js`, preloading the page's own copies of the
schema, router, editor, and query libraries. `frame-shell-entry` points at
`/assets/v1/frame-shell-BrIqb8O3.js`.

Route shapes the boot script knows, which match the deep-link table:

- `/chat/<uuid>`
- `/cowork/<session_ or cse_ id>`, and the same under `local_sessions` and
  `cowork_sessions`
- `/new`, `/login`, `/`

## The frame shell

`frame-shell.html` is titled Artifact. `frame-connect.html` is titled Connect.
`frame-manage.html` is titled Share. Connect and Share are 440px cards.
Artifact is the full shell: a header, a loading slot, an error panel, and an
iframe slot `#frame-content`.

The inline boot script reads `window.claudeDesktopArtifactPane` when the
desktop preload has set it, otherwise the query string. It copies `mode`
(`light`, `dark`, `system`), `platform` (`web`, `desktop`), and `font`
(`anthropic`, `system`) onto the document. Embedded plus `chrome=none` hides
the header and sets the viewport to cover. Platform `desktop` selects the
host-capability token `host-tools`. The capability tokens the shell will
keep, if requested, include:

`comment-mode`, `comments-list`, `comment-composer`, `comment-threads`,
`page-comments`, `artifact-nav`, `artifact-nav-vanity`, `comment-summon`,
`open-in-claude`, `open-chat`, `stage-in-chat`, `cloud-session`,
`cowork-task`, `no-send-all`, `summon-into-chat`, `summon-into-session`,
`summon-into-channel`, `connector-off`, `confirm-page-sends`,
`viewer-context`, `chrome`, `export`, `page-foot`, `chrome-readonly`,
`selection-menu`, `chat-beside`, `claude-tips`, `header`, `duplicate`,
`sheet-arrow`, `sheet-backdrop`, `presence`, `host-tools`, `context-card`,
`context-send`, `context-send-held`, `host-nav`, `url-anchor`,
`edge-to-edge`, `edge-to-edge-sides`, `keyboard-done`, `page-background`,
`scroll-chain`, `save-file`, `save-blob`, `host-keys`, `device-camera`,
`device-microphone`, `device-speech`, `device-geolocation`, `device-motion`,
`device-screenshare`, `device-midi`, `transparent`, `host-messages`.

The desktop preload passes `embedded: true`, `platform: "desktop"`,
`chrome: "none"`, and an optional `hostcaps` object. The shell then treats
`host-tools` as the desktop capability. The other tokens are the web and
mobile vocabulary in the same file. Which of them the desktop actually sets
depends on the object the main process passes into `hostcaps`. That object
was not recovered field by field.

Boot fetches `/api/frame/<uuid>` with credentials and these headers:

| Header | Value |
| --- | --- |
| `X-Frame-CP` | `go` |
| `X-Frame-Platform` | query `platform`, else `desktop` when the user agent contains `Electron/`, else ios, android, or web |
| `X-Frame-Surface` | query `surface` if it is `chat`, `cowork`, `code`, `slack`, `teams`, or `standalone`; a top frame defaults to `standalone` |
| `X-Frame-Client-Version` | the `build-timestamp` meta (`1791900498`) |
| `X-Frame-Session-Id` | a random id for the page load |

Org is chosen from the query, then session storage, then a localStorage hint
keyed by the frame uuid, then a `lastActiveOrg` cookie. A vanity slug is the
filename with the uuid or a 22-character id stripped. The request adds `via`,
`bk` (`initial` or `reboot`), `actor=id`, and `vt=1`. A parent frame may
hand over a prefetch it already started (`parent.__frameHostBoot`). After
the JSON returns, a hidden sandboxed iframe can load
`https://<frameUchost>/_t?__frame_t=<assetToken>` when the response says
token exchange and pregrant are on and the contract version is new enough.
A second hidden iframe loads `/_warmup` on that host. A 60 second watch
replaces the spinner with "Artifact took too long to load". A module script
that fails to load says "Couldn't load artifact".

`frame-runtime-pin` is `manifest.94ea33548242e2e0.json`. Locale catalogs load
from `/i18n/frame-shell/<locale>.json`. The shell's module is
`frame-shell-BrIqb8O3.js`, with deferred and broker chunks beside it.

This fetch is same-origin. In the desktop, "same origin" is whatever
partition loaded the frame shell. It is not, by itself, a call to
`claude.ai` from the main process. The main process can still be the thing
serving that origin through `protocol.handle("https")`. Both are in the
build. Which requests the handle function answers, versus which it lets the
network stack send, was not fully traced through every branch.

## How the desktop injects the port

`claudePagePreview.js` runs in the preview web contents.

On the top frame it exposes `claudeDesktopArtifactPane` and listens for
`window.message`. The message must come from `window` itself, must match
`location.origin`, and must look like `{__frame_host: true, kind: "hello",
v: 1}`. The preload then `ipcRenderer.send`s `claude-page:host-tools-hello`.
The host replies on `claude-page:host-tools-port` with a `MessagePort` in
`event.ports[0]`. The preload posts `{__frame_host: true, kind: "port", v: 1}`
back to the page, transferring that port. The page's `host-tools` capability
is this port. The preload does not parse the port's messages.

On a subframe the preload does not expose the pane. A trusted `pointerdown`
is forwarded as `claude-page:subframe-pointerdown`, at most once a second.
Pointer lock is watched. The host can send
`claude-page:subframe-exit-pointer-lock`, and the preload walks every
same-origin descendant frame and calls `exitPointerLock`, retrying for 15
ticks of 10 ms. That exists so a preview playing a game or a video does not
keep the pointer when the host wants it back.

The main-process side of the same names sits next to the webauthn gate and
the `in-app-browser` partition. A permission request from a Claude-page
preview is blocked unless a narrower allow in that function matches. The
frame shell's device capabilities (`device-camera`, `device-microphone`, and
the rest) are therefore requests the preview still has to win through the
permission callback. They are not granted by appearing in the HTML.

## Launch previews

A local server preview uses a different partition per job:

- `persist:launch-preview-`
- `persist:launch-preview-session-`
- `persist:launch-preview-static`
- `persist:launch-preview-cowork-shared`

`Launch` is the service, 107 methods. The ones that define the preview:
`showPreview`, `hidePreview`, `focusPreview`, `destroyPreview`,
`refreshPreview`, `setPreviewViewport`, `clearPreviewViewport`,
`setPreviewChromeFocus`, `getPreviewUrl`, `getPreviewNavState`,
`getPreviewTabs`, `getPreviewAllowedOrigins`, `addPreviewAllowedOrigin`,
`clearPreviewAllowedOrigins`, `goBack`, `goForward`, `capturePreviewScreenshot`,
`capturePreviewScreenshotIfChanged`, `getPreviewLastFrame`,
`discardPreviewLastFrame`, `armPreviewLiveStream`, `exportPreview`,
`elementSelected`, `closePreviewTab`, `closeAllPreviewTabs`,
`focusOrOpenPreviewTab`, `teardownRemoteSessionPreview`. Remote panes:
`admitRemotePaneOriginOnce`, `ensureRemoteBlankPreview`,
`ensureChatBlankPreview`, `setRemoteSessionViewed`. Data for the preview:
`clearPreviewBrowsingData`, `clearCoworkBrowserData`.

Media inside a launch preview is the camera/microphone allow in the
permission callback. A denial emits
`desktop_launch_preview_media_permission_denied`. Log tags around the policy:
`LaunchPreview`, `LaunchPreviewManager`, `PreviewPolicy`,
`PreviewOriginPolicy`, `PreviewIsolation`, `PreviewBlocklist`,
`PreviewContext`, `RemotePaneConsent`, `RemotePreviewConsent`,
`preview ssrf`. The SSRF tag is the evidence of a check. The check's host
list was not extracted.

`getChromeImportAvailability`, `getCoworkChromeImportAvailability`,
`acceptChromeImportSharedOffer`, and `getChromeImportSharedOffer` sit on this
same service because a preview can be seeded from Chrome. Server candidates
for a local dev server are `detectServerCandidates` and
`detectServerCandidatesWithAgent`.

The `claude-browser-shim.js` script is the other side of "open this URL in
the preview". It is a shell script and a JavaScript program in one file. If
the URL is http(s), Node is available, and `CLAUDE_BROWSER_BRIDGE` is set,
it re-executes under Node. The bridge value is JSON `{socket, key}` and the
key is 64 hex characters. It writes one line, `{v: 1, key, url}`, at most
8192 bytes, to a unix socket, and waits up to 3000 ms for at most 1024 bytes
back. A reply of `pane` means the desktop accepted the URL into the in-app
browser. Anything else falls through to `/usr/bin/open`. The child
environment strips `BROWSER` and `CLAUDE_BROWSER_BRIDGE`. Only absolute http
and https URLs without userinfo are sent. The socket was not contacted. The
`pane` reply is the same word as the preview surface the `Launch` service
shows.

## What is still the web app's business

Chat streaming, the composer after hydration, project settings, and the
admin pages are the `ion-dist` modules. Their route entry is the HTML above.
Their implementation is the hashed assets, which this pass did not read
function by function. The desktop's contribution on those routes is the
bridge: boot features, deep links, file pickers, notifications, and the
session services. A behavior that only exists after `index-DthvVvF0.js`
evaluates is not claimed here.
