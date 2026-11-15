# Arcade port

**Execution plan, 2026-11-15:** [part 08](08-delivery.md) is the current
65-feature / A01–A28 delivery map for [#13624](https://github.com/OpenAgentsInc/openagents/issues/13624).
The owner selected one issue at a time, pushing each completed change. A live
NIP-01 kind-1 feed ships **before** the messaging foundation; the first
TestFlight upload follows that feed, then another after each major feature.
Continue work while Apple processes uploads. Part 08 supersedes older
phone-first identity, refresh/composer-control and release-order assumptions.

**Account/settings checkpoint, 1088:** the native Account hub now includes profile
editing, relay management, privacy, notification permission/security push, public
keys, additional login keys/passkeys, protected backups, device sessions and
resources. The drawer follows Arcade’s 70-point sliding icon rail, while Recent
chats keeps the wider history pane. See [the settings mapping](07-account-settings.md).

**Mobile checkpoint, 1087:** current app chrome now uses the shared GPU
Arcade component scene with a white/grayscale skin and several shared
animations. Native text/input/accessibility adapters remain. See
[the mapping](03-design-and-components.md#1087-current-mobile-component-mapping)
and [the release evidence](../verification/2026-11-14-arcade-mobile/README.md).
This does not complete the full A01–A28 parity catalog.

Arcade (`OpenAgentsInc/arcade`, formerly `ArcadeLabsInc/arcade`) is the Nostr
and AI chat app OpenAgents built in 2023 on React Native 0.71 and Expo 48. It
put public channels, encrypted direct messages, small encrypted groups and an
AI chat in one inbox, around a Nostr key, contacts and relays, with a black
and cyan "HUD" look. These docs audit it and plan its move into OpenAgents
iOS on Coax Native and `coax/lib/nostr`, with current NIPs and in white and
grayscale (issue #13607).

## Where things are

- Reference clone, read-only: `~/work/arcade` at `3f10e1e` (2023-08-05).
  Its Nostr library is the submodule `app/arclib` at `542ecec`. Never edit,
  commit or push there.
- Canonical NIP texts, read-only: `~/work/projects/repos/nips` (the audits
  checked `1fb3a95`). The repository's pinned mirror is `nostr/nips/official/`.
- The target: `apps/openagents-ios/`, `apps/shared/native/`, `coax/native/`,
  `coax/lib/nostr/`, `components/`.

## The docs

| Doc | What it covers |
| --- | --- |
| [01-codebase.md](01-codebase.md) | Arcade as a product and a codebase: every screen, route, flow, store, service, dependency and test, and the 65-row feature inventory the port is built from |
| [02-nostr.md](02-nostr.md) | Every Nostr use in Arcade (kinds, tags, ciphers, relays, keys), then the current NIP and `coax/lib/nostr` module for each, and the library's gap list |
| [03-design-and-components.md](03-design-and-components.md) | Arcade's tokens, type, motion, every component and screen layout, then the grayscale mapping onto the mono skin and the kit |
| [04-target-app.md](04-target-app.md) | OpenAgents iOS and Coax Native at `450ca4738f`: screens, controls, state, networking, crypto, the Update versus TestFlight line, and what the port needs from the host |
| [05-port-plan.md](05-port-plan.md) | The plan: scope and a disposition for every inventory row, the Nostr design, the one native build, the design system, phases with issues to file, ownership, owner decisions, risks |
| [06-rendering-and-animation.md](06-rendering-and-animation.md) | Complete animation catalog (A01–A28), CPU/UI-thread versus GPU assessment, shared native GPU visuals with platform services, DOM GPU islands and several inline worlds |
| [07-account-settings.md](07-account-settings.md) | Shipped 1088 account/settings mapping and 1090 workspace corrections |
| [08-delivery.md](08-delivery.md) | Current issue order, release gates, all 65 feature dispositions, all 28 motion cases and acceptance requirements |

**2026-11-14 rendering update:** the owner requires every Arcade animation to
be portable, including dormant/demo effects. Part 06 and
[components/rendering.md](../../components/rendering.md) supersede earlier
recommendations to discard glow, blur, typing or custom spinner behavior.
All need gallery specimens; which ones appear in the app is a separate product
choice. The recommended target is shared GPU visuals on iOS/Android with native
input/accessibility services, and DOM text with GPU islands on web. This is a
plan, not an implemented switch. Phase 0 now includes that component foundation.

Headline findings:

**01, the codebase**

- Arcade shipped v0.1.0 to v0.3.2 between 2023-06-12 and 2023-07-25: one
  inbox of NIP-28 channels, NIP-04 and "blinded" DMs, "NIP-112" groups and an
  AI chat ("Faerie") on `api.arcade.chat`.
- Received events are never signature-checked (`skipVerification: true`,
  `RelayProvider.tsx:40`).
- The private key is stored twice: in SecureStore and inside the encrypted
  MobX snapshot `root-v1`; Log out leaves channel keys and the SQLite cache
  for the next account.
- AI chats are unauthenticated: anyone with an npub can read that user's
  conversations.
- Signup fails if the `@arcade.chat` NIP-05 Lambda is down; the marketplace
  exists in code but is unreachable and broken.

**02, Nostr**

- "NIP-112" is nostr-protocol/nips PR #580, never merged; Arcade's code is an
  earlier prototype that does not match it (kind 99 wraps, AES-CBC without a
  MAC, unsigned senders, anyone can write).
- "Blinded" DMs and the `??` cipher are Arcade's own; NIP-04 is
  `unrecommended`. Every DM mode becomes NIP-17, which `nip17.coax` already
  sends and opens for up to 10 recipients.
- Groups: NIP-17 rooms up to 10, NIP-29 on relay.openagents.com for larger
  and public groups, Marmot only if end-to-end encryption above 10 is needed.
  NIP-28 is `unrecommended` and `retired.coax` refuses to sign it.
- Arcade's kinds 10003 and 30040 now mean bookmarks and publication indexes.
- The phone cannot do NIP-44 yet: libsecp256k1 is built without ECDH
  (`project.yml` line 88).

**03, design and components**

- The shipped look is "black glass with cyan hairlines"; the Arwes frames,
  glow and HUD tab bar were tried and mostly backed out.
- Every Arcade token maps to a mono gray: cyan washes to SURFACE and RAISED,
  hairlines to LINE_DIM, accents to INK_MUTED and INK.
- Part 03 originally kept frame/swipe motion and dropped glow/typing. Part 06
  supersedes that scope: preserve every animation capability, including the
  custom popup blur and animated image, in reusable specimens. Production uses
  grayscale tokens, DM Sans and GeistMono; no separate Faerie product is added.
- Native needs images, avatars, rows, switches, long press, swipe and a
  confirm; the drawn frame view already exists (see 05 §0, C2).
- Its list of non-gray colours in the native app predates commit
  `569f03794e`, which already fixed them (05 §0, C1).

**04, the target app**

- OpenAgents iOS 1.0.0 (1085) is a UIKit host running Coax in JavaScriptCore;
  screens ship by signed Update, host changes need a TestFlight build.
- The app module has no WebSocket, no local storage and no secure random;
  worlds have sockets and a file store in `PageHost.m` that can be lifted.
- A Nostr key exists (`NostrKey.swift`, Keychain `com.openagents.app.nostr`
  / `nsec-v1`) and signs NIP-98, but no screen uses it, and `auth.keyLogin`
  is not in the op table.
- One TestFlight build with sockets, store, sign, NIP-44, random and key
  login lets every later screen ship by Update.
- `apps/openagents-ios/OpenAgents/Wallet/BreezKey.swift` holds a Breez API key
  certificate in source (05 §9).

**05, the plan**

- Phase 0 is one TestFlight build: 12 native operations (N1 to N12) and 11
  controls (K1 to K11).
- Phases 1 to 5 ship by Update: identity and relays, contacts and DMs,
  private groups, public groups (plus a relay change), images and files.
- 16 Nostr work items (G1 to G16, one of them on the relay) belong to the Nostr owner.
- 18 owner decisions, the eight that block Phase 0 first.

**06, rendering and animation**

- Arcade mixes Reanimated UI-thread work, Skia drawing, JS timers, native
  transitions and a GIF. “Animated” does not mean its state runs in a shader.
- Reuse `components/` rather than importing GPUI/Skia or writing two widget
  sets. CPU Coax evaluates interaction/layout/motion; GPU draws and composites.
- Existing GPU primitives skip arcs/images. Springs, transform groups,
  image/background blur and scene composition need implementation.
- UIKit/Android still supply IME, AutoFill, accessibility and system services.
  DOM text stays DOM and can contain GPU islands.
- Three inline worlds require per-view state/callbacks and aggregate GPU
  budgeting; today's Apple `phCurrent` binds one page at a time.

## Decisions waiting on the owner

This is the historical decision inventory from the initial plan. Use
[part 08](08-delivery.md) for current execution: managed account identity,
drawer navigation, grayscale components, simplified composer and automatic
sync already have owner decisions and must not be requested again.

The full list, with options, recommendations, reasons and what each blocks,
is [05-port-plan.md §7](05-port-plan.md#7-decisions-for-the-owner). The ones
that block Phase 0:

1. D1: who builds the native Nostr operations.
2. D2: a dedicated TestFlight build, or fold into the next one.
3. D3: identity (phone key, nsec import, one identity, account link, Log out,
   backup).
4. D4: default relays and user-added relays.
5. D5: media upload path.
6. D6: push notifications in v1.
7. D7: drawer or tab bar.
8. D8: profile picture fallback.

## Keeping the docs current

- The audits (01 to 04) describe fixed revisions: Arcade at `3f10e1e`, the
  target at `450ca4738f` (04's header). Do not edit them to track later
  work; a correction to an audit's reading of its own revision is fine and
  says so.
- Part 06 is a dated addendum. Its source catalog stays pinned; architecture
  status and implementation evidence update through the plan and component
  rendering specification. A recommendation is not a passed device check.
- The port's issues update [05-port-plan.md](05-port-plan.md): when an issue
  lands, mark its row in section 5; when the owner decides, move the decision
  out of section 7 into a "Decided" note with the date; when main changes a
  fact the plan relies on, add it to section 0.
- New port work gets its own issue, filed from section 5's lists, not from
  the audits.
