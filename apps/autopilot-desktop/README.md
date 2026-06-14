# Autopilot Desktop

Autopilot Desktop is the Electrobun desktop shell for the Autopilot Coder GUI.
It is planned as a Bun-native sibling to the Pylon TUI: the Bun main process
will own local Pylon control access over loopback, while the webview renders the
operator interface.

The UI is intended to reuse the same Effect and Foldkit stack as
`apps/openagents.com`, with typed bun<->webview RPC as the boundary between
credentialed local control and public-safe projections.

## macOS Signing And Notarization

Build the app with:

```sh
bun run --cwd apps/autopilot-desktop build
```

Then sign and notarize the built `.app` with Apple Developer ID:

```sh
OA_DESKTOP_APP_PATH="/path/to/Autopilot Desktop.app" \
OA_DEVELOPER_ID_APPLICATION="Developer ID Application: OpenAgents, Inc. (...)" \
bun run --cwd apps/autopilot-desktop notarize:macos
```

`scripts/notarize-macos.sh` signs with hardened runtime, submits the archive to
`xcrun notarytool`, staples the ticket, and verifies with `spctl`. Notary auth
uses either `OA_NOTARY_KEYCHAIN_PROFILE` or the App Store Connect API key in the
workspace `.secrets/appstoreconnect.env`.

## Desktop Update Feed

After notarization, stage a full artifact plus an optional BSDIFF delta for
`updates.openagents.com`:

```sh
bun run --cwd apps/oa-updates desktop:publish -- \
  --channel stable \
  --version 0.0.2 \
  --artifact ./AutopilotDesktop-0.0.2.zip \
  --previous-version 0.0.1 \
  --previous-artifact ./AutopilotDesktop-0.0.1.zip
```

The script writes content-addressed artifacts and `desktop-dist/releases.json`.
Deploy `apps/oa-updates` with `OA_DESKTOP_RELEASES_DIST=/app/desktop-dist`, and
desktop clients read `https://updates.openagents.com/desktop/stable/feed.json`.

Pricing is TBD.
