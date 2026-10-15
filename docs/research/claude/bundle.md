# Bundle and resources

## Bundle map

The inventory of the application directory is complete within the walk that
was run. The projection of that inventory is partial. Roles below follow
path conventions. Identifiers in the other chapters come from the plists,
not from the path.

| Role | Path |
| --- | --- |
| Application | `.` (`Contents/MacOS/Claude`) |
| Nested application | `Claude Helper.app`, `Claude Helper (GPU).app`, `Claude Helper (Renderer).app`, `Claude Helper (Plugin).app` |
| Framework | Electron Framework, Mantle, ReactiveObjC, Squirrel |
| Helper application | `Contents/Helpers/Claude iOS Sim.app` |

Bare helpers, not app bundles: `app-cu-helper`, `chrome-native-host`,
`disclaimer`, `permission-fixer`.

Component counts: 11 bundle-metadata records, 39 executables, 482 framework
entries, 4 native libraries, 3,892 JavaScript entries, 11 signing records,
0 privileged helpers, 0 launchd plists, 4 bare helpers.

Native libraries named by the inventory: `libffmpeg.dylib`,
`libvk_swiftshader.dylib`, and `libmsalruntime_arm64.dylib`. The MSAL
library is recorded both under the archive path and under
`app.asar.unpacked`.

Fourteen symlinks are the standard `Versions/Current` and unversioned names
for Electron, Mantle, ReactiveObjC, and Squirrel. Symlink targets were not
walked separately; the bundles are reported at their real paths.

Runtime families recorded for the bundle are JavaScript and native. That
classification comes from file formats and framework paths. It is not an
observation of a process loading them. A framework path does not by itself
establish Swift or Objective-C. Language claims in [native modules](native.md)
come from the Mach-O metadata.

The inventory also produced path-based bridge hypotheses: 151,788
candidates, 9,813 emitted, 141,975 omitted. Every
emitted candidate has the same basis, a JavaScript file paired with a
native file. The most common native paths are each paired with 252
JavaScript components, including every helper executable and Electron
Framework. That even split is the projection repeating one rule. It is not
evidence that each helper is called from 252 scripts.

## Interface and icon assets

The decoded Interface Builder document is Electron's stock main menu. The
graph has 16 nodes: 10 unknown, 3 controls, 1 scene, 1 action, and 1
Objective-C selector. Class and label strings include `NSApplication`,
`AtomApplication`, `NSFontManager`, `Main Menu`, `Open Recent`, `Quit`, and
`terminate:`. The archive path names
`Electron Framework.framework/.../MainMenu.nib`. A target-action selector
is not proof the binary implements it. Constraints and pixel layout were
not decoded.

`Contents/Resources/Assets.car` has 32 records. Image bytes were not
extracted. Names: `Claude_Assets/Color-1` through `Color-5`, `Gradient-1`,
`Gradient-2`, `Claude_Assets/spark` (rendition `image.svg`), `Claude` (PNG
renditions at 16, 32, 64, 128, 256, 512, and 1024, plus `Claude.iconstack`
and IconGroup), `CameraCursor` (`CameraCursor.svg`), `ClaudeSpark`
(`ClaudeSpark.pdf`), `Noise` (`Noise.png`), and two `ZZZZPackedAsset`
renditions.

`Contents/Resources` also contains `electron.icns`, a `fonts/` directory,
many `*.lproj` directories, and locale JSON files such as `en-US.json` and
`fr-FR.json`. The strings inside those locale files were not extracted.

## Quick actions

`Contents/Resources/quick-actions/Info.plist` registers an `NSServices`
message `runWorkflowAsService`, icon `NSActionTemplate`, menu-item title
`default` = `__TITLE__`. The same plist repeats the Electron archive
integrity hash for `Resources/app.asar`. `document.wflow` beside it is not
a property list and was not decoded.

## ion-dist

`Contents/Resources/ion-dist` is a second web tree, separate from
`app.asar`. Static parsing stopped after
`assets/v1/cef1e6037-DkhsUYfO.js`. Remaining sources were not analyzed. The directory digest on that partial record is
`ec34292b7d85207e70a6246a2d60cd2bb9c8d51cf3c85e255520ae79496881fb`. The
partial record's integrity-contradiction list was empty in the header that
was read.

