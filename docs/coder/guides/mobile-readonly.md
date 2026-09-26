# Read saved Codex and Claude chats on iPhone

Coder's iOS reader pairs with a computer and displays its retained Codex and
Claude Code conversations. Select a chat to load its native transcript, then
keep it open to receive appended records. The original harness continues to
own the conversation. This app cannot send a prompt, run a tool, approve an
operation, or resume an agent.

The first delivery uses the existing **Coder** iOS app identity and Xcode
setup. [Native build instructions](../../../bins/coder-ios/README.md) and the
[verification record](../verification/2026-09-26-mobile-reader.md) describe the
build and the checks actually performed.

## Connect the phone

1. Open Coder on the phone and tap **Connect**. Copy or share the displayed
   device public key. The device secret stays in its own Keychain entry; this
   does not use or transfer a Codex, Claude, or Apple account credential.
2. On the computer, build the connector from this checkout:

   ```sh
   cargo build --release -p coder-connect
   ```

3. Pair the displayed public key with the roots you want to read:

   ```sh
   cargo run --release -p coder-connect -- pair \
     --client YOUR_PHONE_PUBLIC_KEY \
     --relay wss://relay.openagents.com/ \
     --codex-root "$HOME/.codex" \
     --claude-root "$HOME/.claude" \
     --expires-secs 86400 > connection.json
   ```

   Replace `YOUR_PHONE_PUBLIC_KEY`. Omit a root to exclude that harness. Each
   selected root covers its supported retained conversations, including archived
   Codex chats and Claude subagents, and future matching files under that root.
   It does not scan account credential files. Text already recorded in a chat
   is still part of the disclosed transcript.

4. Transfer the complete `connection.json` through a trusted channel, paste it
   into **Connection code**, and tap **Connect to computer**. It binds the
   computer key, phone key, relay, source categories, and expiry. It contains
   an encrypted signed grant, not an account credential or plaintext history.
5. Keep the connector running on the computer:

   ```sh
   cargo run --release -p coder-connect -- serve
   ```

6. Return to the phone's chat list. Select a chat. The foreground reader loads
   history in bounded pages and checks for new records every five seconds.

The phone and computer need Internet access to the same compatible relay;
no inbound port, shared filesystem, or direct network route is required.
The computer must remain awake and the connector must be running for fresh
reads. A relay must support NIP-42 authentication and the private-artifact
visibility policy; arbitrary public relays are not interchangeable.

## Read and navigate

- **Earlier** and **Later** select retained transcript pages. **Latest** resumes
  the newest cached page. Long messages have **Previous part** and **Next part**
  controls so the readable text is not limited to a preview.
- **Follow new messages** follows the end while new pages arrive. Dragging the
  transcript pauses follow and pins the page that was on screen, including if
  a refresh was already in flight. Resume explicitly when ready.
- **Show exact source bytes** exposes the retained native record fragments.
  Unknown events, malformed JSON, tool data, and oversized records stay
  inspectable. A fragment that splits UTF-8 is displayed as base64, with its
  original byte range. Markdown links are inert.
- The byte counter reports received source bytes against the last observed
  file size. It is not an execution progress estimate. A partial final line is
  labeled as a record the harness is still writing.
- **Reload history** discards that chat's cached pages and starts again from
  the beginning. Use it after a changed or replaced source is reported.

The app shows the persisted native records, not text the harness never saved.
Images and attachments referenced by a record are not fetched or decoded as
media. A file missing on the computer is visible as unavailable; selecting a
metadata row cannot recreate its transcript.

## Cache, reconnect, and revocation

The device persists connection metadata, catalog pages, transcript pages, and
cursors in its protected application-support directory. Each cache entry is
NIP-44 encrypted to the device's own key and authenticated against its logical
identity. Writes are atomic. Files are excluded from device backup, and the
Keychain identity uses device-only, unlocked access. App termination can lose
an in-flight refresh but cannot promote a cursor past an unwritten page.

Cached lists open before the network answers. Connection failures keep cached
content visible with an offline label; an old cache does not establish that
access remains authorized. The client verifies the host signature, exact
request and grant, recipient, source identity, record offsets, and expiry
before accepting a response. A changed source cannot append to old content.

The cache has a 64 MiB budget and an 8,192-file bound. Older page eviction is
explicit when it affects the visible history; reload retrieves those bytes
again while access remains valid. Catalogs are bounded to 4,096 retained rows.
Complete records up to 256 KiB have a readable projection; larger records
remain available as exact paged fragments. Individual source files are bounded
to 512 MiB. See the [reader's complete limits](../../../crates/coder-history/README.md).

Use the phone's **Disconnect and erase cached chats** to forget the pairing
and erase local content while retaining the device identity. This does not
revoke the computer grant. To revoke it on the computer, use the `grant` field
from the connection code:

```sh
cargo run --release -p coder-connect -- revoke --grant GRANT_ID
```

An online client erases its cache when it receives an authenticated revocation
or expiry refusal. Local declared expiry also ends access. Revocation cannot
remotely erase an offline copy before the client learns about it, and it
cannot recall an already admitted reply. Pair again to renew an expired grant
or change the source scope; the maximum grant lifetime is 30 days.

## Implementation boundaries

- [`coder-history`](../../../crates/coder-history/README.md) opens only explicitly
  selected host roots and returns native records with verifiable cursors.
- [`coder-connect`](../../../crates/coder-connect/README.md) implements the
  [SESS retained-history observer profile](../../../nips/openagents/NIP-SESS.md#read-only-observation-of-retained-foreign-history)
  over authenticated Nostr connections and encrypted private `3188` artifacts.
- [`coder-mobile`](../../../crates/coder-mobile/src/lib.rs) owns synchronization,
  cache, typed application actions, and view projection in Rust.
- [`rust-native`](../../../crates/rust-native/README.md) supplies generic validated
  lists, text, buttons, styles, and revision-bound activation. The Coder palette
  belongs to [`coder-ui`](../../../crates/coder-ui/src/theme.rs).
- The [thin SwiftUI host](../../../bins/coder-ios/host/App/) owns native controls,
  Keychain, lifecycle, selection, and scrolling. It does not interpret chat
  records, run tools, or invent application intents.

Future writing needs a distinct admitted control path, native session adapter,
retry semantics, and approval boundaries. Observation grants do not acquire
those rights when a renderer gains a composer. The broader
[suite migration](../migration-status.md) remains a separate delivery track.
