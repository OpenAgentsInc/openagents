# The Lobby

Written 2026-11-16 against openagents `42092fc2e9`. The first version
described under [MVP](#mvp-what-is-built) is built; everything else is the
plan.

## What it is

One room everyone in the app is in. You see who is online and what each
person is doing ("playing Everglade", "building a level"), you chat, and
you tap someone to open the game they are in.

It is built on Nostr, with the OpenAgents Coax Nostr library
(`coax/lib/nostr/` in openagents) and the OpenAgents relay
(`wss://relay.openagents.com`). Every message and every "I am here" is a
signed Nostr event. The app's core writes, signs, checks and reads them
(`coax/lobby.coax`); the Swift host only holds the socket and the key.

## Identity

- **Every install has a Nostr key.** 32 random bytes made on the device and
  kept in its Keychain. The same key already signs Decide requests.
- **Logging in links that key to your OpenAgents account.** The OpenAgents
  server does not create a key with an account: it links keys an app proves
  it holds (`POST /api/auth/link/nostr`, a NIP-98 proof over a one-time
  URL). GameBuilder does that when you log in, with the label `app`. Your
  account then lists the key.
- **You can read the lobby without an account. Saying anything needs one.**
- **Names.** A name is 3 to 20 letters, digits, spaces, `_` or `-`, chosen
  in the lobby and carried in your presence. Until you choose, you are
  "Player" and four characters of your key.

What is not there yet: nobody else can check that a key in the lobby belongs
to a real account. The link is on the server, and there is no public lookup
from a key to an account. See [Trust](#trust-and-safety).

## The events

All carry the room tag `["t","gamebuilder.lobby"]` and a NIP-40
`expiration`, which the relay enforces by deleting them.

| What | Kind | Tags | Lives |
| --- | --- | --- | --- |
| Presence | 30315 (NIP-38 user status, addressable) | `d` = `gamebuilder.lobby`; `name`; `game` (the game's ID, or `own`); `doing` (`lobby`, `editing`, `playing`) | 150 seconds; sent again every 50 |
| Chat | 42 (NIP-28's channel message kind) | the room tag | 6 hours |
| Report | 1984 (NIP-56) | `p` the key reported, with a reason | Kept |

Notes on the choices:

- **Presence as NIP-38** means "online" needs no server: a person is here
  while an unexpired status of theirs exists, and gone 150 seconds after
  their last one. The newest per key replaces the last.
- **Chat is not full NIP-28.** NIP-28 names a channel by the ID of a kind 40
  event that creates it; the library has no NIP-28 and the room has no such
  event yet. The MVP uses kind 42 with the room named by a `t` tag.
  Creating the channel properly, from the owner's key, is a small follow-up
  that lets other Nostr clients see the room as a channel.
- **Chat expires.** A lobby's talk is of the moment. Six hours means no
  permanent record of what children said, and nothing to moderate
  yesterday's.
- **NIP-17 is for private messages**, not a public room. The library has it
  (with NIP-44 and NIP-59); it is the right tool for direct messages and
  invitations later, and the relay already requires AUTH to read gift
  wraps.

## The relay

`wss://relay.openagents.com`, the Coax relay in openagents (`nostr/relay/`).
What the client has to respect:

- Reading is open. **Writing needs NIP-42 AUTH**: the relay sends a
  challenge on connect and the client answers with a signed kind 22242.
- 60 events a minute per key, 120 per connection. Presence every 50 seconds
  and a chat cap of one message a second stay far under it.
- It enforces `expiration`.

## The screen

Mobile first. A button beside PLAY opens the lobby as a sheet over the
editor; it shows the number online.

1. **Who's here.** A row of people across the top: name, and what they are
   doing with the game's name. You are first.
2. **The talk.** Messages, newest at the bottom, each with its sender's
   name.
3. **Say something.** A field and a send button. Logged out, it is a "Log
   in to chat" button.
4. **Tap a person.** Open the game they are in; mute them; report them.

## Trust and safety

The first people in this room include children. The MVP has the minimum,
and the list after it is what must exist before the app is public.

In the MVP:

- **Mute.** Hides a person and everything they say, on your device.
- **Report.** Publishes a NIP-56 report and mutes them.
- **A word mask.** A short list, replaced with stars, applied to what you
  send and what you read. Names are restricted to plain characters.
- **No links.** A message with a web address is refused.
- **Short and slow.** 280 characters, one a second.
- **Nothing lasts.** Chat expires in six hours.
- **Every event is verified.** A message whose signature does not check is
  dropped.

Not in the MVP, and needed before this is public:

- **Only accounts may speak, enforced by the relay.** Today any Nostr key
  can write to the room from any client; the app only stops *its own* user
  speaking logged out. The fix is on the server: a room where the relay
  accepts writes only from keys linked to an account. NIP-29 relay groups
  are the shape; the relay has them, capped at 8 members today.
- **Someone reads the reports.** Reports are published; nobody is assigned
  to act on them, and there is no ban.
- **Mutes that follow you.** They are in memory now; NIP-51 mute lists are
  in the library.
- **A real filter.** The word list is a dozen words.
- **A parent's switch.** Chat off, or read-only, per device.
- **App Review.** A public chat needs filtering, reporting, blocking and a
  way to contact the developer (guideline 1.2). The first three exist in
  thin form.
- **A legal read** on children's chat, the same one publishing needs.

## Joining games

In the MVP, "join" means: open the game the other person is in, on your own
device. You are not in the same match. Being in the same match is the
multiplayer plan (`docs/scrappers-multiplayer.md`), and the lobby is where
it will start from: a person hosting a match shows it in their presence,
and tapping them asks to join. Invitations between two people go by NIP-17.

## MVP: what is built

- `coax/lobby.coax`: the room's state, the events above written and signed,
  NIP-42 AUTH, incoming frames verified and read, presence and its expiry,
  names, the word mask, mute, report, and the frames to send. With checks.
- `host/Sources/Lobby.swift`: the WebSocket, the device key, and the screen.
- Linking the device key to the account at login (`Account.swift`).

## Next

1. The relay-enforced accounts-only room (server work, in openagents).
2. A proper NIP-28 channel, or a NIP-29 group once its member cap is raised.
3. Mute lists that persist (NIP-51), and a person who reads reports.
4. Direct messages and game invitations by NIP-17.
5. Hosting a match from the lobby, with the multiplayer core.
6. Profiles as kind 0, so a name is the same in every Nostr client.

## Open questions

1. **Is a public room right for the first release**, given who the first
   users are? The alternative is friends-only: you see people you have
   added, by code.
2. **Who acts on reports?**
3. **Should the lobby be per game** (an Everglade room, a Scrappers room)
   as well as global?
4. **Whose relay long term?** The games are meant to live on their own
   domain; the lobby is on the OpenAgents relay today.
