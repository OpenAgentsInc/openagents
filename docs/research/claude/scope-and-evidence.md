# Scope and evidence

## What was inspected

The subject is the installed application bundle
`/Applications/Claude.app`, version `2.37337.1`. A read-only installer image
was mounted at `/Volumes/Claude` during the investigation and was not written.

Inspected surfaces:

- Bundle layout, Info.plist files, the asset catalog, the Electron main-menu
  archive, and the Services registration under `quick-actions`.
- The code signature, entitlements, and the embedded Developer ID
  provisioning profile.
- The Electron fuse record inside Electron Framework, on both the x86_64 and
  arm64 slices.
- The ASAR archive header and `package.json`, plus a static parse of the
  JavaScript in that archive.
- The preload scripts and the main-process chunk, read as text: window
  options, the `claude://` handler, the VM controller, custom-scheme
  registration, the artifact document handler, and the managed-config schema.
  `ion-dist/index.html`, `frame-shell.html`, `frame-connect.html`, and
  `frame-manage.html` were read. The hashed modules those documents load were
  not reconstructed function by function.
- Mach-O identity, linkage, exports, and embedded Go or Rust compiler metadata
  for the Claude-specific binaries.
- A static overview of the arm64 slice of the host stub and of the smaller
  Claude-specific images: disclaimer, permission fixer, Chrome native host,
  computer-use helper, computer-use addon, native binding, `pty.node`, and
  the MSAL native addon.
- Directory names of `ion-dist`, the browser-shim script, the iOS Simulator
  seatbelt profile, and the first sector of the two `smol-bin` images.

## How to read a claim

| Mark | Meaning |
| --- | --- |
| Observed | The value is a field, byte, string, hash, or count taken from a shipped file. |
| Derived | The value follows from an observed Electron fuse or from a documented layout rule, without a process having been run. |
| Inferred | The value is a reading across several observations, such as a compiler commit taken from a `/rustc/<40-hex>/` path. The inference is named in the chapter. |
| Unknown | This pass did not establish it. A zero in a coverage summary is a coverage result, not proof that a feature is absent. |

Static structure is not observed execution. The application was not launched.
No Electron debug port was opened, no browser-shim socket was contacted, and
no sandbox profile was applied.

## Identity of the inspected files

| File | Whole-file SHA-256 |
| --- | --- |
| `Contents/MacOS/Claude` | `53aa57b29cbb0340cd2685b5f0d036c6987810e02d7e38dbfbec42cd3c3928cc` |
| `Contents/Resources/app.asar` | `054f8902cfbcf4757a220c13828a29892ed62390cc08cbfafd85693188dbefdd` |
| `Contents/Frameworks/Electron Framework.framework/Versions/A/Electron Framework` | `cc93bddf5973f6f92f0e91d7df5152c6c937cb52508b08aa089d228041ac0685` |
| `Contents/Resources/Assets.car` | `e11093b86f7636103c54b22cd5d54d32fd03c00b99ea48aa8358cd1a14b9eac0` |
| `Contents/embedded.provisionprofile` | `47163ea1a0f9af4b400a6644bb5b334b53fbff08945f8589ad3dd121ce8f4069` |
| `Contents/Resources/claude-browser-shim.js` | `b8bb071fa1e4d2da27246f7d7a52e42871c331735f6d0d270a59af31cee1b635` |

The bundle directory hash recorded by the application inventory is
`601681d5a290153f2ecb8775ba18b811dd90aa3d401f4b3224b74099215675d7`. That is a
directory digest, not the hash of any one file.

Electron's own archive-integrity record, stored in Info.plist for
`Resources/app.asar`, is SHA-256
`fa676a98c74fd9cfb7934b6516f402f7ac9f296f66deb0ec3385bec9ca5301c2`. That value
is not the whole-file hash of `app.asar`. Both are reported because they are
different fields. This pass did not execute the integrity check.

## Deliberate limits

- User support files, conversations, and credentials were not opened.
- Electron Framework was not decompiled. Its Chromium and V8 contents are
  outside this report. The six `compile-cache/*.jsc` bytecode files were not
  decoded.
- `swift_addon.node`, `github-mcp-server`, `claude-ios-sim`,
  `SimulatorFrameworkBridge-iOS`, and Squirrel have linkage and metadata, and
  do not have procedure-level overviews.
- The second web tree, `ion-dist`, was only partly parsed. Analysis stopped
  after `assets/v1/cef1e6037-DkhsUYfO.js`.
- The `.skill` archives, the Office stdio script, and the locale JSON files
  were inventoried and not read as documents.
- The two `smol-bin` images begin with an exFAT boot sector. Their
  filesystems were not mounted.
- `document.wflow` under quick actions is not a property list and was not
  decoded.

Chapter [coverage](coverage.md) lists the JavaScript graph's partial counts
next to the questions they leave open.
