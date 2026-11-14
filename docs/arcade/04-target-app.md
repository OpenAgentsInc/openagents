# Arcade port, part 4: the target app (OpenAgents iOS and Coax Native)

Audited at `openagents` commit `450ca4738f941d5ff7b6141812a2f89a0e3b4f0f`
(origin/main, 2026-11-14). Issue: #13607.

**Rendering addendum, 2026-11-14:** [audit 06](06-rendering-and-animation.md)
and [components/rendering.md](../../components/rendering.md) specify the new
GPU component/motion and mixed-surface target. The installed UIKit runtime
described here remains real; the proposed target is not an OTA capability of
that runtime. The dated requirements at the end supplement this fixed audit.

Another thread is building OpenAgents iOS right now. Recent commits touch the
Projects and Usage screens. This doc describes `main` at the commit above. Check
`apps/openagents-ios/README.md` for builds after 1085.

**Verified** means I read the code or doc named. **Inferred** means I drew a
conclusion from code without running it. This audit ran no builds, simulators or
devices.

Parts 1 to 3 audit Arcade itself. This part covers the app that Arcade's Nostr
chat ports into. Arcade, as read lightly here (`arcade/README.md`,
`arcade/package.json`, `arcade/app/screens/`, `arcade/app/navigators/`), is an
Expo 48 / React Native 0.71.8 app at version 0.3.1. It uses `nostr-tools`
1.11.1, `react-native-quick-sqlite` (through `any-sqlite`) and
`expo-notifications`. It has a tab bar (`TabNavigator.tsx`, `TabBar.tsx`) and
about 28 screens: channels, direct messages, contacts, relays, profile, listings,
AI, backup and login.

## Contents

