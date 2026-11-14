# Arcade port, part 5: the port plan

Tracking issue: #13607. This plan follows from the four audits in this
folder. It decides what the port builds, in what order, on which modules,
and who builds each part. It files no issues: section 5 lists them for the
port's tracker.

**2026-11-14 rendering amendment:** the owner requires every Arcade animation
ported to the shared Coax component library, including dormant/demo effects.
[Audit 06](06-rendering-and-animation.md) is the completeness list;
[components/rendering.md](../../components/rendering.md) is the architecture
recommendation and acceptance plan. Shared GPU visuals with platform text/
accessibility services supersede building a permanent per-platform widget
set. The shipped UIKit runtime is retained until the new path qualifies.

| Input | Revision |
| --- | --- |
| Arcade, read-only clone `~/work/arcade` | `3f10e1e` (2023-08-05), `app/arclib` at `542ecec` |
| [01-codebase.md](01-codebase.md) | `89fc8c8c27` |
| [02-nostr.md](02-nostr.md) | `5bf0d987e0` |
| [03-design-and-components.md](03-design-and-components.md) | `4958f3ddbb` |
| [04-target-app.md](04-target-app.md) | `26baeb6c18`, describing target commit `450ca4738f` |
| Spot checks for this plan | `origin/main` at `5bf0d987e0` (2026-11-14) |

"Verified" means read in the code at `5bf0d987e0`. Everything else cites the
audit it comes from.

## Contents

0. Where the audits disagree, and which fact this plan uses
1. Scope and the 65-row disposition table
2. Nostr design
3. Native foundation (the one TestFlight build)
4. Design system
5. Phases and the issues to file
6. Ownership and coordination
7. Decisions for the owner
8. Risks and what is not in this plan
9. Flagged outside the port: the committed Breez key

---

## 0. Where the audits disagree

The audits were written in parallel on 2026-11-14 while the iOS app changed
underneath them. Where two disagree, or where the code at `5bf0d987e0`
disagrees with one, this plan uses the fact in the right-hand column.

| # | Topic | Audit says | This plan uses | Why |
| --- | --- | --- | --- | --- |
| C1 | Non-grayscale colours in the native app | 03 B1 and B5.2: blue send disc `#346bf1` (`NativeComposer.swift` line 91), links `#8cb4ff`, refresh in system green and orange; native colours `#e5e5e5`, `#262626`, `#383838`, `#191919` | **Already fixed on main.** `components/skin/native-chat.coax` lines 9 to 13 derive every role from the mono tokens (link = ink, underlined; bubble = surface; code fill = raised; border = line dim). The send button is `NativeButton(.action)` (`NativeComposer.swift` line 10). The refresh states are shapes (`NativeControls.swift` line 106). `coax/native/stock-style.coax` rejects system colours. The one leftover is the stale line in `coax/native/composer.md` line 50 that still names `#346bf1`. | Commit `569f03794e` ("every control drawn from the component library in the mono skin") landed before 04's target commit and is what 04 §1.4 describes. 03's native values are exactly those of the tree before it (`git show 569f03794e^` of `native-chat.coax` and of `NativeComposer.swift`, whose send disc was `UIColor(red:52/255,green:107/255,blue:241/255)`). Verified. |
| C2 | Drawn frames on native | 03 B1: "no ... drawn frames yet"; B3: the framed-view primitive is "the largest single native addition" | `NativeFrameView` exists (`apps/shared/native/NativeChrome.swift` line 141) and draws every mono frame role (button, panel, field, notice, header, row, bubble, box) from Coax points (`mono_native_frame`). The work is to project more elements through it, not to build it. | 04 §1.4, §2.2; verified. |
| C3 | Message bubble shape | 03 B1 and B5.1: the person's message is a rounded T3 bubble, radius 20 | The person's message already stands in the skin's dim `bubble` frame (`NativeMessage.swift` lines 122 and 139); the answer is unframed. Decision D11 is about keeping it. | Verified. |
| C4 | Native controls for switches, alerts, sheets, menus | 03 B3 maps to `UISwitch`, `UIAlertController`, `UISheetPresentationController`, `UIContextMenuInteraction`, `UITabBarController` | Draw each through `NativeChrome.swift` on skin tokens. `stock-style.coax` fails `UISwitch`, `UIAlertController`, `UIVisualEffectView` and corner radii outside `NativeChrome.swift`; a platform-owned control needs a `// stock:` reason. Menus use the existing `NativeMenu`; sheets use the existing `role=dialog` sheet. | `coax/native/stock-style.coax` lines 1 to 60, verified; 04 §1.4 and §6. |
| C5 | Fonts | 03 open question 4: only DM Sans is bundled; Geist unsettled | DM Sans for body (`components/fonts`), GeistMono for chrome labels, already packaged from `components/zeron/fonts` (Geist, Geist Medium, Geist SemiBold, GeistMono present). | 04 §1.4; directory verified. |
| C6 | Group module | 04 §5 and open question 3: no NIP-28 and "no obvious group module" | `nip29.coax` exists but covers the relay side only; there is no NIP-29 client module. | 02 B13, which cross-checks 04. Verified the file exists. |
| C7 | Public channels | 04 §5 and risk 11: "Add `nip28.coax`" | Do not build NIP-28 channels. NIP-28 is `unrecommended` ("try NIP-29 instead"); `retired.coax` refuses to sign kinds 40 to 44. A small read-only NIP-28 reader is built only if decision D9 keeps old channel history. | 02 A4, B5, checked against the nips clone at `1fb3a95`. |
| C8 | Default relay | 04 risk 5: "the `immortal` relay" | `relay.openagents.com`, which is the Coax relay in `nostr/relay/` since 2026-11-12 (#13361). | 02 B2; `coax/nostr.md` §17 line 609, verified. |
| C9 | Navigation | 03 B5.4: keep three tabs, drop the drawer | The app is drawer-based (`NativeWorkspace`); there is no native tab control. Decision D7 recommends the drawer. | 04 §1.3, §5, risk 7. |
| C10 | Where joined channels and settings go | 01 §12.1: `nip78.coax` app data or `nip51-private.coax` | Joined groups: kind 10009 (NIP-51 simple groups). Private follows: kind 30000 private items. Settings: kind 30078 (NIP-78), NIP-44 to self, or the account. Never kind 0, 10003 or 30040. | 02 B6, which checked kinds against today's registry. |
| C11 | NIP status | 01 open question 7: the nips clone was missing | The clone exists at `~/work/projects/repos/nips` (`1fb3a95`); 02 checked every status there. | 02 header. |
| C12 | Animation scope, amended 2026-11-14 | 03/this plan drop glow, GIF/splash/typing and replace custom blur/spinners | Every A01–A28 case in 06 gets a reusable shared gallery specimen, including dormant code. Product screens can choose quieter variants. | Owner request; 06's pinned source catalog. |
| C13 | Permanent component renderer, amended 2026-11-14 | K controls drawn separately through the Swift host | Recommend shared GPU paint/motion for new native component visuals, native text/accessibility services, DOM text with GPU islands. Current UIKit remains compatible until qualification. | `components/rendering.md`; no speed/parity claim without actual-host evidence. |
| C14 | Identity assumption, amended 2026-11-14 | The phone key is the primary npub | Account-managed identities were subsequently approved and deployed; reconcile the Nostr ops with that custody before treating a phone key as canonical. Rendering does not decide identity. | `docs/protocol/account-identity-v1.md`; existing linked keys remain preserved. |

---

## 1. Scope

### 1.1 The product the port produces

Arcade was one inbox for Nostr chat: public channels, encrypted direct
messages, small private groups and an AI chat, around a Nostr identity,
contacts and relays (01 §1.1). The port puts that idea inside OpenAgents iOS
as **Nostr messaging next to the agent**, not as a second app:

- **One inbox.** The drawer's "Recent chats" (`home/navigation.coax`) lists
  agent chats and Nostr conversations together, newest first. A Nostr
  conversation is a direct message (NIP-17), a private group of up to 10
  (a NIP-17 room), or a public or large group (NIP-29 on
  relay.openagents.com).
- **The agent is Arcade's AI chat.** Arcade's "Faerie" chat becomes the
  account chat the app already has (`/api/chat/complete/v1`, under the
  account session). There is no second AI.
- **One identity.** The phone's Nostr key (`NostrKey.swift`, Keychain
  `com.openagents.app.nostr` / `nsec-v1`) is the user's npub, linked to the
  email account. Profile, follows, relays and groups hang off it.
- **Settings gains three rows:** Your profile, Relays, Nostr key. Everything
  else in Settings stays.

How it sits next to what the app already has (04 §1.2):

| Existing | Relation to the port |
| --- | --- |
| Agent chat (page 3) and saved chats in the drawer | Shares the drawer list and the composer. Agent chats stay on the server (`chat.account_threads_v1`); Nostr events live in the new local store (04 §4.2). |
| Your computers (page 4) | Untouched. |
| Worlds | Untouched. A world may later link to a group; not in this plan. |
| Projects (pages 5 to 9) and member chat | Untouched. Project member chat stays on the account API; it is not moved to Nostr in this plan. |
| Bitcoin balance (Extras) | Untouched. Zaps and paid listings need wallet send, which is compiled out (`COAX_NATIVE_BALANCE`, 04 §5); deferred. |
| Log in by email code | Stays the front door. The Nostr key joins the account (`auth.keyLogin`, `linkNostr`); there is no separate "Create account". |

### 1.2 Rules that replace Arcade's defects

