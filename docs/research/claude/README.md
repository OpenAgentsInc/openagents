# Claude desktop architecture

Assessment date: 2026-10-15.

This report describes the shipped macOS Claude desktop application installed at
`/Applications/Claude.app`. The inspected build is version `2.37337.1`, bundle
identifier `com.anthropic.claudefordesktop`, signed by Anthropic PBC on 13
October 2026. It is an Electron 44.7.0 application whose product package is
`@ant/desktop`. The interface is a set of renderer documents around a small
native host. Product features that leave the web UI — computer use, Cowork,
device pairing, local terminals, a GitHub MCP server, Microsoft sign-in, an
in-app browser handoff, and an iOS Simulator helper — are separate native
binaries and archive entries beside that host.

The application was not launched. No chat, token, cookie, or
`~/Library/Application Support/Claude` file was opened. Facts below are file
contents, signatures, linkage, and static structure of this installed build.
They are not observed runtime behavior.

## Read the report

| Document | Contents |
| --- | --- |
| [Scope and evidence](scope-and-evidence.md) | What was inspected, how claims are marked, and the digests that identify the build |
| [Identity and packaging](identity-and-packaging.md) | Bundle identity, signature, entitlements, Electron fuses, document types, and the Developer ID profile |
| [Application](application.md) | ASAR layout, package manifest, windows, IPC, context bridge, workers, and packaged resources |
| [Native modules](native.md) | Host stub, Swift and Rust-like addons, helpers, Go MCP server, MSAL, and linkage |
| [Bundle and resources](bundle.md) | Frameworks, second web UI, browser shim, iOS Simulator helper, images, and assets |
| [Coverage](coverage.md) | Which surfaces are only partly described, and which questions this pass left open |

## The architecture in short

1. **A thin Electron host.** `Contents/MacOS/Claude` links only Electron
   Framework and the system C library. Its arm64 executable text is 348 bytes
   and 16 procedures. Electron Framework is version 44.7.0 and about 402 MB
   universal. The real application is `Contents/Resources/app.asar`.
2. **The app is loaded only from that archive, with Node debug switches
   fused off.** Both architectures disable running as plain Node and disable
   Node's `--inspect` arguments. Both enable an embedded archive-integrity
   check and load-only-from-ASAR. Cookie encryption and extra `file:`
   privileges are on.
3. **The desktop UI is several windows, not one page.** Static analysis
   counted 14 browser-window constructions. Six HTML documents ship: main,
   buddy, about, find-in-page, quick, and a local-exec consent window. Main
   build files also name computer-use teaching, watch-record, Cowork
   artifacts, a design window, and a page preview.
4. **Privileged work is native and out of process.** Computer use is a Swift
   addon linked to ScreenCaptureKit and Accessibility. A second Swift addon
   links Virtualization, `vmnet`, and a Go userspace network stack. A Rust-like
   N-API addon links Accessibility and Security. A small helper spawns a child
   with the system TCC-disclaim API. An iOS Simulator helper links private
   CoreSimulator and ships a deny-default sandbox profile.
5. **Integrations are files in the archive.** A universal Go binary implements
   GitHub's MCP server `v1.6.0`. An Office directory carries an MSAL runtime
   and a stdio MCP script. Bundled `.skill` files are zips of the exported
   type `com.anthropic.skill`. Mermaid, Chart.js, and Grid.js ship as artifact
   libraries.

## What this report is not

It is not a line-by-line reconstruction of the minified interface, and it is
not a claim about what a signed-in session does. The JavaScript graph is
partial: large hashed bundles kept static facts and were not fully expanded.
Endpoint-shaped strings in that graph are property and call names, not
confirmed servers. A compiler path embedded in a binary identifies
a compiler commit; it does not authenticate package versions.