1. [OpenAgents iOS today](#1-openagents-ios-today)
2. [Coax Native for a newcomer](#2-coax-native-for-a-newcomer)
3. [Can coax/lib/nostr run in the app?](#3-can-coaxlibnostr-run-in-the-app)
4. [Other mobile surfaces and chat storage](#4-other-mobile-surfaces-and-chat-storage)
5. [Requirement fit](#5-requirement-fit)
6. [Binding constraints](#6-binding-constraints)
7. [Risks and recommendations](#7-risks-and-recommendations)
8. [Files read](#files-read)
9. [Open questions](#open-questions)

## 1. OpenAgents iOS today

### 1.1 Build and identity (verified)

| Fact | Value | Source |
| --- | --- | --- |
| Current build | 1.0.0 (1085), "native Projects" | `apps/openagents-ios/project.yml` (`CURRENT_PROJECT_VERSION`), `apps/openagents-ios/README.md` |
| Bundle ID / team | `com.openagents.app` / HQWSG26L43 | `project.yml` |
| Minimum OS and devices | iOS 18.0, iPhone and iPad (device family 1, 2) | `project.yml` |
| UI stack | UIKit host plus JavaScriptCore running Coax compiled to Wasm and then to JS (since build 1070). There is no WebView in the app shell. | `README.md`, `coax/native/README.md` |
| Swift package | Breez SDK Spark 0.27.2 (read-only balance) | `project.yml` |
| Embedded frameworks | SpliceBotsRuntime, EvergladeRuntime, EvergladeForestRuntime, MadeRealmRuntime (Metal worlds) | `project.yml` |
| Appearance | Dark only (`UIUserInterfaceStyle: Dark`) | `project.yml` |
| URL scheme and links | `openagents://`; applinks and webcredentials for openagents.com and staging | entitlements, `Info.plist` |
| Push | `aps-environment` = `development` only, and no registration code | `OpenAgents/*.entitlements` line 5 |
| Usage strings | Camera, Face ID, microphone, local network | `Info.plist` |

Build history (README): 1068 WebKit replacement app → 1069 Bitcoin balance → 1070
UIKit/JSC → 1071 composer and account chat → 1072 native messages → 1073 update
deduplication → 1074 Your computers → 1075 streamed worlds (frozen at tag
`openagents-ios-1075`, 18.7 MB IPA) → 1076/1077 link fixes → 1078 Everglade →
1079 work rows → 1080 full-screen world → 1081 mono HUD → 1082 history and
detail navigation → 1083 quiet Chat home and drawer → 1085 native Projects.

Many Swift folders in `apps/openagents-ios/OpenAgents/` are **not compiled**.
These include the legacy `HomeViewController`, Voice, the wallet send/receive
screens and `HauntViewController`. The only app files in the target are
`SignIn/PhoneSignIn.swift`, `SignIn/NostrKey.swift`, `AppAnalytics`,
`Platform/NostrSign.c`, `SparkHost`, `Wallet/WalletKeychain.swift`, `BIP39`,
`Wallet/BreezKey.swift` and `Shared/` (`project.yml` sources list). Do not treat
the legacy Swift as a feature that ships.

### 1.2 Screens (verified, `home/app.coax` page numbers)

| Page | Screen | What it shows |
| --- | --- | --- |
| −1 | Loading | Runtime start |
| 0 | Log in | Email field, "Log in" (`/api/auth/email/start`) |
| 1 | Code | Six-digit code (`oneTimeCode` content type) (`/api/auth/email/verify`) |
| 2 | Home | Header "OpenAgents"; "Jump in" with Splice Bots, Everglade and Worlds links; the composer (`home/shell.coax`) |
| 3 | Chat | Account chat transcript (markdown messages, tool rows) plus the composer |
| 4 | Environments ("Your computers") | Claude Code sessions on a Mac or CoderOS, with Stop and permission prompts (`apps/shared/environment-app.coax`) |
| 5 | Projects | Project list (`home/projects.coax`, `apps/shared/projects-app.coax`) |
| 6–9 | Project screens | Drops, review, publish, grow, member chat |
| dialog | Settings | Updates, About, Usage (Claude limits, "Connect a Claude account"), Extras, Log out (`home/navigation.coax`) |
| module | Extras → Bitcoin | Read-only "N sats" (`extras/app.coax`) |
| module | Worlds | World list and builder (`worlds/app.coax`), then a full-screen Metal world with a 44-pt floating menu |

### 1.3 Navigation and chrome (verified)

- **Drawer, not tabs.** `NativeWorkspace.swift` draws a left drawer on iPhone.
  At a width of 600 pt or more it becomes a 280–380 pt sidebar. It has a 0x99
  black backdrop, an edge pan to open and a left pan to close, and it respects
  Reduce Motion.
- Drawer contents (`home/navigation.coax`): Chat / Projects / Worlds, then
  "Recent chats" with search, computer groups and an Account group, and a
  Settings footer.
- A 44-pt header toolbar holds sidebar and menu icons as SF Symbols
  (`NativeSkin.symbol`: sidebar, menu, plus, x, settings, arrow-rotate-cw).
- A bottom dock above `keyboardLayoutGuide` holds the composer
  (`NativeControls.swift`).
- Any `role=dialog` subtree opens as a modal sheet. There are no other
  presentation styles.
- Copy is budgeted. `home/mobile-copy-checks.coax` caps the visible words at 146.

### 1.4 Visual style (verified)

The ground is **black**. Every colour is a grey. Ink is white or off-white.
"White and grayscale" in the brief means black, greys and white.

| Token (`components/skin/mono-native.coax`) | RGBA |
| --- | --- |
| GROUND | `000000` |
| SURFACE | `0a0a0a` |
| RAISED | `161616` |
| PRESSED | `262626` |
| LINE_DIM / LINE / LINE_BRIGHT | `2a2a2a` / `6b6b6b` / `ffffff` |
| INK / INK_MUTED / INK_FAINT / INK_BRIGHT | `f2f2f2` / `9a9a9a` / `5c5c5c` / `ffffff` |
| PRIMARY / PRIMARY_LIT / ON_PRIMARY | `c8c8c8` / `e0e0e0` / `000000` |
| GRID_LINE / GRID_DOT | `070707` / `181818` (40-pt tile) |

Other measures: hairline 1, hit target 44, header 44, gutter 20, gap 12. Labels
are 11 pt uppercase with 0.18 em tracking. Buttons are 44 high with 18 padding;
icon buttons are 32 square with 16-pt icons. A field is one 44-high underline.
Button variants are DEFAULT, PRIMARY, QUIET, ICON, DESTRUCTIVE, ROW and ACTION.
Button states are REST, PRESSED, DISABLED, BUSY and CURRENT. Frames have cut
corners and draw from Coax points (`NativeFrameView`, `mono_native_frame`).

Fonts:

| Use | Face | Source |
| --- | --- | --- |
| Body | DM Sans (Regular, Medium, Bold, Italic, BoldItalic) | `components/fonts` |
| Chrome labels | GeistMono (Geist also bundled) | `components/zeron/fonts`, packaged at `Starter/build/zeron` |
| Code | System monospaced | `NativeMessage.swift` |

Chat sizes (`components/skin/native-chat.coax`): body 16/23, code 14.4, h1–h6 at
21/19/17/16/15/14, user bubble at 0.85 of the width. Work rows are 28 high with
14-pt text. The composer is 44 high with 16/23 text and grows to four lines
(104 pt). `coax/native/composer.md` names a send colour of `#346bf1`, but the
current `NativeComposer` uses the grey ACTION button. That doc is stale on this
point.

`coax/native/stock-style.coax` (run by `coax/native/check.sh`) rejects stock
controls, system colours, colours written in a host file, and
`cornerRadius` or shadows outside `NativeChrome.swift`. Arcade's own look must
pass through the skin tokens, not Swift literals.

### 1.5 One screen from source to pixels: the Log in screen (verified)

1. `apps/openagents-ios/home/app.coax` keeps a `Starter` struct with `page = 0`.
   Each frame it builds a DOM-shaped tree from kit calls: a heading, an email
   field, and a "Log in" button.
2. `coax/web/wasm.sh` compiles the module to Wasm. `coax/native/compile.coax`
   then runs `wasm-opt` and `wasm2js --emscripten`, which produces
   `home.js`. `coax/native/package.coax` signs it into a CNU1 update, or the
   build bundles it.
3. At launch, `NativeHost.swift` loads the script into a JSContext with 32 MiB of
   memory (`coax/native/starter/runtime.js`). The only native function in that
   context is `cnUTF8`.
4. Each frame, Swift calls `cnFrame` and receives an array of commands
   (`NativeProgram.swift` line 32 caps a frame at 65536 commands). The ops are:
   1 create, 2 remove, 3 move, 4 text, 5 classes, 6 set attribute, 7 remove
   attribute, 8 array, 10 value, 11 focus and 12 reset. A frame may create at
   most 4096 nodes (line 58).
5. `NativeProgram.elements()` maps tags and classes to roles: `input` → field,
   `button` → button, `h1` → heading, and so on. `NativeControls.swift` turns the
   roles into `NativeField` (a UITextField with the `emailAddress` content type)
   and `NativeButton` (PRIMARY variant). They sit in a vertical `UIStackView`
   inside a `UIScrollView` (line 7). The colours come from the skin tokens above.
6. A tap sends event kind 1 (press) to the module. The module asks for service
   op 2 `auth.emailStart` (`NativeHost.swift` line 96). `AppAuth` makes the
   request in Swift, and the answer comes back as event 102 (or 202 on
   failure). The module moves to page 1, and the next frame's diff swaps the
   controls.

### 1.6 Where state lives (verified)

| State | Where | Notes |
| --- | --- | --- |
| UI state (page, drafts, lists) | Coax module memory in JSC | Lost on cold launch. There are 16 in-memory drafts (`apps/shared/app-navigation.coax`). |
| Chats | Server (`chat.account_threads_v1`) | The app keeps no local chat store (§4). |
| Tokens | Keychain | See §1.7 |
| Updates | App Support, staged by `NativeUpdates.swift` | Signed packages and their generation |
| World data | `PageHost.m` file store, `config.store/bytes/<b64ns>/<ver>/<b64key>`, mode 0600 | Worlds only. The app module cannot reach it. |
| Spark wallet data | `Application Support/spark-mainnet` | Breez SDK |

### 1.7 Keychain namespaces (verified)

All items are generic passwords, readable after first unlock, on this device
only.

| Service | Account | Holds | Source |
| --- | --- | --- | --- |
| `com.openagents.app.auth` | `tokens-v1` | JSON access and refresh tokens | `apps/shared/host/apple/AppAuth.swift` lines 13, 522 |
| `com.openagents.app.auth` | `device-key-v1` | Secure Enclave P-256 key blob that binds tokens to the device | `apps/shared/host/apple/DeviceKey.swift` |
| `com.openagents.app.spark` | `seed-v1` | 16 or 32 bytes of BIP-39 entropy. The native build only reads it; `addSeed` is compiled out by `COAX_NATIVE_BALANCE`. | `OpenAgents/Wallet/WalletKeychain.swift` line 31 |
| `com.openagents.app.nostr` | `nsec-v1` | A 32-byte Nostr secret key, made lazily and wiped from memory after use | `OpenAgents/SignIn/NostrKey.swift` |

`OpenAgents/Wallet/BreezKey.swift` holds a Breez API key certificate as a
literal in source. This doc does not quote it.

### 1.8 Account model and Nostr identity (verified)

- Login is by email code. The server returns an `oa_at_` access token (one hour)
  and an `oa_rt_` refresh token, both bound to the device key. Email-only
  accounts get an opaque ownership handle. History covers every Nostr key linked
  to the account.
- **There is a Nostr identity on the phone, but the app's screens do not use it.**
  `NostrKey.swift` stores a key, and `Platform/NostrSign.c` signs with vendored
  libsecp256k1 v0.6.0. The build flags are `ENABLE_MODULE_EXTRAKEYS` and
  `ENABLE_MODULE_SCHNORRSIG`, with **no ECDH module** (`project.yml` line 88).
  Its functions are `oa_nostr_seckey_valid`, `oa_nostr_pubkey`, `oa_nostr_sign`
  (BIP-340; verifies before returning) and `oa_nostr_verify`.
- `PhoneSignIn.swift` signs NIP-98 requests through
  `metaverse/phone-signin.coax`, compiled to C.
- `AppAuth.keyLogin` (Nostr challenge → `/api/auth/nostr`), `passkeyLogin`,
  `linkNostr`/`unlinkNostr` and `pushTarget` exist in Swift. `call()` has
  `auth.keyLogin` and `auth.passkeyLogin` cases (`NativeHost.swift` lines
  193–194). **However, the op table on line 96 has no entry for them, so Coax
  cannot reach them.** Exposing them is a one-line table change, but it changes
  the installed contract, so it needs a binary (§2.11).

## 2. Coax Native for a newcomer

### 2.1 The model

Coax Native is a native host with a script engine. The app logic is a Coax
module that runs as JavaScript in JavaScriptCore. It owns no pixels and no I/O.
Each frame it emits a tree of DOM-like nodes. The Swift host diffs that tree into
UIKit views and turns taps and typing back into events. Everything the module
does outside itself (HTTP, auth, clipboard, links, documents, updates) is a
**named service op** that Swift carries out and answers with an event. The module
cannot open a socket, read a file or touch the Keychain. Those powers exist only
where Swift lends them.

So there are two parts with different release speeds:

- **The installed contract.** This is the Swift host, the service op table, the
  tag-to-control mapping and the skin's native pieces. A runtime fingerprint
  (SHA-256 of the contract sources, `coax/native/runtime.coax`) identifies it.
  Changing it needs a TestFlight build.
- **The module.** This is the Coax code and its compiled JS. It ships by signed
  Update to every phone whose fingerprint matches.

### 2.2 Controls and layouts a screen can use (verified, `apps/shared/native/`)

| Control | Swift type | Notes |
| --- | --- | --- |
| Text / label | `UILabel` | Body, muted and label styles from the skin |
| Heading | `UILabel` | h1–h6 sizes |
| Button | `NativeButton` | Seven variants, five states, optional SF Symbol |
| Single-line field | `NativeField` (UITextField) | Keyboards: email, number pad (`tel`), decimal pad (`number`). Content types: `oneTimeCode`, `emailAddress`. |
| Multi-line field | `NativeTextArea` | Up to 8 lines, or a fixed 240 pt for code |
| Composer | `NativeComposer` (UITextView) | Plus menu (New chat, Clear draft), model menu, send/stop. The mic button is present but disabled. |
| Message | `NativeMessage` (TextKit) | Markdown: code blocks with Copy, tables, rules, block quotes, task lists, user bubble |
| Tool row | `NativeTool` | Disclosure row with detail |
| Menu | `NativeMenu` | Popover list |
| Wrapping chips | `NativeFlow` | Quiet buttons that wrap |
| Frame | `NativeFrameView` | Cut-corner CAShapeLayer path from Coax points |
| Spinner / progress | `NativeRingView` / `NativeBar` | |
| Header rule | `NativeHeaderRule` | |
| Sheet | a `role=dialog` subtree | Modal `NativeControls` sheet |
| Workspace | `NativeWorkspace` | Drawer or sidebar |
| Document pick/upload | `NativeDocuments` | UIDocumentPicker, 64 MiB, PUT only to `https://storage.googleapis.com/` |
| World | `NativeWorldHost` (MTKView) | Compiled-in world frameworks only |

Layout is one vertical stack per surface: header, top dock, scrolling body, and a
bottom dock above the keyboard. There is no grid, horizontal scroller, tab bar,
split view inside a screen, or absolute positioning. `NativeFlow` is the only
wrapping layout.

**Not present:** image views (no `img` mapping in `NativeProgram.elements()`),
avatars, haptics, swipe actions, long-press menus, pull to refresh, bottom
sheets with detents, UICollectionView/UITableView, toggles, QR scanning, audio
recording and video.

### 2.3 Long lists and scrolling (verified)

`NativeControls` realizes every node. It has no recycling, but it does support
follow-to-end scrolling for chat. The hard limits are 4096 nodes per tree and the
module's 1 MiB event inbox (`home/face.c` `DOM_INBOX_CAP`). The app works within
them with paging: history shows at most 60 chats, and the drawer keeps
`NAV_THREADS` 512 but shows `NAV_VISIBLE` 96 with a page size of 32
(`apps/shared/app-navigation.coax`). `coax/native.md` lists a "keyed list" in the
first control set, but no virtualized native list exists yet (inferred from the
absence of any collection view in `apps/shared/native/`).

### 2.4 Text input and keyboard (verified)

There are fields, text areas and the composer. A tap outside the field dismisses
the keyboard. The bottom dock follows `keyboardLayoutGuide`. Events are edit (2)
and submit (3). Focus is op 11. There is no input accessory bar and no
autocomplete beyond the system's.

### 2.5 Images and media

There are none in the app module. SF Symbols are the only images, and only
through `NativeSkin.symbol`. Fonts are bundled. `components/core/media.coax`
exists in the kit, but it was not checked whether it projects natively (open
question). World producers draw with Metal and load streamed assets (bucket
`openagentsgemini-native-world-assets`, `coax/native/worlds/assets-wire.coax`:
2048 files, 64 MiB each, 192 MiB in total).

### 2.6 Navigation, gestures, haptics, animation (verified)

- Navigation is module state: `page` in `home/app.coax`, the drawer and modal
  sheets. There is no UINavigationController stack and no swipe-back gesture.
- Gestures: taps, the drawer's edge and left pans, and scrolling.
- Haptics: none.
- Animation: the drawer slide, the spinner, and the frames' "march" dash
  (0.6 s) and turn (1.8 s). All of them honour Reduce Motion.

### 2.7 Custom drawing

- **Metal:** only inside compiled-in world frameworks, through
  `metaverse-engine/host/apple/PageHost.m` and `NativeWorldHost`. A new
  producer is a new framework, which needs a binary.
- **Core Graphics / CAShapeLayer:** `NativeFrameView` draws paths that Coax
  supplies. That is the nearest thing to a canvas for app screens.
- **Canvas:** there is no general canvas element in the app projection.

### 2.8 Local storage and databases

The app module has none. It cannot persist anything, even a draft, across a
cold launch. `PageHost.m` already has a namespaced file store
(`coaxial_storage_get/put` from line 1380: namespace 1–64 bytes, key 1–128
bytes, value up to 1 MiB, files at mode 0600), but only worlds can reach it.
There is no SQLite in the target.

### 2.9 Networking

| Kind | App module | Worlds |
| --- | --- | --- |
| HTTP | Op 4, through `AppAuth` with the bearer token added natively. GET or POST only. Path must pass `cn_api` or `nwp_account` (`coax/native/updates-policy.coax` line 29, `coax/native/path-policy.coax`). Body ≤ 64 KiB, response ≤ 1 MiB, 60 s timeout, same-origin redirects. One request at a time. | Same-site HTTP |
| WebSocket | **None** | 4 slots, `ws`/`wss` to any host, Origin header set, messages up to 8 MiB (`PageHost.m` line 1322) |
| Streaming responses | None. Chat replies arrive whole, and computers are polled. | n/a |

### 2.10 Crypto (verified)

| Primitive | Native today | Reachable from the module |
| --- | --- | --- |
| secp256k1 keygen and x-only pubkey | `oa_nostr_pubkey` (C) | No |
| BIP-340 Schnorr sign and verify | `oa_nostr_sign`, `oa_nostr_verify` | No |
| ECDH (for NIP-44) | **Not compiled** (no `ENABLE_MODULE_ECDH`) | No |
| ChaCha20, HMAC-SHA256, HKDF | CryptoKit has HKDF and HMAC; ChaCha20 is in Coax (`coax/lib/crypto`) | Pure Coax code runs in JSC |
| Ed25519 verify | CryptoKit (updates) | No |
| Secure random | SecRandom in Swift | **Not exposed.** JSC has no `crypto.getRandomValues`. |

Pure-Coax secp256k1 exists, but the Nostr signing tables are built at compile
time and need `--run-steps 250000000`. In JSC that code would also hold the
secret key inside script memory, which `coax/nostr.md` §5 rules out for custody.

### 2.11 Push notifications and background tasks

Neither exists. The entitlement is `aps-environment development`, and nothing
calls `registerForRemoteNotifications`. There are no `BGTaskScheduler`
identifiers in `Info.plist`. `AppAuth.pushTarget` exists but nothing calls it.

### 2.12 Update versus TestFlight (verified, `coax/native-updates.md`, `coax/native/updates-policy.coax`)

| Change | Path |
| --- | --- |
| Coax module code, screens, copy, layout from existing controls, skin token values used by the module | **Update** (signed CNU1 package) |
| New service op, a change to the op table, a new HTTP path allowlist, a new tag or control in Swift, a new native skin piece, new entitlements, new SDKs or frameworks, new Info.plist keys, a new world producer | **New TestFlight build** (it changes the runtime fingerprint) |

How an Update works: an Ed25519 signature (CryptoKit) over
`"Coax Native Package v1\0"` plus the manifest. The envelope is "CNU1", a 4-byte
length, the manifest (≤ 16384 bytes) and a 64-byte signature. A package holds at
most 24 files, 8 MiB each and 32 MiB in total. Phones fetch
`https://updates.openagents.com/runtimes/<fingerprint>` (Cloud Run `oa-updates`,
bucket `openagentsgemini-native-updates`). An update activates on cold launch or
when the user taps "Apply update". A trial becomes healthy after it reports ready
and then stays 30 s in the foreground; if it fails, the phone rolls back. App
Store rules 2.5.2, 2.5.19 and 4.7 limit an Update to interpreted code that does
not change the app's purpose.

### 2.13 Size and performance limits (verified unless marked)

| Limit | Value |
| --- | --- |
| JSC memory | 32 MiB |
| Script size | 8 MiB |
| Nodes per tree / commands per frame | 4096 / 65536 |
| Module inbox | 1 MiB |
| HTTP body / response | 64 KiB / 1 MiB |
| Update package | 24 files, 32 MiB |
| Cold-frame goal | 100 ms p50 (`coax/native.md`). This is a goal, not a measured result. |

### 2.14 How screens are tested (verified)

- `coax/native/tests/runtime-oracle.cjs` runs the wasm2js output against the
  real Wasm build through login, chat and environment flows and compares the
  frames.
- `coax/native/check.sh` runs the style check (`stock-style.coax`) and the
  policy asserts (`#assert` in `updates-policy.coax`, `text-link-policy.coax`
  and others).
- `home/mobile-copy-checks.coax` and `worlds/world-copy-checks.coax` check copy.
- XCTest targets `OpenAgentsTests` (StarterTests) and `OpenAgentsUITests`
  (StarterUITests). The CoaxNativeLook scheme runs with `-OAAutomation YES`
  against a local API at `http://127.0.0.1:13591`. Other schemes (CoaxNativeChat,
  CoaxNativeProjects and so on) drive single flows.
- Releases record verification JSON in `docs/verification/2026-11-1x-*.json`.
  `apps/openagents-ios/release.sh` uploads through altool.

## 3. Can coax/lib/nostr run in the app?

**Short answer: the pure logic can, but nothing that makes Nostr useful can.**
Today the app links only `coax/lib/nostr/nip51.coax` (for `nl_contains_folded`
search) with its event, tags and JSON files, plus `coax/lib/nostr/json.coax`.
No relay, signer, NIP-44 or store code runs (verified by the imports of
`home/app.coax` and `apps/shared/app-navigation.coax`).

### 3.1 What the library needs from a host

| Library piece | What it is | Host need |
| --- | --- | --- |
| `relay.coax` | Pure client state machine: 8 subscriptions, 8 publishes, 64 seen IDs, backoff, NIP-42 hook | A socket that moves bytes (`coax/lib/net/ws.coax` frames over `coax/platform/socket-native.c` or the browser socket) |
| `keys.coax` | Signer-request contract (local test slot, NIP-07, NIP-46 later), with slot states ASKED/APPROVED/SIGNED/DENIED/CANCELLED/EXPIRED | A signer effect that the host fulfils |
| NIP-44 v2 (`nip44.coax`, `host/browser.coax`) | ChaCha20 + HMAC + HKDF over an ECDH shared secret | ECDH, random bytes |
| `nip17`, `nip59` | Gift-wrapped DMs | NIP-44, signing, random bytes, clock |
| `store.coax` | The **relay's** in-memory verified store, which forgets on restart | Not a client database. `coax/nostr.md` §8 says the durable store is not built. |
| `vault.coax` | NIP-VAULT key slots (HKDF + AES-256-GCM) for Nostr, device, passkey-PRF and recovery methods | AES-GCM, random bytes, storage |

### 3.2 Web versus native, piece by piece

| Host piece | Web (`coax/web/host.js`) | Native equivalent | Exists? |
| --- | --- | --- | --- |
| WebSocket | `coaxial_socket_*` imports, at most 4, text and binary. Real use: `apps/openagents-web/machine-web-face.c` opens the relay with `coaxial_socket_open(cw_socket_url(...))`. | `PageHost.m` `coaxial_socket_open` (line 1322) | **For worlds only.** Needs NativeHost service ops (`socket.open/send/close`) plus inbound events. |
| Persistent storage | `coaxial_storage_get/put` over IndexedDB or localStorage (`coax/web/STORAGE.md`) | `PageHost.m` `phStorage` (line 1380) | **For worlds only.** Needs NativeHost ops, or a SQLite-backed event store. |
| Signing | `coaxial_nip07_sign` (browser extension) | `NostrKey.swift` + `oa_nostr_sign` | **Code yes, op no.** Needs `nostr.pubkey` and `nostr.sign(event-id)` ops. The key never enters JSC. |
| ECDH / NIP-44 | Module code with keys the caller brings (`host/browser.coax`) | — | **Missing.** Either add the libsecp ECDH module and a `nostr.nip44Encrypt/Decrypt` op, or a `nostr.sharedSecret(pubkey)` op. The first keeps the conversation key out of JSC too. |
| Random bytes | `crypto.getRandomValues` (`coax/platform/entropy-wasm.c`) | SecRandom | **Missing in JSC.** Needs a `random(n)` op, or native-side NIP-44 that makes its own nonce. |
| Clock | `coaxial_now_seconds` | `Date()` in Swift | Partly. The module gets time through events. Confirm whether a wall-clock call is exposed. |
| Push | Web push not used | APNs | **Missing** (entitlement is development; no registration) |
| Background | Tab must be open | BGTaskScheduler / push | **Missing** |

The working web examples (`apps/openagents-web/nostr-chat.coax`, the
`nostr-lab-*` lessons) run against an in-process relay with test vector keys and
no network. They show the protocol but not a live client. `machine-web.coax`
is the one real web client that uses socket and storage.

### 3.3 The smallest native Nostr module

All of these are binary changes (one TestFlight build if they land together):

1. NativeHost service ops `socket.open`, `socket.send`, `socket.close`, with
   inbound frames delivered as events. Lift the code from `PageHost.m`, keep
   its 4-slot and 8 MiB caps, and add a relay allowlist decision (§7).
2. Ops `store.get`, `store.put` and `store.list` over the `phStorage` file
   layout, or a small SQLite event table keyed by id, kind, pubkey and
   `created_at`. The file store is enough for a first version with a capped
   history. A searchable DM history needs SQLite.
3. Ops `nostr.pubkey` and `nostr.sign(id32)` over `NostrKey` and `NostrSign.c`.
   Add `nostr.encrypt44(peer, plaintext)` and `nostr.decrypt44(peer, payload)`
   after turning on `ENABLE_MODULE_ECDH` in `project.yml` line 88. The NIP-44
   cipher logic stays in Coax compiled to C; only the C side sees keys.
4. `random(n)` for gift-wrap ephemeral keys, or generate those natively too.
5. Wire `auth.keyLogin` and `nostr.link` into the op table so the phone key
   joins the account (§1.8).
6. Later: APNs registration (`aps-environment production`, `pushTarget`), and a
   notification service extension that can decrypt a wrapped DM preview.

After that build, relay logic, NIP-17/59 flows, channel lists and every screen
ship by Update.

## 4. Other mobile surfaces and chat storage

### 4.1 Other surfaces (brief)

| Surface | Status | Source |
| --- | --- | --- |
| OpenAgents mobile (`apps/openagents-mobile/`) | The workspace's named mobile operator path; Rust/mobile crate. Not inspected in depth here. | workspace `AGENTS.md` |
| Android | An Android C host for the same shared Coax modules exists (`apps/shared/host/android/`). Not inspected further. | `apps/shared/host/android/*` |
| Everglade mobile (`apps/everglade-mobile`) | Not read | — |
| Shared app (`apps/shared/openagents-app.coax`) | The book-milestone shared app (#13516). It runs in the iOS binary with `-OASharedApp` (`StarterApp.swift`). | `StarterApp.swift` |
| Web (`apps/openagents-web/`) | `/chat` is the reference UI. Its kit and mono skin define the look (`components/kit`). | — |

### 4.2 How chat is stored and synced (verified)

- Wire: `proto/coder/account-chat-v1.json`, schema `openagents.chat.thread.v1`,
  codec `coax/web/api/account-chat-wire.coax`.
- Store: Postgres table `chat.account_threads_v1` on the server. There is **no
  local copy on the phone**.
- Write: `POST /api/chat/thread/v1` with compare-and-swap on `revision`.
- Read: `GET /api/chat/thread/v1?id=` and `GET /api/chat/history/v1` (at most 60
  chats, `chat-history-wire.coax`).
- Reply: `POST /api/chat/complete/v1` (team beta, metered Vertex). The reply
  arrives in one piece; nothing streams.
- Limits: 32 messages, 48,000 bytes per thread, 16,384 bytes per message,
  65,536 encoded bytes, 4,096-byte draft. IDs are `m_` plus 20 characters.
  Older chats open on the web.
- Related routes: `/api/chat/project/v1`, `/api/chat/project/complete/v1`,
  `/api/chat/world/complete/v1`.
- Sync is "fetch on open, CAS on write". There is no realtime push.

Arcade's chats are Nostr events on relays: public channels (NIP-28) and DMs
(NIP-04 in the old app). They have no place in this account-thread store. A port
needs a separate local event store (§3.3), not new fields in
`openagents.chat.thread.v1`.

## 5. Requirement fit

Legend: **Today** = usable now. **Small** = a small extension (named). **Build** =
missing; what to build and where. **TF** = needs a new TestFlight build. **U** =
deliverable by Update.

| Arcade need | Fit | What and where | Ship |
| --- | --- | --- | --- |
| Screens built from text, buttons, fields, headings | Today | `components/kit` + `apps/shared/native` | U |
| Black/grey skin, DM Sans, GeistMono | Today | `components/skin/mono-native.coax` | U |
| Drawer navigation (channels, DMs, contacts, relays) | Today | `NativeWorkspace`, `apps/shared/app-navigation.coax` | U |
| Bottom tab bar like Arcade's `TabNavigator` | Build | A native tab control in `apps/shared/native` + kit. Recommend reusing the drawer instead. | TF |
| Message bubbles with markdown | Today | `NativeMessage` | U |
| Composer with send | Today | `NativeComposer` | U |
| Long message history | Small | Paging in Coax within the 4096-node cap (U). A real virtualized keyed list (UICollectionView) in `apps/shared/native` is TF. | U / TF |
| Avatars and images in messages | Build | An `img` role mapped to a UIImageView with a cached HTTPS fetch, in `NativeProgram`/`NativeControls` | TF |
| Modal sheets (create channel, add contact) | Today | `role=dialog` | U |
| Long-press menu, swipe actions, haptics | Build | `apps/shared/native` gestures + `UIFeedbackGenerator` service op | TF |
| QR scan / show (invite, npub) | Build | Camera scanner op (usage string exists) and a QR image node | TF |
| Relay WebSockets | Build | `socket.*` ops in `NativeHost.swift`, lifted from `PageHost.m` | TF |
| Relay logic (subs, backoff, NIP-42) | Today | `coax/lib/nostr/relay.coax` (pure Coax) once sockets exist | U |
| Nostr key on the device | Today | `NostrKey.swift`, Keychain `com.openagents.app.nostr`/`nsec-v1` | — |
| Sign events with the Keychain key | Small | `nostr.pubkey`, `nostr.sign` ops over `NostrSign.c` | TF |
| NIP-44 encryption | Build | Turn on libsecp ECDH (`project.yml` line 88) + `nostr.encrypt44/decrypt44` ops; cipher from `coax/lib/nostr/nip44.coax` compiled to C | TF |
| NIP-17/59 gift-wrapped DMs | Small | `coax/lib/nostr/nip17.coax`, `nip59.coax` in the module, after the ops above | U |
| NIP-28 public channels | Build | Not present in `coax/lib/nostr`. Add `nip28.coax` there. | U |
| NIP-65 relay lists, profiles (kind 0), contacts (kind 3) | Small | `coax/lib/nostr/nip65.coax` and event helpers | U |
| Local event store / offline history | Build | `store.*` ops over the `phStorage` layout or SQLite in `apps/shared/native` | TF |
| Import or back up an nsec | Build | A screen in Coax (U) + `nostr.import`/`nostr.export` ops with a Face ID gate (TF). Consider `vault.coax` for backup. | TF |
| Log in with the Nostr key / link it to the account | Small | Add `auth.keyLogin` and `nostr.link` to the op table (`NativeHost.swift` line 96) | TF |
| Push notifications for new DMs | Build | APNs production entitlement, registration, `pushTarget`, a relay-watching push service on the server, a notification service extension | TF |
| Background relay sync | Build | BGTaskScheduler or push-driven fetch | TF |
| AI chat (Arcade's AI screens) | Today | `/api/chat/complete/v1` and the Chat screen | U |
| New HTTP routes (for example a push registration route) | Small | Extend `cn_api`/`nwp_account` | TF |
| Listings / marketplace (NIP-99 style) | Build | Screens in Coax (U). Payments are Bitcoin and Lightning only, and spending is not exposed (wallet is read-only). | U / TF |
| Zaps / Lightning payments | Build | Wallet send is compiled out of the native build (`COAX_NATIVE_BALANCE`). Needs a send op and a review of spend authority. | TF |
| Contacts search | Today | `nl_contains_folded` from `nip51.coax` | U |
| Light mode | Missing by policy | The app is dark only | — |

## 6. Binding constraints

| Constraint | Source |
| --- | --- |
| Do not use stock controls, system colours, host-file colours, or corner radius or shadows outside `NativeChrome.swift` | `coax/native/stock-style.coax`, `coax/native/check.sh` |
| An Update may change only interpreted module code within the app's purpose; new native APIs, SDKs and entitlements need a binary | `coax/native-updates.md` (App Store 2.5.2, 2.5.19, 4.7; DPLA 3.3.1 B–C) |
| HTTP only to allowlisted paths, GET/POST, 64 KiB / 1 MiB, one at a time | `coax/native/updates-policy.coax` (`cn_api`), `coax/native/path-policy.coax`, `NativeHost.swift` |
| Links open only for HTTP(S), at most 4096 bytes | `coax/native/text-link-policy.coax` (`nt_link`) |
| 4096 nodes, 65536 commands, 32 MiB JSC, 8 MiB script | `NativeProgram.swift` lines 32, 58; `coax/native/starter/runtime.js` |
| Secret keys stay in native custody; the script gets effects, not keys | `coax/nostr.md` §5 |
| A durable relay/event store is required but not built | `coax/nostr.md` §8 |
| Visible copy on home is capped (146 words) | `home/mobile-copy-checks.coax` |
| UI matches the `/chat` components on the mono skin; missing controls go into the kit | Workspace memory "UI matches /chat components"; `docs/reuse.md` (not read) |
| Never hand-roll: extend `coax/lib/nostr` and `components/kit`; do not add a parallel Nostr stack | Workspace `AGENTS.md` (nostr-effect / shared modules), memory "Never hand-roll" |
| Auth is email code plus Nostr; no phone numbers or SMS; say "Log in" | Brief; memory "No phone numbers", "Log in, not sign in" |
| Payments are Bitcoin and Lightning only | Brief; memory "Bitcoin-only rails" |
| No routing by keyword matching for intent or retrieval | Workspace `AGENTS.md` "Semantic Routing And Retrieval" |
| Nostr work is owned by another agent since 2026-11-12 | Memory "Nostr owned elsewhere" — coordinate before building §3.3 |

## 7. Risks and recommendations

| # | Risk | Recommendation |
| --- | --- | --- |
| 1 | Every Nostr capability is a host change. Shipping them one at a time means many TestFlight builds and fingerprint changes. | Design the full op set in §3.3 (sockets, store, sign, NIP-44, random, keyLogin) and land it in **one** binary. Then ship all Arcade screens by Update. |
| 2 | Signing or NIP-44 in JSC would put the nsec in script memory. | Keep keys in `NostrKey`/C. Expose only `sign(id)` and `encrypt44/decrypt44`. |
| 3 | The not-virtualized stack and 4096-node cap break busy channels. | Start with Coax paging (a window of about 100 messages). Plan a keyed virtual list in `apps/shared/native` + `components/kit` for the same binary if channels are in scope. |
| 4 | No local storage: the app forgets everything on cold launch. | Reuse `phStorage` (already reviewed and 0600) for the first version; move to SQLite only if search across history is required. |
| 5 | Opening sockets to any relay leaks the user's IP and pubkey interest to third parties. | Default to OpenAgents relays (the `immortal` relay). Let the user add relays explicitly. Enforce `wss` only. |
| 6 | No push means DMs arrive only while the app is open. | State that limit in the first version. Push needs a server-side relay watcher and a production APNs entitlement; scope it as its own issue. |
| 7 | Arcade's tab bar and image-heavy screens do not fit the mono, drawer-based design. | Port the flows, not the layout: drawer for sections, text-first rows, avatars only after the image node exists. |
| 8 | Two Nostr identities: the account's linked keys and the phone's `nsec-v1`. | Link the phone key to the account at first use (`linkNostr`) and show one npub. Decide whether imported nsecs replace or join it. |
| 9 | Another thread is changing the iOS app daily. | Rebase on current `main` before each change. Land host ops in `apps/shared/native` in small commits and coordinate the runtime fingerprint change with that thread's release. |
| 10 | Nostr is owned by another agent. | Hand §3.3 to the Nostr owner or agree a split before building. |
| 11 | NIP-28 is missing from `coax/lib/nostr`. Arcade's channels depend on it. | Add `nip28.coax` in `coax/lib/nostr`, or decide that channels map to NIP-29 groups. |
| 12 | `coax/native/composer.md` names a blue send colour that the code does not use. | Treat the code and skin as the truth; fix that doc line in a later change. |

## Files read

Worktree root `/Users/christopherdavid/work/oa-arcade-target` unless noted.

- `apps/openagents-ios/README.md`, `project.yml`, `StarterApp.swift`,
  `generate.sh`, `build.sh`, `release.sh`, `Info.plist`, entitlements
- `apps/openagents-ios/home/README.md`, `app.coax`, `shell.coax`,
  `navigation.coax`, `projects.coax`, `usage.coax` (part), `host.js`, `face.c`,
  `mobile-copy-checks.coax`
- `apps/openagents-ios/extras/app.coax`, `apps/openagents-ios/worlds/app.coax`,
  `world-binding.coax`
- `apps/openagents-ios/OpenAgents/SignIn/NostrKey.swift`,
  `SignIn/PhoneSignIn.swift`, `Platform/NostrSign.c` and `.h`,
  `Wallet/WalletKeychain.swift`, `Wallet/BreezKey.swift` (secret not quoted),
  `NativeBridge.h`
- `apps/shared/native/NativeHost.swift`, `NativeProgram.swift`,
  `NativeTypes.swift`, `NativeControls.swift`, `NativeWorkspace.swift`,
  `NativeDocuments.swift`, `NativeChrome.swift`, `NativeComposer.swift`,
  `NativeTool.swift`, `NativeMessage.swift`, `NativeUpdates.swift` (start)
- `apps/shared/host/apple/AppAuth.swift` (key parts), `DeviceKey.swift` (parts),
  `PhoneTurn.swift`
- `apps/shared/openagents-app.coax`, `environment-app.coax`,
  `app-navigation.coax`, `provider-accounts.coax`, `projects-app.coax`;
  listing of `apps/shared/host/android/`
- `coax/native.md`, `coax/native-updates.md`, `coax/native/README.md`,
  `composer.md`, `check.sh`, `generate.sh`, `compile.coax`, `package.coax`,
  `prepare.coax`, `publish.coax`, `runtime.coax`, `profile.coax`,
  `text-host*`, `text-services-wire.coax`, `text-link-policy.coax`,
  `path-policy.coax`, `documents*.coax`, `workspace.coax`, `stock-style.coax`,
  `updates-policy.coax`, `updates-wire.coax`, `sign.swift`, `cloud-run.yaml`,
  `starter/*`, `worlds/*`, `tests/runtime-oracle.cjs`
- `components/skin/mono-native.coax`, `components/skin/native-chat.coax`
- `metaverse-engine/host/apple/PageHost.m` (sockets, storage)
- `coax/lib/nostr/relay.coax`, `keys.coax`, `store.coax`, `vault.coax`,
  `host/browser.coax`, `ws/change.coax` (heads); directory listing of
  `coax/lib/nostr/`
- `coax/nostr.md` §5, §8, §12
- `coax/web/host.js` (socket, storage and NIP-07 imports)
- `apps/openagents-web/nostr-chat.coax` (head), `nostr-chat-face.c`,
  `nostr-lab-relays.coax` (head), `machine-web.coax` (head),
  `machine-web-face.c` (lines 1–26, 76–112)
- `proto/coder/account-chat-v1.json`, `coax/web/api/account-chat-wire.coax`,
  `chat-history-wire.coax` (route facts)
- `/Users/christopherdavid/work/arcade/README.md`, `package.json`,
  `app/screens/` and `app/navigators/` (names only)

Not read: `docs/reuse.md`, `components/spec.md`, the book chapters,
`components/core/media.coax`, `components/kit/listing.coax`,
`NativeWorld*.swift`, `apps/openagents-mobile/`, `apps/everglade-mobile/`,
`NEEDS_OWNER.md` iOS entries.

## Open questions

1. Who builds the native Nostr ops in §3.3: the Nostr owner (since 2026-11-12)
   or the Arcade port? Recommendation: the Nostr owner builds the ops; the port
   builds screens.
2. Should the op set land in the next iOS build that the other thread is
   preparing, or in a dedicated build? Recommendation: one dedicated build with
   the full set.
3. Public channels: NIP-28 (Arcade's original) or NIP-29 groups on the
   `immortal` relay? `coax/lib/nostr` has neither NIP-28 nor an obvious group
   module.
4. Which relays by default, and may users add any `wss` relay?
5. Is the phone's `nsec-v1` the user's Nostr identity, or should users import
   their existing nsec (as Arcade allowed)? How does it link to the email
   account?
6. Is push in scope for the first version? It needs a server-side relay watcher
   and a production APNs entitlement.
7. Does `components/core/media.coax` already project to native images, or must
   an `img` role be added to `NativeProgram`?
8. Is a virtualized list needed for the first version, or is paging in Coax
   enough?
9. Do Arcade's listings and AI screens come over at all, or only chat (DMs and
   channels)?
10. Can the module read the wall clock directly, or only through events?
    (`created_at` needs it.)

## 2026-11-14: additional rendering requirements

The initial target inspection does not qualify a GPU component UI. At
`96825b529e` the shared GPU backend exists, but `gpu_draws` skips arcs and
images; it has no complete arbitrary transform/effect compositor. The shared
ARWES motion code provides easing and timelines, not the complete Arcade
spring/gesture catalog. The Android Vulkan carrier exists, but is not proof
of IME, AutoFill, TalkBack or an entire mobile app.

The native component runtime must add shared Coax motion/paint, GPU image and
blur resources, a presentation clock independent of app-data work, native
text/composition and semantic bridges. Compatibility UIKit views can stay
while those paths are qualified. New component state/layout/paint remains
in `components/`; the host only provides platform effects and bindings.

For several inline worlds, `PageHost.m:130` has `phCurrent` explicitly binding
one open page. Audit every static page state and asynchronous callback,
introduce explicit per-view context/generation through a versioned adapter,
and compose admitted scene outputs with one aggregate budget. Full-screen
world success does not qualify three independent embedded worlds.

The one-build objective must include these installed capabilities as well as
the Nostr ops. A new compositor, effect pipeline or host/input/semantic ABI
needs a binary/fingerprint change; later compatible component/motion code can
ship by Update. No default changes before the gallery, actual iOS/Android
editing/accessibility, web-island and multi-scene performance/recovery gates
in [the rendering plan](../../components/rendering.md#delivery-order-and-acceptance).
