# Verse chat

Verse chat follows the structure of Horse Isle 1's chat: its channels,
two chat windows, method selector, and limits. It uses existing Nostr NIPs
for transport, is drawn only in the amber ladder, and adds RuneScape-style
speech bubbles over avatars. This page records the reference, the NIP
review, and what Verse implements.

## The reference: Horse Isle 1

Horse Isle is a browser horse MMO. The details below come from the HISP
server reimplementation
([`ChatMsg.cs`](https://github.com/islehorse/HISP/blob/master/HorseIsleServer/LibHISP/Game/Chat/ChatMsg.cs),
[`Command.cs`](https://github.com/islehorse/HISP/blob/master/HorseIsleServer/LibHISP/Game/Chat/Command.cs)),
the original game strings in
[`HorseIsleData/messages.json`](https://github.com/islehorse/HorseIsleData/blob/master/gamedata/messages.json),
and the official help center's Chat articles.

### Channels

| Channel | Audience | Shortcut |
| --- | --- | --- |
| CHAT ALL | Everyone on the server | `/a` |
| ADS CHAT | Everyone who hasn't muted ads | `/$` |
| CHAT ISLAND | Players on the same isle | `/i` |
| CHAT NEAR | About one screen around you | `/n` |
| CHAT HERE | The same map tile | `/h` |
| CHAT BUDDIES | Mutual buddies | `/b` |
| Private message | One player, by name prefix | `/name` |
| Mod and admin chat | Staff | `/mod`, `/admin` |

### Selecting a channel

There were no tabs. A dropdown left of the input line listed the chat
methods and every online player. A `/` shortcut overrode it for one line.

### Layout

- Two chat windows along the bottom:
  - Left, the global window: ALL, ADS, ISLAND, and server notices.
  - Right, the personal window: HERE, BUDDIES, PMs, and socials.
- Lines read `NAME: message`, with no channel tags and no timestamps. The
  channel showed only through color.
- The sender saw an audience count on their own copy, such as
  `[4 listening]` or `(2 here)`.

### Limits and moderation

- ALL and ADS lines were capped at 150 characters. ADS allowed one post a
  minute. ALL drew on a budget that refilled over time.
- A word of five or more letters in capitals was refused: "Please do not use
  all CAPS, it looks as if you are yelling."
- `!MUTE <channel>` and `!UNMUTE <channel>` controlled what a player saw.
  `!MUTE ALL` spared ads.
- A profanity filter and an abuse report sat on top.

### Worlds

Separate servers, named after horse colors (Pinto, Roan, Palomino, and so
on), never shared chat. Isles scoped ISLAND chat; towns and areas did not.
Horse Isle 1 had no text over avatars.

## NIP review

| NIP | What it gives | Verse use |
| --- | --- | --- |
| [NIP-01](../../nips/official/01.md) | Events, filters, relay messages. | Everything. |
| [NIP-C7](../../nips/official/C7.md) | Kind `9` chat messages; a reply quotes its parent with `q`. | Every public and room line is kind `9`. |
| [NIP-29](../../nips/official/29.md) | Relay-based groups: an `h` tag on member events, relay-signed `39000` metadata, join (`9021`) and leave (`9022`), and moderation (`9000`–`9010`). The relay enforces membership; `restricted`, `private`, and `closed` flags set who may write, read, and join. Only the relay key, or an admin, may create a group. | Rooms: open groups the relay creates, readable and writable by anyone. |
| [NIP-44](../../nips/official/44.md) | Versioned payload encryption: secp256k1 ECDH, HKDF, ChaCha20, HMAC-SHA256, padding. No forward secrecy and no deniability on its own. | Inside NIP-17. |
| [NIP-59](../../nips/official/59.md) | Gift wrap: an unsigned rumor, sealed (`13`) by the author, wrapped (`1059`) under a one-time key, timestamps randomized up to two days back. | Inside NIP-17. |
| [NIP-17](../../nips/official/17.md) | Private direct messages: a kind `14` rumor, sealed and gift-wrapped to each recipient and to the sender. Relays should serve a `1059` only to the authenticated recipient (NIP-42). Up to about ten participants. | Private messages. |
| [NIP-42](../../nips/official/42.md) | Client authentication to a relay by signing a challenge (kind `22242`). | Required to read your own gift wraps on our relay. |
| [NIP-EE](../../nips/official/EE.md) | MLS end-to-end encrypted direct and group messaging, with forward secrecy and post-compromise security. Marked `unrecommended` and superseded by the Marmot protocol. | Not used. It is the candidate for private guild chat later; Marmot is where to look first. |
| [NIP-28](../../nips/official/28.md) | Public chat channels (`40`–`44`) with client-side moderation. Marked `unrecommended`; use NIP-29 instead. | Not used. |
| [NIP-MV](../../nips/openagents/NIP-MV.md) | Shared 3D worlds. Its world chat section scopes kind `9` lines by world, channel, zone, cell, and position. | ALL, ADS, ZONE, NEAR, and HERE. |

Scope does not make a line private. ZONE, NEAR, and HERE are public events
that clients filter by distance; anyone subscribed to the world can read
them. Only PMs are private.

## What Verse implements

### Channels

| Verse | Horse Isle | Wire | Window | Over the head |
| --- | --- | --- | --- | --- |
| ALL | CHAT ALL | kind `9`, `t=all` | Left | Yes |
| ADS | ADS CHAT | kind `9`, `t=ads` | Left | No |
| ZONE | CHAT ISLAND | kind `9`, `t=zone`, `z=<zone>` | Left | Yes |
| NEAR (40 m) | CHAT NEAR | kind `9`, `t=near`, `pos` | Right | Yes |
| HERE (3 m) | CHAT HERE | kind `9`, `t=here`, `pos` | Right | Yes |
| `#lounge`, `#trading-post`, `#builders` | (Horse Isle 3's clubs) | kind `9`, `h=<room>` (NIP-29) | Right | No |
| PM | Private message | NIP-17 gift wrap | Right | No |

- **Zones.** Zones are Verse's isles: the Plaza (within 60 m of the center)
  and the North, South, East, and West Wards around it.
- **Worlds.** Horse Isle's servers correspond to Verse worlds. A world is a
  world identifier on a relay; the default is `verse-plaza`.
- **Buddies.** BUDDIES is not implemented. Mutual NIP-02 follows plus NIP-17
  is the likely shape.

### Controls

- `Enter` opens the chat line and `Enter` sends it. `Esc` cancels and
  `Up` recalls the last line.
- `Tab` cycles the method selector (`[ALL]`, `[ADS]`, `[ZONE]`, `[NEAR]`,
  `[HERE]`, then the rooms, then the current PM target).
- `/` opens the line with a shortcut already typed:
  - `/a`, `/$`, `/z`, `/n`, and `/h` send to a channel.
  - `/r <room> <text>` sends to a room.
  - `/<name> <text>` sends a private message to the first player whose name
    starts with `<name>`.
  - `/<name>` alone makes that player the PM target.
- `!mute <channel>` and `!unmute <channel>` take `all`, `ads`, `zone`,
  `near`, `here`, `rooms`, `pm`, `logins`, or `gestures`. `!mute all` spares
  ads, as in Horse Isle.

### Limits

These are Horse Isle's own, with Verse numbers:

- ALL and ADS lines are at most 150 characters.
- ADS allows one post a minute.
- ALL spends from a budget of 15 that earns one back every 20 seconds.
- Other lines are capped at 500 characters.
- A five-letter word in capitals is refused.

Refusals appear as `CHAT NOT SENT:` notices.

### Layout

- Two framed windows along the bottom:
  - WORLD on the left: ALL, ADS, ZONE, and login notices.
  - PERSONAL on the right: NEAR, HERE, rooms, PMs, and greetings between
    agents.
- The input line sits below them, with the method selector at its left.
- Color is not available, so each line starts with a quarter-bright channel
  tag (`all`, `$`, `zone`, `near`, `here`, `#room`, `pm`). The speaker's
  name is full brightness and the text is three-quarter.
- Your own copy of a line carries Horse Isle's audience note, such as
  `[3 near]`, `(1 here)`, or `[4 listening]`.
- Other players carry a half-bright name tag over their heads within 60 m.
- Public lines on ALL, ZONE, NEAR, and HERE float over the speaker in a
  framed bubble for seven seconds.

### Text

Text is Fira Mono Medium (SIL Open Font License 1.1, in
`crates/verse/assets/` with its license), rasterized once into a glyph
atlas.

### The relay

`scripts/verse-relay.sh` creates the three rooms as the relay on startup
(`verse --seed-rooms`), because NIP-29 lets only the relay key create a
group. PMs need NIP-42: the client answers the relay's challenge, then
subscribes to its own gift wraps. The challenge names the relay URL, so a
player on another machine must connect with the same URL the relay
announces.

## Not yet

- BUDDIES.
- A player list with PM, mute, and profile buttons.
- Mod and admin channels, and reporting.
- A profanity filter.
- Scrolling back through chat history.
- Private guilds with NIP-EE or Marmot.
- Showing outside Nostr users (kind `1` posts) as bubbles over stand-in
  avatars.
