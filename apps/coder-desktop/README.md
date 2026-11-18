# Coder for desktop

Coder is a Coax Native desktop app. It targets macOS first. Today it is a
window with the message bar: the shared chat composer
(`apps/shared/composer/surface.coax`, drawn by `components/kit/composer.coax`)
on the shared GPU carrier and Mac host, with the shared AppKit editor
(`apps/shared/native/KitEditorMac.swift`) over its text.

Send is disabled, because nothing receives a message yet.

## Build and open

```sh
apps/coder-desktop/build.sh --open
```

The app is `apps/coder-desktop/build/Coder.app`, signed ad hoc. Its bundle
identifier is `com.openagents.coder`.

## Files

| File | What |
| --- | --- |
| `app.coax` | The compiled Coax: the composer surface and the atlas tables |
| `carrier.c` | The shared component carrier, with the composer surface |
| `App.swift` | The AppKit lifecycle, the window, the menus and the editor's placement |
| `build.sh` | Coax to C and Metal, then clang, swiftc and codesign |