| Arcade defect (source) | Rule in the port |
| --- | --- |
| No signature verification: `skipVerification: true`, cache rows `verified = 0` (01 §11.3.3, 02 A12.1) | Every event is verified (`event_verify_fast.coax` or `message.coax`'s verifying read) before it enters any store, list or screen. An unverified event is dropped, never shown. |
| Private key in app state: a second copy in the `root-v1` snapshot, channel keys only there (01 §11.3.1, 02 A12.7) | The secret key never leaves native memory. The Coax module gets `nostr.pubkey`, `nostr.sign` and NIP-44 effects only (`coax/nostr.md` §5; 04 §3.3). Group secrets, if any, live in the native key store, never in module state or the event store. |
| Logout leaves keys and data: channel store, SQLite cache, `meta`, `appk` survive (01 §11.3.2, 02 A12.8) | Log out wipes every per-account record: the event store namespace, group state, drafts, previews. A test logs out, logs in as a second account and finds nothing of the first. What happens to the Keychain key is decision D3. |
| Unauthenticated AI chats readable by npub (01 §7.1, §11.3.4) | AI chat is the account chat under the bearer token (`AppAuth.authorize`). Nothing is keyed by npub. |
| Hard dependency on a registration service: signup fails if the NIP-05 Lambda is down (01 §4.3) | No step of Log in or first use depends on a service other than the account API and one relay. NIP-05 is optional and checked, never required. |
| Writes that drop data: kind 3 rewritten with `p` tags only; lists written before they were read (01 §11.2.5, §11.2.6) | Read before write: a replaceable list (3, 10002, 10050, 10009, 30000) is written only after the newest copy was read from the user's relays, and the write keeps every tag it did not change. |
| Custom ciphers and unregistered kinds (02 A3, A5, A6) | Only NIPs from `nostr/nips/official/` and their kinds. Never write kinds 4, 40 to 44, 99, 400 to 403, 10003 (as contacts), 20003, 30040 or 30199. |
| Images in private chats uploaded in the clear (01 §11.3.7) | Media in a NIP-17 conversation is encrypted before upload (NIP-17 kind 15). |
| Link previews fetched from the device on render (01 §11.3.8) | No fetch of a linked page from the device unless the user taps it. |
| Key copied to the clipboard forever (01 §11.3.9) | No plain nsec on the clipboard. Export is NIP-49 `ncryptsec` once that module is approved (02 B13.7). |
| Members known only to the inviting device (01 §11.2.13) | Membership comes from the protocol: a room's `p` tags, a NIP-29 group's kind 39002. |
| Failed sends only logged (01 §11.1) | An outbox with visible states: sending, sent, failed, with retry (`relay.coax` resends unacknowledged publishes). |

### 1.3 Disposition of every inventory row

From 01 §13. Disposition: **Port** (as is), **Update** (port, updated to
what is named), **Merge** (into the named OpenAgents feature), **Defer**,
**Drop**. Phase refers to section 5.

| # | Feature | Disposition | Reason | Phase |
| --- | --- | --- | --- | --- |
| 1 | Welcome with Create account and Log in | Merge: the existing Log in (page 0, email code), Nostr key second | Email code creates the account; one "Log in" flow (03 B5.6) | 1 |
| 2 | Create a Nostr identity on device | Merge: `NostrKey.swift` lazy key | The phone already makes one key; no second generator (02 B1) | 0 |
| 3 | Profile setup at signup | Update: an optional "Your profile" screen after first Log in; kind 0 with NIP-01/24 fields | Signup must not wait on a profile; kind 0 writer is gap G13 | 1 |
| 4 | Free NIP-05 name at signup | Defer (D16) | `@arcade.chat` is not ours; needs our own `nostr.json` service | 6 |
| 5 | Publish kind 0 | Update: standard fields only; read `username` as a fallback for `name`; never write `other` | 02 B1 | 1 |
| 6 | Log in with nsec or hex | Update: "Use an existing Nostr key" writes into the Keychain slot through a native op, Face ID gated, never through JSC | 02 B1, 04 §5; depends on D3 | 0, 1 |
| 7 | Restore state on login | Update: read 10002, 10050, 3, 30000, 10009, 10000 before any write | Read-before-write rule (1.2) | 1 |
| 8 | Auto-join default channels | Update: default NIP-29 groups on relay.openagents.com | Arcade's defaults were NIP-28 channels on Arcade's relays, whose state is unknown (02 B5, Q1); names are D9 | 4 |
| 9 | Key backup | Update: NIP-49 export behind Face ID once `nip49` is approved; no export screen before then | 02 B13.7, 03 B5.7; D3 | 6 |
| 10 | Log out | Merge: the existing Settings → Log out, extended to wipe all Nostr data, with a confirm | Defect rule (1.2) | 1 |
| 11 | Unified inbox | Merge: drawer "Recent chats", agent and Nostr conversations together, with unread counts | 03 B5.4, 04 §1.3 | 2 |
| 12 | Inbox row content | Port: avatar, name, last sender, last line, time (`h:mm A` under 12 hours, else `MMM DD`) | 01 row 12 | 2 |
| 13 | Pull to refresh | Update: live subscriptions plus the existing refresh button; no pull gesture | Native has no pull to refresh (04 §2.2); subscriptions make it unneeded | 2 |
| 14 | Side drawer | Merge: `NativeWorkspace` drawer | 04 §1.3; D7 | 2 |
| 15 | New conversation sheet | Merge: the header and composer Plus menu gains New message, New group, Find groups | 03 B5.4 | 2 |
| 16 | Contact list with remove | Update: follows (kind 3) through `follows.coax` and a kind 3 writer | 02 B6 | 2 |
| 17 | Add contact by npub, hex, NIP-05 | Update: `nip19-core.coax`, `nip05.coax` (host fetch, redirects refused); drop Arcade's fixed suffix list | 02 A1, B1 | 2 |
| 18 | Suggested people | Update: follows of follows (kind 3) and NIP-50 search on our relay | No nostr.band (02 B11); search needs G6 | 2 |
| 19 | Public follow | Update: kind 3 writer that keeps relays, petnames and other tags | 01 §11.2.6 | 2 |
| 20 | Private follow | Update: NIP-51 private items in kind 30000 via `nip51-private.coax` | Kind 10003 is bookmarks now (02 B6) | 2 |
| 21 | Per-contact DM mode (legacy or blinded) | Drop | One DM mode: NIP-17 (02 B3) | — |
| 22 | User profile view | Update: add a loading state; Message, Follow, Private follow | 01 row 22 | 2 |
| 23 | Own profile and settings hub | Merge: Settings dialog gains Your profile, Relays, Nostr key | 04 §1.2 | 1 |
| 24 | Edit profile | Update: add banner; standard fields; no push-flag fields | 01 §11.2.2 | 1 |
| 25 | Public channel chat | Update: open NIP-29 group, kind 9 messages (NIP-C7), `q` replies, paged history | 02 B5 | 4 |
| 26 | Channel discovery | Update: kind 39000 from relay.openagents.com, plus kind 10009 lists of people you follow | 02 B11 | 4 |
| 27 | Create public channel | Update: NIP-29 `create-group` (kind 9007) on relay.openagents.com | NIP-29 text at `1fb3a95` | 4 |
| 28 | Join and leave, synced | Update: kinds 9021 and 9022, list in kind 10009 | 02 B5 | 4 |
| 29 | Channel manager | Merge: a group's own settings screen, reached from the group header | 03 B5.4 drops the drawer button | 4 |
| 30 | Create private group | Update: NIP-17 room, up to 10 members | 02 B4; D10 | 3 |
| 31 | Invite to private group | Update: adding a member starts a new room with the new `p` set (NIP-17); large groups use NIP-29 invites (kind 9009) | 02 B4 | 3, 4 |
| 32 | Private group members | Update: a room's `p` tags; a NIP-29 group's kind 39002 | Membership from the protocol (1.2) | 3, 4 |
| 33 | Direct messages | Update: NIP-17 (kinds 14, 13, 1059) through `nip17.coax`, `nip59.coax`, `nip44.coax` | 02 B3 | 2 |
| 34 | Blinded DMs and mask badge | Drop | NIP-17 hides the sender from relays; Arcade's format lets either party forge (02 A6). Reading old ones is D14 | — |
| 35 | Swipe to reply | Update: native swipe on a message (Phase 0 control); reply is `["e", id, relay]` on kind 14 (NIP-17) and `q` on kind 9 (NIP-C7); add the replied-to author's `p` | 02 B6; NIP-17 text | 2 |
| 36 | Message rendering, mentions | Update: `nip27.coax`; mentions show names and open the profile or note | 02 B8 | 2 |
| 37 | Inline images | Port: on the Phase 0 image node | 04 §5 | 5 |
| 38 | Inline video | Defer | Needs an AVPlayer view in a later binary; not core to messaging | 6 |
| 39 | Link preview card | Defer | Needs a server-side preview service; never fetched on render (1.2) | 6 |
| 40 | Attach an image | Update: upload path per D5; kind 15 encrypted file in NIP-17 conversations | 01 §11.3.7; G3 | 5 |
| 41 | Reputation next to sender | Drop | Arcade's `quality` annotation left NIP-32 in 2023 (02 A7); 03 B5.9 | — |
| 42 | AI chat ("Faerie") | Merge: the account chat, with the OpenAgents mark as its avatar | 03 B2.3, B5.5; D13 | 2 |
| 43 | AI voice input | Defer to the app's own voice work | The composer's mic is present but disabled (04 §2.2); not a Nostr feature | — |
| 44 | AI model picker | Merge: the composer's existing model menu | 04 §2.2 | — |
| 45 | Optimistic send with rollback | Update: the Nostr outbox shows sending, sent, failed and retries | 1.2 | 2 |
| 46 | Relay manager | Update: publish kind 10002 and 10050; keep URL paths; `wss` only | 02 B2 | 1 |
| 47 | Default relays | Update: relay.openagents.com for 10002 and 10050 | D4 | 1 |
| 48 | Push preferences | Defer (D6) | No APNs today (04 §2.11) | 6 |
| 49 | Push registration via kind 30199 | Drop | Register devices through the account service when push lands (02 B14.8) | — |
| 50 | Privacy setting (default DM mode) | Drop | Never read in Arcade; one DM mode now | — |
| 51 | Private settings inside kind 0 | Update: kind 30078 (NIP-78), NIP-44 to self, or the account | 02 B6 | 1 |
| 52 | Local event cache | Update: a client event store module (G2) over the native store ops; verified events only; wiped on Log out | 02 B13.2, 04 §3.3 | 0, 1 |
| 53 | Batched profile fetch | Port: `profiles.coax` through the outbox planner | 02 B2 | 2 |
| 54 | In-app changelog | Merge: Settings → About and `release-notes/` | 01 row 54 | — |
| 55 | Social and source links | Merge: Settings → About, pointing at OpenAgents | 01 row 55 | — |
| 56 | Marketplace listings | Defer (D12) | Unreachable in Arcade; if kept, NIP-99 kind 30402 priced in BTC or sats | 6 |
| 57 | Offers and accept | Defer (D12) | If kept, offers over NIP-17 to the listing's author, by its `a` address (02 B7) | 6 |
| 58 | "Comment to trade" commands | Drop | A regex command grammar is the ad hoc string routing the workspace forbids; no successor (02 B7) | — |
| 59 | Geohash on listings | Defer with listings | Keep the 5-character cap if location returns | 6 |
| 60 | Discover hub | Merge: group discovery (row 26) | Placeholder in Arcade | 4 |
| 61 | Error boundary | Merge: the app's update trial and rollback (04 §2.12) | Ignite boilerplate | — |
| 62 | Haptics | Update: inside the native controls (swipe complete, menu open), no service op | 03 B5.10 | 0 |
| 63 | Long-press popup menu | Update: long press opens the existing `NativeMenu` (Phase 0 gesture) | C4 | 0, 2 |
| 64 | List fade-in, HUD frames, splash | Update: skin frames through `NativeFrameView` and the skin's enter motion; no splash | 03 B2.3, C2 | 0 |
| 65 | Dark-only appearance | Port | The app is dark only (`UIUserInterfaceStyle: Dark`, 04 §1.1) | — |

---

## 2. Nostr design

### 2.1 Chosen NIP per feature

Statuses are from 02 Part B (checked at nips `1fb3a95`). Modules are in
`coax/lib/nostr/` unless a path is given; each was confirmed to exist at
`5bf0d987e0`.

| Feature | NIP and kinds | Module | State today (02) | Work item |
| --- | --- | --- | --- | --- |
| Identity and key storage | NIP-19 (`npub`, `nsec`); key in Keychain `com.openagents.app.nostr` / `nsec-v1` | `nip19-core.coax`, `keys.coax` (signer contract); `NostrKey.swift`, `Platform/NostrSign.c` | Generate and sign natively; no import; not reachable from Coax | N1 to N5 (section 3), G8 |
| Key backup | NIP-49 `ncryptsec` | `crypto/scrypt_nip49.coax` (experimental) | No `nip49.coax` | G7 (later) |
| Keys kept elsewhere | NIP-46 | `nip46.coax` | Complete, NIP-44 transport | Later, not v1 |
| Relay list and outbox | NIP-65 kind 10002; outbox planner | `nip65.coax`, `outbox.coax`, `relay.coax`, `message.coax`, `coax/lib/net/relay_conn.coax` | Reader and planner; no 10002 writer named | G15 |
| Relay auth | NIP-42 kind 22242 | `nip42.coax`, `nip42-event.coax` | Complete; relay.openagents.com requires it for writes and for reading one's own wraps | — |
| DM inbox relays | NIP-17 kind 10050 | `nip17.coax`, `nip51.coax` | Read and required by `nip17_plan` | G15 (writer) |
| Direct messages | NIP-17 (14, 15) in NIP-59 (13, 1059), NIP-44 v2 | `nip17.coax`, `nip59.coax`, `nip44.coax`, `nip44-key.coax` | Send and open complete for up to 10 recipients | Needs N3 and N4 on the phone |
| Old DMs | NIP-04 kind 4, read only | `nip04.coax` (encrypt is a named refusal) | Decrypt only | Needs N3 (ECDH) to read; D14 |
| Small private groups (up to 10) | NIP-17 rooms | `nip17.coax` | Same as DMs | G3 for reactions, deletes, files |
| Large and public groups | NIP-29: `h` tag, 9000 to 9022, relay-signed 39000 to 39003; chat is NIP-C7 kind 9 | `nip29.coax` (relay side), `nostr/relay/relay_groups.coax`, `group_read_policy.coax`; `nipc7.coax` | Relay side only; relay does not advertise 29 | G1, G14 |
| End-to-end groups above 10 | Marmot (MLS); NIP-EE is `unrecommended` | none | Not built; missing X25519, HPKE, AES-128-GCM, Ed25519 signing | G11, only if D10 says so |
| Public channels | NIP-29 open groups (NIP-28 is `unrecommended`) | as above; `retired.coax` refuses 40 to 44 | — | G1; G9 only for read-only history |
| Contacts and follows | NIP-02 kind 3 | `follows.coax` | Reader | G16 (writer that keeps tags) |
| Private follows | NIP-51 private items, kind 30000 | `nip51.coax`, `nip51-private.coax` | Seal and open, NIP-44 to self | Needs N3 |
| Joined groups | NIP-51 kind 10009 (10005 only to remember old NIP-28 channels) | `nip51.coax` | Tag rules | G15 |
| Mute | NIP-51 kind 10000 | `nip51.coax` | Answers "is this muted" | — |
| Profiles | NIP-01 kind 0 with NIP-24 fields | `metadata.coax`, `profiles.coax` | Reads three fields | G13 |
| NIP-05 checks | NIP-05 | `nip05.coax` | Offline parse and check; host fetches; redirects refused | Host fetch through the existing `http` op if the path policy allows it, else a later op (open question Q3) |
| App settings | NIP-78 kind 30078, NIP-44 to self | none needed (plain addressable event) | — | — |
| Replies | NIP-17 `e` reply tag on kind 14; NIP-C7 `q` on kind 9; NIP-22 kind 1111 for comments on non-chat kinds | `nip17.coax`, `nipc7.coax`, `nip22.coax` | Reader and writers | — |
| Reactions | NIP-25 kind 7 | `nip25.coax` | Reader and tally | G3 inside rooms; a kind 7 writer for groups |
| Deletion | NIP-09 kind 5 | `nostr/relay/store_delete.coax` | Relay side only | G5 |
| Mentions and links | NIP-21, NIP-27 (NIP-08 read only) | `nip27.coax`, `nip19-core.coax` | Complete | — |
| Media | NIP-B7 Blossom (kinds 10063, 24242), NIP-94; NIP-92 `imeta` | `nipb7.coax`, `nip94.coax`; no `nip92.coax` | Not checked for this port (02 B13.12) | G12; depends on D5 |
| Search | NIP-50 | `nip50.coax` (relay side) | Relay answers; client `NostrFilter` has no `search` | G6 |
| HTTP auth | NIP-98 | `nip98_sign.coax`, `coax/web/api/nip98.coax` | Complete; the phone already uses it | — |
| Listings, if kept | NIP-99 kinds 30402, 30403; BTC or sats price | `nip99.coax` (reader); `nip15.coax` read only | No writer | G4, only if D12 keeps listings |
| Zaps, if any | NIP-57 kinds 9734, 9735 | `nip57.coax` | Complete library | Needs wallet send; not in this plan |
| Reputation | NIP-32 kind 1985 | `nip32.coax` | Reader | Not in this plan (row 41) |

### 2.2 Gap list as work items

Numbered G1 to G16 so section 5 can refer to them. G1 to G14 are 02 B13's
list in its order; G15 and G16 are writers this plan needs that 02 names only
as readers. All belong to the Nostr owner (section 6).

| # | Work item | Needed by | Phase |
| --- | --- | --- | --- |
| G1 | NIP-29 client: join and leave requests (9021, 9022), `h` and `previous` tags, `create-group` (9007), invites (9009), and reading relay-signed 39000 to 39003 against the relay's NIP-11 `self` key | Public and large groups | 4 |
| G2 | Client event store: durable, verified, replaceable and addressable rules, per-account namespace, wipe on Log out, capped history; over the native store ops (N2) | Every phase from 1 | 1 |
| G3 | NIP-17 extras: reactions, deletes and edits inside a room, expiration on each wrap, kind 15 file encryption (`coax/lib/crypto/aes_gcm.coax` now exists) | Phases 3 and 5 | 3, 5 |
| G4 | NIP-99 writer | Listings, if kept | 6 |
| G5 | NIP-09 client writer | Deleting own group messages | 4 |
| G6 | `search` in client filters (`filter.coax` `NostrFilter`) | Find people and groups | 2 |
| G7 | Reviewed NIP-49 module (`ncryptsec` reader and writer) | Key export | 6 |
| G8 | Key import on iOS (`NostrKey` only generates) | Row 6; depends on D3 | 0 |
| G9 | NIP-28 read-only reader (kinds 40 to 42, marked `e` tags) | Only if D9 keeps old channel history | 4 |
| G10 | Arcade legacy decrypt adapter (02 B15) | Only if D14 says yes | 6 |
| G11 | MLS / Marmot | Only if D10 needs end-to-end encryption above 10 | not planned |
| G12 | Media metadata: a NIP-92 `imeta` module, and a review of `nipb7.coax` and `nip94.coax` against the D5 path | Attachments | 5 |
| G13 | Kind 0 writer for NIP-24 fields including `lud16` | Your profile | 1 |
| G14 | Advertise and wire NIP-29 on relay.openagents.com (`supported_nips` in `nostr/relay/relay_server.coax` line 597 lacks 29; 02 B14.2, open question 5, #13382) | Phase 4 | 4 |
| G15 | Writers for kinds 10002, 10050 and 10009 that keep unknown tags (read before write) | Relays, groups | 1, 4 |
| G16 | Kind 3 writer that keeps relay hints, petnames and other tags | Follows | 2 |

ECDH on the phone (02 B13.14) is not a library gap: it is native work item N3.

---

## 3. Native foundation

### 3.1 The one build

04 §3.3 and risk 1: every capability below changes the installed contract
(the runtime fingerprint), so it needs a TestFlight build. They land
together in **one** build so that every later phase ships by signed Update.
The principle from `coax/nostr.md` §5 holds throughout: the script gets
effects, never keys.

The component foundation below is now part of that objective. The original
N1–N12/K1–K11 list does not include a complete GPU compositor, autonomous
motion runtime or qualified mobile IME/accessibility bridge. Do not promise
all animation/scene capabilities over the air to build 1085. Consolidate the
new installed contracts once qualified; additional unqualified capabilities
can require later binaries. Nostr library/service work can proceed separately.

### 3.2 Every native operation and control

"Exists" is as of `5bf0d987e0`. The op table is
`apps/shared/native/NativeHost.swift` line 96 (verified: it holds `auth.state`,
`auth.emailStart`, `auth.emailVerify`, `http`, `auth.logout`,
`updates.check`, `updates.status`, `http.cancel`, `clipboard.write`,
`link.open`, `environment.commandIDs`, `environment.wait`, `world.open`,
`documents.pick`, `documents.upload`, `documents.release`).

**Service ops**

| # | Name | Purpose | Needed by | Exists? |
| --- | --- | --- | --- | --- |
| N1 | `socket.open`, `socket.send`, `socket.close`, with inbound `socket.message` and `socket.closed` events | Relay WebSockets for `relay.coax`. Lift from `PageHost.m` line 1322: 4 slots, 8 MiB messages. `wss` only. Which hosts are allowed is D4 | Every Nostr feature | For worlds only (`PageHost.m`); not in `NativeHost` |
| N2 | `store.get`, `store.put`, `store.list`, `store.wipe` | The client event store (G2) and small app state, per-account namespace, files 0600, over the `phStorage` layout (`PageHost.m` line 1380: namespace 1 to 64 bytes, key 1 to 128 bytes, value up to 1 MiB). `store.wipe` serves Log out | Phases 1 to 6; Log out rule | For worlds only |
| N3 | Turn on libsecp256k1 ECDH (`-DENABLE_MODULE_ECDH=1` next to the flags at `project.yml` line 88) and add `nostr.encrypt44(peer, plaintext)` and `nostr.decrypt44(peer, payload)` | NIP-44 v2 with the conversation key kept in C. The cipher is `coax/lib/nostr/nip44.coax` compiled to C; the nonce comes from SecRandom inside the op | DMs, rooms, private follows, settings, reading NIP-04 | No: ECDH not compiled (04 §2.10) |
| N4 | One-time keys for gift wraps: an op that makes an ephemeral key in native memory, returns its pubkey, encrypts with it and signs the wrap with it, then destroys it | NIP-59 wraps without a secret in JSC; pure-Coax signing in JSC is too slow and would hold the key in script memory (04 §2.10) | DMs, rooms | No. 04 §3.3 item 4 offers `random(n)` or native generation; this plan takes native generation. The exact op shape is for the Nostr owner (section 6) |
| N5 | `random(n)` | Secure random bytes for non-secret uses (NIP-59 timestamp jitter, request ids) | DMs, rooms | No: JSC has no `crypto.getRandomValues` |
| N6 | `nostr.pubkey`, `nostr.sign(id32)` | The user's npub and BIP-340 signatures over `NostrKey` and `oa_nostr_sign` | Every write | Code yes (`NostrSign.c`), op no |
| N7 | `nostr.import(nsec or hex)` with a Face ID gate | Row 6, G8 | Only if D3 allows import | No |
| N8 | `auth.keyLogin` and `nostr.link` in the op table | Log in with the phone key; link it to the email account | Phase 1, D3 | `call()` has `auth.keyLogin` (lines 193 to 194) and `AppAuth.linkNostr` exists, but neither is in the op table, so Coax cannot reach them |
| N9 | Wall clock | `created_at` on every event | Every write | Open: 04 open question 10 asks whether the module can read it directly. The build adds an op only if it cannot |
| N10 | `photo.pick` | Avatar, banner and attachments from the photo library (PHPicker) | Phases 1 and 5 | No. `documents.pick` is a document picker only. Whether a photo-library usage string is needed for PHPicker is inferred not to be, and is checked in the issue |
| N11 | HTTP path additions to `cn_api` / `nwp_account` | The media upload route (D5), and a NIP-05 fetch if the host does it (Q3) | Phases 1 and 5 | Policy exists; paths depend on D5 |
| N12 | APNs registration (`aps-environment production`, `registerForRemoteNotifications`, `AppAuth.pushTarget`) | Push | Only if D6 says push in v1 | No (entitlement is development only) |

**Controls and roles.** The table's “Exists” column describes the original
native projection. The amended permanent target is shared kit state/layout/
paint/motion, consumed by the GPU runtime with native semantic/text adapters.
`NativeChrome.swift` remains the compatibility implementation for current
screens. Extend it only where the migration needs a platform bridge; do not
write a second complete set of component visuals there or in Android. Skin
roles and the existing style gate still apply to host-owned chrome.

| # | Control | Purpose | Needed by | Exists? |
| --- | --- | --- | --- | --- |
| K1 | Shared image component: admitted decode/cache, GPU image primitive, aspect fit and loading state; compatible native role where needed | Message images, banners, group pictures | Phases 1, 2, 5 | No `img` mapping (04 §2.2); GPU image primitive currently skipped |
| K2 | Avatar: K1 in the shared circular frame, with a fallback per D8 | Every row and message | Phase 2 | Shared image/resource path and semantic avatar binding needed |
| K3 | Row with avatar, title, line, time and trailing actions (`k_row`, `k_row_with`) | Inbox, contacts, members, groups | Phase 2 | Button `MB_ROW` variant exists; no avatar row |
| K4 | Switch (`k_switch`, `k_switch_row`) drawn and animated by the shared kit, with semantic switch state | Private follow, group flags | Phase 2 | GPU controls exist; tree/native adaptation and A16 parity needed; no separate UISwitch appearance |
| K5 | Shared long-press menu, with owned background blur/lift (A18); native haptic effect | Copy, reply, mute, leave | Phase 2 | `NativeMenu` is compatible today; shared gesture/effect path needed |
| K6 | Shared swipe-to-reply state, translated message, arc trim and spring completion (A07–A10); native light-haptic effect | Row 35 | Phase 2 | No swipe actions, no haptics (04 §2.2); GPU arc path needed |
| K7 | Message from another author: avatar, name and time on the left, in the skin's bubble or unframed (D11) | Every conversation | Phase 2 | `NativeMessage` frames the person's own message only |
| K8 | Confirm component (title, body, two actions), shared modal/focus semantics; existing dialog as compatibility surface | Leave group, remove contact, Log out, replace key | Phases 1 to 4 | Sheet exists; shared confirm/transition variant needed |
| K9 | Reply quote row above the composer, dismissible | Row 35 | Phase 2 | `NativeComposer` exists; no quote row |
| K10 | Attachment thumbnail row in the composer | Row 40 | Phase 5 | No |
| K11 | Enabling the composer Plus menu to hold New message, New group, Find groups, Attach | Row 15 | Phase 2 | Plus menu exists (New chat, Clear draft); new items are module data (verify in the issue whether they need a binary) |

Not in the build: a tab bar (D7 recommends the drawer), a virtualized list
(04 open question 8: page in Coax first, risk R3), video (row 38), QR
scanning (04 §5), background sync (04 §2.11).

### 3.3 What ships afterwards by signed Update

Everything in phases 1 to 5 that is Coax: the Nostr library modules (G1 to
G16 except G14, which is the relay), the client store logic, every screen,
copy and layout from the controls above, skin token values, relay lists,
NIP-17 and NIP-29 flows, and group discovery (04 §2.12). Anything that
needs a new op, a new tag, a new HTTP path, an entitlement or an Info.plist
key needs another build.

---

## 4. Design system

### 4.1 Grayscale token table, final

03 B2.2 mapped every Arcade token to mono. Its native column used the
pre-`569f03794e` `nt_skin()` values (C1). The final table uses the native
tokens in `components/skin/mono-native.coax` (04 §1.4, verified lines 14 to
16), which already include the `surface` and `faint` roles 03 asked for.

| Arcade token | Arcade value | Meaning | Mono token (web) | Native token (`mono-native.coax`) |
| --- | --- | --- | --- | --- |
| `almostBlack`, `black`, `background` | `#030303`, `#000` | Screen, sheets | `--ground` `#000` | GROUND `000000` |
| `overlay50` | cyan 5% | Own bubble, quiet wash | `--surface` `#0a0a0a` | SURFACE `0a0a0a` |
| `overlay20` | cyan 11% | Others' bubble, composer, field, card | `--raised` `#161616` | RAISED `161616` |
| `cyan950` | `#083344` | Swipe disc, listing fill | `--raised` | PRESSED `262626` |
| Press fill `rgba(255,255,255,.1)` | | Press feedback | `:active` on `--raised` | PRESSED `262626` |
| `overlay80` | black 50% | Scrim | scrim | The drawer backdrop's existing scrim (04 §1.3) |
| `cyan900`, `separator`, `#232324` | `#164e63` | Hairlines, frames, dividers, avatar ring | `--line-dim` `#2a2a2a` | LINE_DIM `2a2a2a` |
| `cyan500` and `cyan600` as borders | | Accent borders | `--line` `#6b6b6b` | LINE `6b6b6b` |
| `cyan800`, `cyan700`, `textDim` | `#155e75`, `#0e7490`, `#564E4A` | Inactive icon, placeholder, quiet icon, disabled | `--ink-faint` `#5c5c5c` | INK_FAINT `5c5c5c` |
| `cyan600`, `cyan500`, `gray`, `rgba(255,255,255,.5)` as text | | Timestamps, npub, last line, secondary text | `--ink-muted` `#9a9a9a` | INK_MUTED `9a9a9a` |
| `text`, `white`, `neutral100` | `#fff` | Body | `--ink` `#f2f2f2` | INK `f2f2f2` |
| `cyan400`, `cyan100` | `#22d3ee`, `#cffafe` | Names, emphasis, glyph on a fill | `--ink` | INK_BRIGHT `ffffff` |
| `tint` | `#5BC6E0` | Active state, header actions | `--line-bright` `#fff` | LINE_BRIGHT `ffffff` |
| Links (`cyan500`) | | Links, mentions | ink, underlined | `nt_skin().link` = INK, underlined |
| Primary fill `cyan500` | | Primary button | `.btn` primary `#c8c8c8`, black text | PRIMARY `c8c8c8`, PRIMARY_LIT `e0e0e0`, ON_PRIMARY `000000` |
| Round send `cyan600` | | Send | white disc, black arrow | `NativeButton(.action)` as today |
| `#1C1C1D` AI row | | AI message | none: AI prose is unframed | none |
| `error`, `errorBackground` | `#C03403`, `#F2D6CD` | Error | `k_notice` and words | `notice` frame role and words |
| `#00ffff` wordmark glow | | Glow | dropped | dropped |
| Backdrop | city image | Login backdrop | ARWES grid `#070707` lines, `#181818` dots, 40 px | GRID_LINE `070707`, GRID_DOT `181818`, 40-pt tile |

Measures come from the native skin, not from Arcade: hairline 1, hit target
44, header 44, gutter 20, gap 12, labels 11 pt uppercase at 0.18 em,
buttons 44 high with 18 padding, icon buttons 32 with 16-pt icons, a field is
one 44-high underline (04 §1.4).

### 4.2 Component mapping

From 03 B3, with C2 and C4 applied. **E** existing kit component, **X**
extend, **N** new in `components/kit`. Native column names the Phase 0 item.
Existing Swift types below identify compatibility views. Under C13, new
permanent visual/motion implementations are shared GPU components, with
platform services and text/semantic adapters; the table does not require a
second Swift/Android widget implementation.

| Arcade component | Kit | Cat. | Native |
| --- | --- | --- | --- |
| Text presets | `k_text`, skin type scale | E | UILabel |
| Primary and ghost buttons | `k_button` | E | `NativeButton` PRIMARY / DEFAULT |
| Text link button | `k_link`, `k_quiet` | E | `NativeButton` QUIET |
| Round icon button, FAB | `k_icon_button` | E / X | `NativeButton` ICON |
| Text field, secret field with reveal | `k_field`, `k_input`, `k_secret` | E / X | `NativeField`; reveal is X |
| Switch | `k_switch`, `k_switch_row` | E | K4 |
| Checkbox, radio (invite picker) | `k_choice`, `k_choices` | E | `NativeButton` ROW with a check symbol |
| List item, section box | `k_row`, `k_set_group`, `k_set_row` | E | K3 on `NativeFrameView` row and panel roles |
| Card, group tile | `k_card`, `k_entry` | E | K1 plus `NativeFrameView` |
| Header | `k_header` | E | existing toolbar row |
| Own message | `k_user` | E | `NativeMessage` (bubble frame) |
| Others' message | `k_message` with author | X | K7 |
| Avatar | `k_avatar` | N | K2 |
| Message text, mentions | `k_inline`, `k_agent_text` | E | `NativeRichText` |
| Message image | `k_image` | E | K1 |
| Video | `k_video` | N | deferred |
| Link preview | `k_preview` | N | deferred |
| Reply quote in a message | `k_inline` quote | X | `NativeRichText` quote stripe |
| Reply bar above composer | `k_composer_begin` reply slot | X | K9 |
| Composer, attachment strip | `k_composer_begin/end`, `k_file` | E / X | `NativeComposer`, K10 |
| Swipe to reply with ring | `k_swipe` | N | K6 |
| Spinner | `k_loading`, `k_glyph` | E | `NativeRingView` |
| Long-press menu | `k_menu` | E | K5 on `NativeMenu` |
| Bottom sheet | `k_pane` or a dialog | X | existing `role=dialog` sheet |
| Confirm | `k_confirm` | N | K8 |
| Drawer | `k_side` | E | `NativeWorkspace` |
| Tab bar | none (D7) | — | none |
| Inbox, contact, member rows | `k_row`, `k_row_with` with avatar | X | K3 |
| Empty state | `k_notice` | E | `notice` |
| Copy | clipboard pattern | E | `clipboard.write` |
| Profile banner | `k_image` | E | K1 |
| Changelog | `k_document` | E | — (merged into About) |
| Frame (drawn) | skin frames | E | `NativeFrameView` (C2) |

Every new kit component (`k_avatar`, `k_video` later, `k_preview` later,
`k_swipe`, `k_confirm`) goes into `components/kit` with its web rendering in
the mono skin, so `/components` shows it and the web gets it too
(`docs/reuse.md`).

### 4.3 Signature elements

The table records production presentation choices. C12 and audit 06 supersede
any inference that “Drop” removes an animation from the library: every source
animation remains required in the reusable gallery, including glow, typing,
custom spinner and popup blur. C13 replaces permanent per-platform painting.

| Element | Call |
| --- | --- |
| Corner-bracket frame | Keep in gray through shared frame geometry; `NativeFrameView` is the current compatibility view |
| Frame enter animation | Preserve A01's 200 ms source specimen; the production skin can retain its 0.55 s enter and 0.035 s stagger, with reduced motion |
| Frame press highlight | Shared animated highlight (A02); preserve 200 ms source specimen, skin supplies production gray values |
| Glow (wordmark, spinner, gradient, spotlight) | Optional on product screens; retain the full reusable effect and motion specimens (06) |
| City background | Drop (D15) |
| Faerie persona and image | Drop; the agent uses the OpenAgents mark (D13) |
| Typing GIF | Keep the existing waiting state in product chat; A23's animated-image playback remains required in the gallery |
| Swipe to reply with donut | Keep, in gray (K6) |
| Long-press blur menu | Shared lifted popup and background blur (A18); `NativeMenu` is the compatibility path and system-owned menus remain platform services |
| Translucent cyan glass | Flat grays (4.1) |
| Square bubbles with a timestamp | Keep the frame idea: main already frames the person's message (C3); D11 |
| Space Grotesk, Protomolecule | Replace (4.4) |
| Cyan "A" app icon | Do not port |

### 4.4 Fonts and icons

- **Fonts:** DM Sans for body (16/23), GeistMono for labels and meta
  (11 pt uppercase), system monospace for code (04 §1.4). Arcade's scale maps
  as 03 B4 gives: `xxl` → h1, `xl` and `lg` → h2, `md` → 16 semibold toolbar
  title, `sm` → body, `xs` → 14 muted, `xxs` → 11 mono uppercase meta (the
  skin's label size rather than 03's 12). Dynamic Type through `UIFontMetrics`
  as `NativeTypography` does now.
- **Icons:** SF Symbols on native through `NativeSkin.symbol`, Apps SDK
  `IC_*` names on web through `k_icon`, with the 03 B4 mapping table. Additions
  `NativeSkin.symbol` needs: `arrowshape.turn.up.left` (reply),
  `person.badge.plus`, `person.badge.minus`, `person.2`, `lock`, `photo.badge.plus`,
  `paperclip`, `globe`, `server.rack` (relays), `checkmark.circle`. Drop
  VenetianMask (row 34) and the Axe decoration. Brand links use
  `IC_EXTERNAL_LINK` with a text label.

### 4.5 The non-grayscale colours already in the app

03 B5.2 asked for one change: send disc to white, links to ink underlined,
refresh states to distinct shapes. **That change is on main** (C1, commit
`569f03794e`). What remains:

1. Fix `coax/native/composer.md` line 50, which still names
   `primary #346bf1`; the code uses the ACTION button (04 §1.4, risk 12).
   Issue P0-10.
2. Keep it fixed: `coax/native/stock-style.coax` already fails system
   colours and host literals; every Phase 0 control passes it.

### 4.6 Copy

Use 03 A5's copy as the source, fix A7's errors ("joinded", "en error", "You
have add", "not it compatible", "Create a offer", "DM's") rather than copying
them, say "Log in" everywhere, keep visible words within
`home/mobile-copy-checks.coax`'s budget (146 on home), and use no system
words (Nostr kind numbers, "relay-signed") in screens. OpenAgents speaks as
"we"; the agent speaks as "I".

---

## 5. Phases

Each phase ends with something a person can use on a phone. Issues are
sized for one agent each. "Headless" means checkable with `coax test`, the
runtime oracle (`coax/native/tests/runtime-oracle.cjs`) or an in-process
relay (`nostr-lab-relays.coax`) with no device. "Device" means TestFlight
or the simulator with a real relay.

Owner abbreviations (section 6): **NO** Nostr owner, **iOS** the iOS thread,
**PA** port agent.

### Phase 0: native foundation (one TestFlight build)

**First: the component and motion foundation.** These are additional planned
work packages, not filed issues. Follow the full requirements in
`components/rendering.md`; none is proven by this documentation change.

| Work package | Scope | Done when |
| --- | --- | --- |
| P0-R0 | Versioned surface/motion/resource/semantic contracts and source fixtures | All A01–A28 mapped, source defaults/bugs recorded, new shapes have `proto/` owners before implementation |
| P0-R1 | Extend shared paint/backend and ARWES motion | Arcs/images, transform/clip/opacity groups, gradients/blur, springs, gestures and cancellation have numeric/render evidence |
| P0-R2 | One animated component gallery on Metal, Android Vulkan and WebGPU | Every A row works, including dormant effects; grayscale/reduced-motion variants and per-platform pictures; no per-app widget copy |
| P0-R3 | Native text and semantic bridges | Real iOS/Android IME, code AutoFill, selection, fallback fonts, VoiceOver/TalkBack, focus and bounded realization pass |
| P0-R4 | DOM GPU islands and three scene views | HTML text plus GPU components, three independent worlds, clipping/input/audio/budget/lifecycle/device-loss checks; authored-realm sandbox intact |

The app migration is incremental after these capabilities qualify. Keep
current UIKit fixes shipping while this work proceeds. Nostr service work
can progress independently of the component migration.

Ends with: a TestFlight build whose Settings shows "Your Nostr key" with the
phone's npub, and a hidden diagnostics row that connects to
relay.openagents.com, authenticates (NIP-42) and round-trips a signed event.
The rendering foundation must also pass its actual-host gates before the
build selects GPU components by default.
Blocked by D1, D2, D3, D4, D5, D6, D7, D8.

| Issue | Scope | Done when | Owner | Depends on |
| --- | --- | --- | --- | --- |
| P0-1 Relay sockets in NativeHost | Add N1, lifted from `PageHost.m` (4 slots, 8 MiB, `wss` only) with the D4 host rule; inbound frames as events. | XCTest opens a local TLS relay, sends REQ, gets EOSE; the module sees `socket.message`; a non-`wss` URL is refused. | iOS (NO reviews) | D4 |
| P0-2 Native store ops | Add N2 over the `phStorage` layout with an account namespace and `store.wipe`. | XCTest writes, lists, wipes; after wipe the namespace is empty and files were 0600. | iOS | — |
| P0-3 Signing ops | Add N6 over `NostrKey` and `oa_nostr_sign`. | The oracle signs an event id; `oa_nostr_verify` and `event_verify_fast.coax` both accept it; no secret byte crosses into JSC (checked by reading the op code). | NO | D1 |
| P0-4 ECDH and NIP-44 ops | Turn on `ENABLE_MODULE_ECDH` (`project.yml` line 88); compile `nip44.coax` to C; add N3. | The NIP-44 v2 test vectors pass through the op; a message encrypted by the op opens with `nip44.coax` in Coax and back. | NO | D1 |
| P0-5 Gift-wrap keys and random | Add N4 and N5. | A NIP-17 message built by `nip17_send` with these ops opens with `nip17_open` on another key; the ephemeral secret is zeroed after the call. | NO | P0-3, P0-4 |
| P0-6 Nostr login ops | Add N8 to the op table; add N7 if D3 allows import. | The phone logs in with its key against the local API; `linkNostr` links it to an email account; import (if in scope) replaces the key after Face ID and a confirm. | iOS | D3 |
| P0-7 Clock and photo pick | Answer 04 open question 10 and add N9 only if needed; add N10. | The module stamps `created_at` from the wall clock; a picked photo returns bytes and a type to the module. | iOS | — |
| P0-8 Image and avatar components | K1, K2 with D8 fallback on the shared paint/resource path; native decode/cache/input effects only. | 50 avatars and 5 images cache and draw on the shared gallery's targets; failure/loading semantics and resource caps pass. | PA (kit) + iOS (host) | D8, P0-R1 |
| P0-9 Rows, switch, menu, swipe, confirm, reply bar | K3 to K9 and K11 with shared motion/paint; native text/semantic/haptic adapters. | Same component specimens and every relevant A row pass on Metal/Vulkan/WebGPU, with actual mobile editing/accessibility and style checks. | PA (kit) + iOS (host) | D7, P0-R2, P0-R3 |
| P0-10 Fix the stale send colour line | Edit `coax/native/composer.md` line 50 to name the ACTION button. | The doc names no non-gray colour. | PA | — |
| P0-11 Contract and release | Collect qualified P0 rendering/native capabilities in one runtime fingerprint; runtime oracle and actual-host checks; TestFlight upload and verification record. | The embedded package is current, a compatible Update applies/rolls back, old runtime feeds still work, and no unqualified renderer is made default. | iOS | all above, P0-R0 to P0-R4 for the full rendering target |

Headless: P0-3 to P0-5 vectors (Coax side), kit components on `/components`,
P0-10. Device or simulator: P0-1, P0-2, P0-6 to P0-9, P0-11.

### Phase 1: your Nostr identity, profile and relays (Update)

Ends with: Settings → Your profile shows your npub, name and picture; you
can edit and publish them; Settings → Relays shows relay.openagents.com and
lets you add a `wss` relay; Log out wipes all Nostr data.

| Issue | Scope | Done when | Owner | Depends on |
| --- | --- | --- | --- | --- |
| P1-1 Client event store module | G2 in `coax/lib/nostr`: verified-only inserts, replaceable and addressable rules, per-account namespace, cap, wipe. | `coax test` passes; an unverified event is refused; a newer kind 0 replaces an older one; wipe empties. | NO | P0-2 |
| P1-2 Kind 0 writer | G13. | Writes NIP-24 fields including `banner` and `lud16`; reads `username` as a fallback; never writes `other`. | NO | — |
| P1-3 Relay list writers | G15 for 10002 and 10050, read-before-write. | A list with unknown tags survives a rewrite; nothing is written before the newest copy is read or the read timed out with a visible message. | NO | P1-1 |
| P1-4 Nostr session in the app | A shared module (`apps/shared/nostr-app.coax`, beside `environment-app.coax`) that holds the relay pool (`relay.coax`, `outbox.coax`) over N1, NIP-42 auth, the store over N2, and the read-at-login order of row 7. | Headless against the in-process relay: login reads 10002, 10050, 3, 30000, 10009, 10000 before any write. | PA | P1-1, P1-3 |
| P1-5 Your profile and edit | Rows 3, 5, 23, 24: screen, avatar and banner pick (N10) and upload (D5). | On a phone: edit name and picture, publish, see it on another client. | PA | P1-2, P1-4, D5 |
| P1-6 Relays screen | Rows 46, 47: list, add, remove, at least one; publishes 10002 and 10050. | On a phone: add a relay; it appears in the published 10002. | PA | P1-3, P1-4 |
| P1-7 Nostr key screen | npub with copy, "Use an existing key" if D3 allows, link state. | On a phone: the npub matches the account's linked key. | PA | P0-6, D3 |
| P1-8 Log out wipe | Row 10: confirm (K8), `store.wipe`, clear module state. | Test: log out, log in as a second account, find no event, draft or group of the first. | PA | P0-2 |

### Phase 2: contacts and direct messages (Update)

Ends with: you can find a person by npub or NIP-05, follow them (publicly or
privately), open their profile, and exchange end-to-end encrypted messages
that appear in the drawer next to agent chats.

| Issue | Scope | Done when | Owner | Depends on |
| --- | --- | --- | --- | --- |
| P2-1 Kind 3 writer | G16. | A kind 3 with relay hints and petnames keeps them after a follow. | NO | — |
| P2-2 Client search filter | G6: `search` in `NostrFilter`. | A REQ with `search` round-trips against relay.openagents.com's NIP-50 index. | NO | — |
| P2-3 Contacts screens | Rows 16 to 20, 22: list, add by npub, hex or NIP-05, suggestions from follows of follows and search, follow, private follow (kind 30000 via N3), profile view. | On a phone: follow privately; the person does not appear in your kind 3. | PA | P2-1, P2-2 |
| P2-4 DM conversation | Rows 33, 35, 36, 45: NIP-17 send and open over N3 to N5; 10050 required (`nip17_plan`); reply tags; mentions; outbox states; paging of about 100 messages. | Two phones exchange messages; a recipient with no 10050 gets a clear "not ready" state, nothing is sent. Headless: two keys against the in-process relay. | PA | P0-5, P1-4 |
| P2-5 One inbox in the drawer | Rows 11 to 15, 42: agent chats and Nostr conversations in "Recent chats", newest first, unread counts, avatars; Plus menu items; agent chat with the OpenAgents mark. | On a phone: a new DM moves its row to the top; the agent chat still opens as before. | PA, with iOS on `home/navigation.coax` | P2-4 |
| P2-6 Message actions | Rows 35, 63: swipe to reply, long-press menu (copy, reply, mute via 10000). | On a phone: swipe a message, the reply bar shows; a muted person's messages are hidden. | PA | P0-9 |
| P2-7 Old NIP-04 threads, read only | Only if D14 says yes: show kind 4 history with a "sent with an older format" note. | An old kind 4 between two test keys shows in the conversation, read only. | PA | P0-4, D14 |

### Phase 3: private groups up to 10 (Update)

Ends with: you can start a group with up to 10 people, end-to-end encrypted,
name it, and add or remove a person (which starts the next room).

| Issue | Scope | Done when | Owner | Depends on |
| --- | --- | --- | --- | --- |
| P3-1 Room extras | G3 part one: reactions, deletes and edits inside a room. | `coax test` passes on each with two recipients. | NO | — |
| P3-2 Create and invite | Rows 30, 31: pick up to 10 contacts (`k_choices`), subject (`nip17_subject`), new room on membership change shown in sequence. | On three phones: create, post, add a fourth; the new room follows the old one under the same name. | PA | P2-4 |
| P3-3 Members screen | Row 32 from the room's `p` tags. | Members match the `p` tags of the latest message. | PA | P3-2 |

### Phase 4: public and large groups on relay.openagents.com (Update, plus a relay deploy)

Ends with: you can find public groups, join one, read and post, and create
your own; new accounts start in the default groups. The app says plainly
that these groups are not end-to-end encrypted.

| Issue | Scope | Done when | Owner | Depends on |
| --- | --- | --- | --- | --- |
| P4-1 Wire and advertise NIP-29 on the relay | G14: add 29 to `supported_nips`, finish "until each is wired" (#13382). Deploy goes through the coordinator. | relay.openagents.com's NIP-11 lists 29; Nostr Lab's group card passes against it. | NO | D9 |
| P4-2 NIP-29 client module | G1. | Headless against the in-process relay: create, join, post kind 9 with `h` and `previous`, leave, read 39000 to 39003 signed by the relay's `self` key. | NO | P4-1 |
| P4-3 Joined-groups list | G15 for 10009. | Joining on one phone shows the group on another. | NO | P1-3 |
| P4-4 Group conversation | Rows 25, 28: read and post kind 9, `q` replies, paging, join and leave, "not end-to-end encrypted" notice. | On a phone: join, post, leave; the group leaves your 10009. | PA | P4-2, P4-3 |
| P4-5 Discover and create | Rows 26, 27, 60: kind 39000 list, groups your follows joined, create-group. | On a phone: create a group; it appears in discovery on another phone. | PA | P4-2 |
| P4-6 Group settings and moderation | Row 29: name, picture, about, invites (9009 `code`), remove a member, delete a message (G5). | An admin removes a member and the relay refuses that member's next post. | PA (+ NO for G5) | P4-2 |
| P4-7 Default groups | Row 8: the D9 groups created on relay.openagents.com and joined at first use. | A new account's 10009 holds them. | PA | P4-5, D9 |
| P4-8 Old channel history | G9 and a read-only view, only if D9 keeps it. | The three Arcade channel ids show their history read only. | NO + PA | D9 |

### Phase 5: images and files (Update; a build only if D5 needs a new path)

Ends with: you can send a photo in a DM, group or room; private ones are
encrypted before upload.

| Issue | Scope | Done when | Owner | Depends on |
| --- | --- | --- | --- | --- |
| P5-1 Media metadata | G12: `imeta` module; review `nipb7.coax` and `nip94.coax` for the D5 path. | Round-trip of an `imeta` tag with hash, size and type. | NO | D5 |
| P5-2 Encrypted files in rooms | G3 part two: kind 15 with AES-GCM (`coax/lib/crypto/aes_gcm.coax`). | A kind 15 file opens on the recipient and fails on anyone else. | NO | — |
| P5-3 Attach and show | Rows 37, 40: K10 attachment row, upload per D5, images inline on K1. | On a phone: send a photo in a DM; the server stores only ciphertext. | PA | P5-1, P5-2 |

### Phase 6: later, each behind its decision

Not ordered; each is its own issue when its decision says go: push (D6, N12,
a relay-watching push service, a notification service extension); NIP-49 key
export (G7, row 9); listings (D12, G4); NIP-05 names (D16); old Arcade data
adapter (D14, G10); video (row 38); link previews from a server (row 39);
zaps (needs wallet send).

### Dependency summary

```
D1-D8 ──────> Phase 0 build ──> Phase 1 ──> Phase 2 ──> Phase 3
                                     │            └──────> Phase 5 (D5)
                                     └── D9 ──> Phase 4 (relay deploy P4-1)
```

---

## 6. Ownership and coordination

| Work | Owner | Why |
| --- | --- | --- |
| Everything in `coax/lib/nostr/`, `nostr/relay/` and `nostr/` (G1 to G16, P4-1) | Nostr owner | Workspace rule since 2026-11-12 (04 §6, 02 header) |
| The Coax and C side of N3 to N6 (cipher compiled to C, op contract) | Nostr owner, if D1 says so | Keys and crypto are Nostr custody |
| `apps/shared/native/`, `apps/openagents-ios/` Swift, `project.yml`, Info.plist, entitlements, TestFlight builds and Updates | iOS thread | It is building Projects and Usage now and owns the release (04 §7 risk 9) |
| `components/kit` additions and their native controls' Coax side | Port agent, reviewed by the iOS thread | New UI goes into the kit (`docs/reuse.md`) |
| Screens: `apps/shared/nostr-app.coax` and its pages in `apps/openagents-ios/home/` | Port agent | The port's own work |
| Relay and API deploys | The coordinator, serially | Subagents never deploy |

Agree before starting Phase 0:

1. **The op contract**: the exact names, arguments, events and error codes
   of N1 to N11, written down once (in `coax/native/` beside the other
   service contracts) by the Nostr owner and the iOS thread together.
2. **The release slot**: which TestFlight build carries the fingerprint
   change (D2), and that the iOS thread's own native changes land in the
   same build or after it, not across it.
3. **The store layout**: namespace per account, key scheme for events, caps,
   and what `store.wipe` removes.
4. **Where the screens live**: `apps/shared/nostr-app.coax` (so the Android
   host can reuse it later) and how it plugs into `home/navigation.coax`'s
   drawer, which the iOS thread also edits.
5. **The copy budget**: how Nostr rows fit `home/mobile-copy-checks.coax`.

---

## 7. Decisions for the owner

One list, merged from 01 §15, 02 B14 and open questions, 03 B5 and open
questions, and 04 open questions and risks. Agents act on the
recommendation unless the owner says otherwise, except where a decision
needs the owner's yes (marked).

### Blocking Phase 0

**D1. Who builds the native Nostr operations?** (04 Q1, risk 10)
Options: (a) the Nostr owner builds the op contract and its Coax/C side, the
iOS thread lands the Swift and ships; (b) the iOS thread builds all of it;
(c) the port agent builds it with Nostr-owner review.
Recommendation: (a). Reason: keys and NIP-44 are Nostr custody, and the
release is the iOS thread's. Blocks: all of Phase 0.

**D2. A dedicated TestFlight build, or fold the ops into the next one?** (04 Q2)
Options: (a) one dedicated build with the full Phase 0 set; (b) fold into the
iOS thread's next build.
Recommendation: (a). Reason: one fingerprint change, tested alone; a folded
build mixes two threads' risk and the next build's date is not ours. Blocks:
P0-11.

**D3. Identity.** (02 B14.5, 02 Q7; 03 B5.7; 04 Q5, risk 8; 01 row 9)

**Subsequent owner decision:** account-managed identity keys are approved and
deployed (`docs/protocol/account-identity-v1.md`). The phone-first options
below predate that decision. Do not ask the owner to choose custody again or
silently mint/link a replacement canonical identity. Reconcile messaging's
sign/encrypt/import contracts with managed and existing independently linked
identities; the current managed-identity API does not expose private keys or
arbitrary signing. That protocol work is separate from the rendering choice.

- (a) Key: keep only the phone's `nsec-v1`, or also allow importing an nsec
  or hex key (Arcade's "Enter access key")?
- (b) Multiple identities per phone?
- (c) Link to the email account: link the phone key at first use and show
  one npub?
- (d) Log out: does the Keychain key stay?
- (e) Backup of a phone-made key.
Recommendation: (a) allow import, which replaces the phone key after Face ID
and a confirm that says the old key is gone unless backed up; offer NIP-46
later for keys kept elsewhere. (b) No, one identity in v1. (c) Yes,
`linkNostr` at first use. (d) The key stays (it is the device's identity and
survives reinstall today); everything else is wiped; a separate "Remove this
key" action with a confirm deletes it. (e) NIP-49 export once that module is
approved; no plain nsec screen before then. Reason: one npub everywhere and no
secret in script memory. Blocks: whether N7 is in the build (P0-6).

**D4. Default relays and user-added relays.** (02 B14.9, 04 Q4, risk 5)
Options: (a) relay.openagents.com only; (b) relay.openagents.com by default,
users may add any `wss` relay; (c) a fixed allowlist.
Recommendation: (b), `wss` only, user-added relays explicit. Reason: an
OpenAgents default avoids leaking IP and interests to third parties by
default, and Nostr is useless without other relays. Blocks: N1's host rule.

**D5. Media upload path.** (02 Q6, B13.12; 01 §12.1)
Options: (a) a Blossom server we run (`nipb7.coax` exists); (b) our API's
signed upload to the private bucket (`coax/web/api/drops.c`
`drops_open_upload`, and `documents.upload`, which already PUTs only to
`https://storage.googleapis.com/`), with public read URLs; (c) a third-party
Blossom server.
Recommendation: (b) for v1, described with `imeta`, because it needs no new
native network permission and stays on our account; decide Blossom interop
later. Private media is encrypted before upload either way. Reason: reuse.
Blocks: N11 and P1-5, P5-3.

**D6. Push notifications in v1?** (01 Q4, Q8; 02 B14.8; 04 Q6, risk 6)
Options: (a) no push in v1, say so; (b) push in v1 (production APNs
entitlement, registration, a relay-watching push service, a notification
service extension).
Recommendation: (a). Reason: it is a server service and an extension of its
own; Arcade's iOS users likely never had push either (01 Q8). Blocks: whether N12 is
in the build.

**D7. Drawer or tab bar?** (03 B5.4 versus 04 §5, risk 7)
Options: (a) the existing drawer, with Nostr conversations in "Recent chats";
(b) three tabs as Arcade had.
Recommendation: (a). Reason: the app is drawer-based and has no tab control;
tabs need a new native control and a build for no new capability. Blocks: K
list in Phase 0.

**D8. Profile picture fallback.** (03 Q6)
Options: initials on a RAISED disc; the OpenAgents mark; a blank ring; a
generated pattern from the pubkey.
Recommendation: initials (first letter of the name, else the first character
after `npub1`) in INK_MUTED on RAISED with a LINE_DIM ring; the OpenAgents
mark only for the agent. Reason: grayscale, readable, and does not make
people look like the agent. Blocks: K2.

### Blocking later phases

**D9. Public channels: NIP-28 as Arcade had, or NIP-29?** (04 Q3; 02 B5,
B14.2, B14.3; 01 row 8)
Options: (a) NIP-29 open groups on relay.openagents.com, NIP-28 never
written; (b) NIP-28 channels.
Recommendation: (a), and enable NIP-29 on the relay. Default groups: carry
"The Lobby" and a feedback group as NIP-29 groups; drop "Global Trade"
unless listings return. Show the three old Arcade channels read only only if
their history still exists on public relays (01 Q3, 02 Q1 are open).
Reason: NIP-28 is `unrecommended` and our library refuses to sign it.
Blocks: Phase 4.

**D10. Group chat design.** (02 B4, B14.1)
Options: (a) NIP-17 rooms up to 10 members, NIP-29 on our relay for larger
and public groups, Marmot (MLS) only if end-to-end encryption above 10 is
required; (b) Marmot for all private groups; (c) NIP-29 for everything.
Recommendation: (a). Reason: NIP-17 is end to end and already built; NIP-29
gives real membership and moderation; Marmot is a large build (MLS, HPKE,
X25519, AES-128-GCM). **Needs the owner's yes** if end-to-end encryption
above 10 people is required, because that adds G11. Blocks: Phases 3 and 4.

**D11. Framed or rounded message bubbles?** (03 B5.1, Q5; C3)
Options: (a) keep main's framed bubble (the skin's dim bubble frame) for the
person's own message and frame others' messages the same way; (b) rounded
bubbles.
Recommendation: (a). Reason: main already frames, it matches `/chat` and
Arcade's square framed bubbles. Shared GPU components take the frame and its
motion from the skin; the Swift compatibility path keeps its existing style
gate. This does not remove any source-parity animation specimen. Blocks: K7.

**D12. Marketplace and listings?** (01 rows 56 to 59; 02 B14.6; 03 B5.8; 04 Q9)
Options: (a) defer; when it returns, NIP-99 listings priced in BTC or sats,
offers over NIP-17, payment Bitcoin and Lightning only; (b) drop for good.
Recommendation: (a). Reason: it never shipped in Arcade, and paying needs
wallet send, which is compiled out of the native build. Blocks: Phase 6
listings only.

**D13. The AI persona.** (01 Q9; 03 B5.5; 04 Q9)
Options: (a) drop Faerie; Arcade's AI chat is the existing account chat with
the OpenAgents mark; (b) keep a separate persona.
Recommendation: (a). Reason: one agent identity and voice already exist.
Blocks: P2-5.

**D14. Reading old Arcade data.** (02 B14.4, B15, Q2; 01 Q6)
Options for each: keys (import, D3); kind 0 and kind 3 (readable as is);
NIP-04 DMs (read only with `nip04.coax`); blinded DMs, encrypted channels
(99, 400 to 403), 10003 and 30040 (a legacy decrypt adapter, G10).
Recommendation: keys, kind 0, kind 3 and NIP-04 yes; no adapter for Arcade's
own ciphers unless the owner names people who still need that data. Reason:
most channel keys were never recoverable off the creating device (02 A5),
and the adapter keeps unauthenticated ciphers alive. Blocks: P2-7, G10.

**D15. Title screen and city art.** (03 B5.6)
Recommendation: no city image; one "Log in" flow with the Nostr key option
second. Blocks: nothing (Phase 1 copy).

**D16. NIP-05 names under an OpenAgents domain.** (01 row 4, Q2)
Options: (a) defer; (b) serve `nostr.json` for opted-in names.
Recommendation: (a); do not migrate `@arcade.chat` names. Blocks: Phase 6.

**D17. Reputation.** (02 B14.7; 03 B5.9)
Recommendation: drop for v1, including the "Rep.: 90%" placeholder. Blocks:
nothing.

**D18. Virtualized list.** (04 Q8, risk 3)
Options: (a) page in Coax (about 100 messages per view) in v1; (b) a keyed
virtual list in `apps/shared/native` in the Phase 0 build.
Recommendation: (a), and measure busy NIP-29 groups in Phase 4 before
deciding on a second build. Reason: paging is how history already works
within the 4096-node cap. Blocks: nothing in Phase 0 under (a).
The new GPU runtime must additionally realize/layout only bounded visible
rows and preserve text/semantic identity (P0-R3); paging is not permission
to reshape and repaint the whole loaded history on each animation frame.

### Settled by main or by this plan (from the source lists)

- 03 B5.2 (non-gray colours): done on main (C1); P0-10 fixes the doc line.
- 03 B5.3 (projection roles and their order): replaced by section 3.2's K
  list.
- 03 B5.10 (haptics): inside K5 and K6, no op.
- 03 B5.11 (copy fixes): section 4.6.
- 03 Q4 (Geist files): answered (C5).
- 02 B14.2 (enable NIP-29 on the relay): folded into D9 and P4-1.
- 02 B14.8 (no push event on Nostr): rule; folded into D6.
- 04 Q7 (does `components/core/media.coax` project natively): open; P0-8
  checks it before adding K1 and uses it if it fits.
- 04 Q10 (wall clock): open; P0-7.
- 01 Q1, Q3, Q5 (is `api.arcade.chat`, Arcade's relays, `top.json` still
  live): open; they matter only to D9's read-only history and D14.

---

## 8. Risks and what is not in this plan

| # | Risk | Mitigation |
| --- | --- | --- |
| R1 | The Phase 0 build slips behind the iOS thread's releases, or its fingerprint change collides with theirs. | D2 dedicated build; agree the release slot first (section 6); rebase on main before each change (04 risk 9). |
| R2 | A secret leaks into JSC through a convenient op. | Ops take ids and public keys only; P0-3 and P0-5 done-when include reading the op code for secret bytes; the ephemeral key is generated and destroyed natively. |
| R3 | Busy groups break the 4096-node cap or the 1 MiB inbox. | Page in Coax (D18); cap the store's history; measure in Phase 4 before any virtual list. |
| R4 | Relay connections leak the user's IP and interests. | D4: our relay by default, others only when the user adds them, `wss` only. |
| R5 | Users expect NIP-29 groups to be private like NIP-17 rooms. | Say "not end-to-end encrypted" on every NIP-29 group and its create screen (P4-4); keep private groups on NIP-17. |
| R6 | Relays forge or replay events. | Verify every event before storing (1.2); newest-wins only after verification; NIP-29 metadata only from the relay's `self` key. |
| R7 | A rewrite of a list drops another client's data. | Read before write and keep unknown tags (G15, G16, P1-3 done-when). |
| R8 | Without push, DMs arrive only while the app is open. | State it in the first version (D6); live subscriptions while open. |
| R9 | Two Nostr identities (the account's linked keys and the phone key) confuse users. | D3: link at first use, show one npub. |
| R10 | Nostr library work and the port collide in `coax/lib/nostr`. | The port agent never edits `coax/lib/nostr`; it files G items to the Nostr owner and builds against their interfaces. |
| R11 | Stock UIKit controls slip in (UISwitch, alerts, blur) and fail the style check late. | Every K item is drawn in `NativeChrome.swift`; run `coax/native/check.sh` in each issue. |
| R12 | Phase 4 needs a relay change and a deploy outside the app's release. | P4-1 first in Phase 4; the coordinator deploys; Phase 2 and 3 do not depend on it. |

Not in this plan:

- Zaps and any payment from the app (needs wallet send and a spend-authority review).
- Marmot / MLS groups (G11), unless D10 changes.
- An Android port of the screens (the shared module makes it possible later).
- Moving project member chat or agent chat onto Nostr.
- A kind 1 feed or notes (Arcade had none at HEAD).
- Reputation, ArcTrust and NIP-32 ratings.
- Voice input, video playback, link previews, QR codes.
- Running or reading `api.arcade.chat`, the NIP-05 Lambda or Arcade's relays.

---

## 9. Flagged outside the port: the committed Breez key

04 §1.7 found an API key certificate written as a literal in
`apps/openagents-ios/OpenAgents/Wallet/BreezKey.swift`, compiled into the app
target, in this public repository. This plan does not quote it.

What the repository already says: `AGENTS.md` line 114 calls "the committed
Breez API key" a deliberate exception, and `docs/lightning/history.md`
decision 5 records why: the Breez team confirmed it is a basic validation
key that cannot be hidden in a shipped app.

Recommendation: the owner confirms that decision still holds for the
current key and that the file holds that basic validation key and nothing
more (for example no other credential added since). If it holds, nothing
changes and 04's flag is closed by pointing at that decision. If it does
not, rotate the key with Breez first and only then move it out of source;
removing it from source without rotating leaves it in git history. This is
an owner decision; no agent changes the file on its own.

---

## Files read

- `docs/arcade/01-codebase.md`, `02-nostr.md`, `03-design-and-components.md`,
  `04-target-app.md` in full.
- `AGENTS.md`, `docs/reuse.md`, `docs/lightning/history.md` (decisions),
  `coax/nostr.md` §17 (line 609).
- Spot checks at `5bf0d987e0`: `components/skin/native-chat.coax`,
  `components/skin/mono-native.coax` (tokens, frame roles, button variants),
  `coax/native/stock-style.coax` (lines 1 to 60), `coax/native/composer.md`
  (line 50), `apps/shared/native/NativeChrome.swift` (`NativeFrameView`,
  `NativeRingView`, `NativeMenu`), `NativeComposer.swift`, `NativeMessage.swift`,
  `NativeControls.swift` (refresh states), `NativeProgram.swift` (roles),
  `NativeHost.swift` (op table, line 96; `call()` cases), `apps/openagents-ios/project.yml`
  (build 1085); listings of `coax/lib/nostr/` and `components/zeron/fonts/`;
  `git log` of the skin files.
- NIP texts at `~/work/projects/repos/nips` `1fb3a95`: `17.md` (kind 14 reply
  tag), `29.md` (9007, 9009, 9021, 9022), `C7.md` (`q`).
- Issue #13607.

## Open questions

Kept open from the audits; this plan does not answer them.

1. Do `relay.arcade.city`, `arc1.arcadelabs.co`, `api.arcade.chat`, the NIP-05
   Lambda and `top.json` still answer? (01 Q1 to Q3, Q5; 02 Q1)
2. Did anyone outside the team use Arcade's private channels or blinded DMs?
   (01 Q6, 02 Q2)
3. Can a NIP-05 fetch go through the existing `http` op under the path
   policy, or does it need its own op? (`nip05.coax` expects the host to
   fetch; the `http` op allows only `cn_api` and `nwp_account` paths.)
4. Is NIP-29 on relay.openagents.com off on purpose or waiting on #13382?
   (02 Q5)
5. Does `components/core/media.coax` project to native images? (04 Q7)
6. Can the module read the wall clock directly? (04 Q10)
7. Does PHPicker need a photo-library usage string in this app? (inferred
   not; checked in P0-7)
