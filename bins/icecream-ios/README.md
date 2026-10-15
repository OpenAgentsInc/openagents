# The Ice Cream Man for iPhone and iPad

A thin UIKit app (`host/`) around one full-screen `WKWebView` that runs the
browser game from `crates/icecream-web`. The game's wasm/js bundle ships
inside the app (`host/App/game/`, built, not checked in) and is served from
`icecream://game/index.html` by a `WKURLSchemeHandler` (`GameScheme.swift`)
with real MIME types, so ES modules and `WebAssembly.instantiateStreaming`
work offline. Nothing is fetched from the network.

- Bundle ID `com.openagents.icecreamman`, team HQWSG26L43, publisher
  OpenAgents, Inc., display name "The Ice Cream Man". Universal (iPhone and
  iPad), iOS 16+, full screen, all orientations except iPhone upside down,
  a LaunchScreen storyboard and a UIScene, so iPad never runs it in
  compatibility mode. iPadOS 26's "Windowed Apps" multitasking ignores
  UIRequiresFullScreen, so the scene's minimum size is the screen (the
  window opens full size); "Full Screen Apps" mode is edge to edge.
- Status bar hidden, home indicator auto-hidden, no zoom (scroll view and
  pinch off, viewport re-asserted), edge to edge; the page handles safe
  areas with `viewport-fit=cover` and `env(safe-area-inset-*)`.
- Saves (`icecream.settings.v1`, `icecream.best.v1`, `icecream.tutorial.v1`)
  are the page's localStorage in the default persistent data store.
- Links in the game are site-relative; the app opens them as
  `https://openagents.com/<path>` in Safari and never navigates away.
- Game controllers (`PadBridge.swift`): left stick / d-pad move (w a s d;
  the d-pad also sends arrows for the tutorial), A jump (Space, Enter),
  B use (e), X next item (q), Y camera view (v), left shoulder or left stick
  press sprint (Shift), right trigger or right shoulder fire (a mouse press
  on the canvas), right stick turns the camera (a right-button drag), Menu is
  Escape. Hardware keyboards go straight to the page.
- The app puts `data-version="ios VERSION (BUILD)"` on `#icecream-canvas`
  when the page has one without a version.

## Build

    export CARGO_TARGET_DIR=~/work/openagents-target-icecream2
    bins/icecream-ios/build.sh sim       # game + simulator build, install, launch
    bins/icecream-ios/build.sh archive   # game + signed App Store archive
    bins/icecream-ios/build.sh ipa       # signed .ipa from the archive, no upload
    bins/icecream-ios/build.sh upload    # upload the archive to App Store Connect
    bins/icecream-ios/build.sh clean     # remove DerivedData and exports

`game` runs `scripts/build-icecream-web.sh --with-page` (wasm32 target and
the Cargo.lock-matching `wasm-bindgen` CLI) and copies the output, minus the
`.gz` copies, into `host/App/game`. `ICECREAM_IOS_BUILD_NUMBER=N` overrides
`CURRENT_PROJECT_VERSION` in `host/project.yml`.

## App Store Connect

`asc.py` (reuses `scripts/release/asc.py`'s key handling; load
`~/work/.secrets/appstoreconnect.env` first):

    python3 bins/icecream-ios/asc.py register     # bundle ID (done: VLBKKBB77F)
    python3 bins/icecream-ios/asc.py profile      # "The Ice Cream Man App Store" profile
    python3 bins/icecream-ios/asc.py app          # app record id, or exit 3 if missing
    python3 bins/icecream-ios/asc.py next-build
    python3 bins/icecream-ios/asc.py wait-valid --build N   # then adds it to internal groups
    python3 bins/icecream-ios/asc.py metadata     # pushes metadata/*.txt

The app record itself is created by the owner in App Store Connect (the API
cannot); see `~/work/NEEDS_OWNER.md`. Release, once it exists:

    ICECREAM_IOS_BUILD_NUMBER=$(python3 bins/icecream-ios/asc.py next-build) bins/icecream-ios/build.sh archive
    bins/icecream-ios/build.sh upload
    python3 bins/icecream-ios/asc.py wait-valid --build N

`metadata/` holds the store text, and `metadata/review.md` the privacy,
age-rating (9+), category, and review-note answers. `screenshots/` holds the
iPhone 6.9" and iPad 13" shots (Simulator; launch arguments
`-IceCreamFragment 'bot&seed=7' -IceCreamLandscape YES` start the game's bot
in landscape). `icon.py` draws the app icon.