Top-level names from a directory listing:

- `_frame-rt/`
- `assets/`
- `audio/`
- `i18n/`
- `images/`
- `index.html`
- `frame-connect.html`
- `frame-manage.html`
- `frame-shell.html`
- `custom-3p-gated-imports.json`
- `scc.json`
- `robots.txt`
- `third-party-notices.txt`
- `favicon.ico`, `favicon-internal.ico`, `favicon-staging.ico`

What those HTML files load was not established. The names `frame-connect`,
`frame-manage`, and `frame-shell` are the visible entry points of this
second UI.

## Browser shim

`Contents/Resources/claude-browser-shim.js` is 6,568 bytes. Despite the
extension, the file is a shell/JavaScript polyglot: the opening lines are
shell that Node treats as comments. The file's own comments call it the
`$BROWSER` shim for Claude desktop Code sessions.

Behavior read from the file:

- If the argument is an `http` or `https` URL, `node` is on `PATH`, and
  `CLAUDE_BROWSER_BRIDGE` is set, the shell re-executes the same file under
  Node. Otherwise it clears `BROWSER` and `CLAUDE_BROWSER_BRIDGE` and runs
  `/usr/bin/open` on Darwin, or `xdg-open` elsewhere.
- The Node body refuses Node older than 14 and exits 3. That floor is
  independent of the desktop package's `engines.node` of `>=22`.
- It relays only an absolute `http(s)` URL with an empty username and
  password. `file://` and other forms go to the operating-system opener.
- `CLAUDE_BROWSER_BRIDGE` must parse as JSON with a non-empty `socket`
  string and a `key` of 64 hex characters.
- The request is one JSON line `{v: 1, key, url}`, at most 8,192 bytes,
  written to that socket. The reply wait is 3,000 ms. The reply cap is
  1,024 bytes.
- A reply whose `result` is `pane` returns success without opening the
  system browser. `declined`, a malformed reply, or no reply falls through
  to the system opener.
- The opener child does not inherit `BROWSER` or `CLAUDE_BROWSER_BRIDGE`.
- `CLAUDE_BROWSER_BRIDGE_PRINT=1` prints the chosen opener instead of
  running it. The comment calls that a test hook.

The socket was not contacted. No Code session was running. The script is
the handoff from a terminal or tool that honors `$BROWSER` into the
desktop's browser pane, with the system browser as the fallback.

## iOS Simulator sandbox profile

`Contents/Resources/claude-ios-sim.sb` is 11,063 bytes of UTF-8. After a
long comment it begins `(version 1)` and `(deny default)`. The comment is
the author's description. This pass did not apply the profile.

The comment says:

- The profile is for the `claude-ios-sim` helper.
- The helper hosts FBSimulatorControl, which loads the private
  CoreSimulator framework and drives a simulator over XPC.
- The helper is intended to run out of process, and to be spawned through
  `disclaimer` so a compromise of the helper does not receive the app's
  screen-recording or accessibility grants.
- Direct network sockets, and file-content reads under the home directory,
  `/Users`, and `/Volumes`, are denied outside CoreSimulator state and the
  helper's own bundle. The comment names a preference-read carve-out for
  `.GlobalPreferences` and `com.apple.CoreSimulator`.
- `mach-lookup`, `mach-register`, `ipc-posix-shm`, and `iokit-open` are
  allowed without a name or class filter. File metadata reads are not
  denied. The comment says a missing rule hangs CoreSimulator rather than
  failing loudly, and that the rules were fitted by running the helper.

The parts that can be checked from other files agree with that description.
`claude-ios-sim` links private CoreSimulator and
AccessibilityPlatformTranslation. The helper is a background-only bundle
with minimum macOS 15.0. `disclaimer` imports
`responsibility_spawnattrs_setdisclaim`. The allow-list body is not copied
here.

## Disk images

`Contents/Resources/smol-bin.arm64.img` is 24,117,248 bytes.
`smol-bin.x64.img` is 25,165,824 bytes. Both start with the bytes
`eb 76 90` followed by the ASCII `EXFAT`. The filesystems were not
mounted. The name, the size, and the Virtualization / `vmnet` linkage of
`swift_addon.node` make a guest-disk reading plausible. The image contents
were not walked, so that use is not demonstrated.
