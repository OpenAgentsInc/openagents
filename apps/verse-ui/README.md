# Verse UI

Owned shadcn/ui components, React, Effect 4, and Effect Atom for the Verse
presentation (#12576, owner-authorized 2026-10-23).

- Covers `/new-lander`, universe entry, the native house editor, first-edit
  sign-in and confirmation, publishing/sharing, personal-world setup, and visits/chat.
- Rust still renders semantic forms and owns account, project, permission,
  save, publication, invitation, voice, and engine controllers. CSS and the shared
  button, input, textarea, and native-select styles skin those existing elements without replacing their identity.
  Effect owns the presentation lifecycle; Atom supplies available tools to the
  React/shadcn keyboard palette. No React render recreates the engine iframe or
  face-paint canvas. Native dialogs retain their existing focus/confirmation behavior.
- This is an incremental presentation migration, not a second domain client.
  The palette only selects existing visible, enabled navigation/tool controls;
  it never confirms an edit, publish, or payment. Dynamic Rust-created controls
  receive the same styles. The original forms work without the JavaScript bundle.
- Aethyrforge remains a separately pinned private runtime. This package neither
  imports private source nor changes its grants or protocol. `/` stays unchanged.
- The external sign-in provider keeps its own screen. This package styles the
  in-Verse entry and first-edit sign-in cards, plus `/login` and `/signup`
  when they return to the Verse. Those authentication pages load CSS only;
  provider actions, CSRF fields, and response status stay with Rust.

Run `npm ci`, `npm run build`, and `npm test` from this directory. Commit the
source, lockfile, and generated `crates/openagents-web/content/verse-ui/` assets
together. The Rust build embeds these checked-in outputs, so it requires neither
Node nor private repository access. Rebuilding the assets must be deterministic.

The UI source comes from the official shadcn registry (MIT); see
[shadcn/ui](https://github.com/shadcn-ui/ui/blob/main/LICENSE.md). Generated browser
code retains dependency license notices. Do not import private screenshots or
vendor art into this package.

For visual checks of account-only screens, set `OPENAGENTS_VERSE_UI_CAPTURES`
to a scratch directory while running `cargo test -p openagents-web --lib
verse_studio::projects::tests`. The test-only exporter retains the actual
server-rendered setup, invitation, and player markup with synthetic test data.
These captures qualify presentation, not live authentication or payment.

The owned dialog overlay uses static CSS for scroll locking, avoiding Radix's
runtime stylesheet injection under the host's strict CSP. Radix Content still
handles modal focus, outside dismissal, and returning focus. Escape in the tool
finder does not also close the native workspace.

With a configured local engine, run the browser regression under its own profile:

```sh
VERSE_UI_REVIEW_URL=http://127.0.0.1:PORT \
VERSE_UI_CAPTURE_DIR=/absolute/scratch/verse-ui \
openagents browser run -- node apps/verse-ui/tools/verify.mjs
```

It checks real guest entry and first-edit sign-in/cancellation, keyboard focus,
workspace retention, engine generation/frame/pose retention, and narrow layouts.
It does not complete provider sign-in, publish anything, or record speech.

From the repository root, run the live mobile journey with the dedicated test account:

```sh
OPENAGENTS_VERIFY_OUTPUT=/absolute/scratch/verse-mobile \
openagents browser run -- node apps/verse-ui/tools/check-mobile-live.mjs
```

This check uses real production APIs and GPU rendering at a phone viewport with
touch input. It creates test projects, saves and publishes changes, opens shared
links as a guest, sets up the test account's planet, checks chat, and returns to
the saved world. It reads the private test credential file at
`~/work/.secrets/openagents-test-account.env`; it never uses the owner's account.
It opens the provider sign-in screen, then uses the existing test app credential.
Provider approval, physical iPhone Safari, and live speech recognition still need
manual checks. Captures and results go to the specified scratch directory.

The Verse flow uses a dark zinc theme: black backgrounds, zinc cards, light text,
and light primary actions. Keep the private engine controls on the same palette.
Scene materials and world lighting are independent of the interface theme.

The world HUD groups map/display controls separately from editing and saving.
Jarvis uses an instrument rail on desktop and compact tool tabs on phones.
Reload and fullscreen live in the display menu; secondary world actions have an
expandable menu. Rust renders the decorative SVG icons, and every icon control
keeps its accessible name. Run the touch-target and retained-world check after
changing this layout:

```sh
VERSE_UI_CAPTURE_DIR=/absolute/scratch/verse-hud \
openagents browser run -- node apps/verse-ui/tools/check-hud.mjs
```

Check text contrast and phone overflow with the browser audit:

```sh
VERSE_UI_REVIEW_URL=https://openagents.com \
VERSE_UI_CAPTURE_DIR=/absolute/scratch/contrast \
openagents browser run -- node apps/verse-ui/tools/contrast.mjs
```

The audit covers both homepage actions, hover and focus, login/signup, guest
entry, Jarvis tabs, the tool finder, first-edit sign-in, and both demo players at
phone and desktop widths. It runs axe contrast checks and checks computed text
colors on translucent surfaces. Inspect the saved screenshots for image
backgrounds that axe cannot measure. Set `VERSE_UI_REVIEW_CSS` to a local built
stylesheet to test before deployment, and `VERSE_UI_REVIEW_FIXTURES` to the
Rust test capture directory to include account-only cards without signing in.

## Aethyr UI protocol

`src/aethyr/` holds the Aethyr controller runtime, the React DOM renderer, and
the entry-dialog demo ([protocol](../../docs/aethyr/ui-protocol.md)). It moves
to `apps/aethyr` later. Two files there are generated from Rust; don't edit
them:

- `protocol.ts`: `cargo run -p aethyr-ui -- --write`
- `tokens.css`: `cargo run -p aethyr-tokens -- --write`

`npm test` checks `protocol.ts` against the Rust schema. Its renderer tests
open the built `ui.css` in Chromium, so run `npm run build` first. Set
`AETHYR_CHROMIUM` to a Chromium binary when Playwright's own isn't
installed, `AETHYR_CAPTURE_DIR` to keep screenshots, `UPDATE_SNAPSHOTS=1` to
rewrite `tests/snapshots/aethyr-renderer.json`, and `AETHYR_WRITE_FIXTURES=1`
to rewrite the controller views the Rust tests read.
