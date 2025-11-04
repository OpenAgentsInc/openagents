Goal recap

You want your iOS app to:
• Show the same thread view as desktop (read from the same Codex data).
• Send messages to the desktop (which runs the OpenAI Codex CLI and manages filesystem access).
• Receive messages/updates from the desktop.
• Avoid iOS touching desktop-specific filesystem locations.

Below are practical architectural options, with trade-offs and implementation notes.

1) Local network API (desktop acts as a server)

Desktop app exposes an HTTP/WebSocket API on the local network. iOS app connects to it when both are on the same Wi‑Fi/LAN.

• How it works:
   • Desktop runs a lightweight web server (e.g., URLSession-based with SwiftNIO/Vapor, or even a simple local HTTP server).
   • REST endpoints for CRUD (threads, messages, summaries).
   • WebSocket for real-time updates (push new messages, status changes).
   • iOS discovers the desktop via:
      • Manual entry of IP:port
      • Bonjour/mDNS service discovery (recommended for zero-config)
• Pros:
   • Simple mental model. Desktop remains the source of truth and manages filesystem/CLI.
   • Real-time updates via WebSocket.
   • No external infrastructure needed.
• Cons:
   • Only works on the same network (unless you add NAT traversal/port forwarding).
   • Needs auth (at least a pairing key/token) to avoid anyone on the LAN connecting.
• Good for:
   • Development, LAN-bound workflows, privacy-focused setups.

Implementation notes:
• Use Bonjour to advertise a service like _codexd._tcp with a TXT record versioning the API.
• TLS: On LAN, you can start with HTTP + token, then move to TLS with local certificates if needed.
• WebSocket messages can carry thread diffs or just “invalidate” signals prompting the iOS app to re-fetch.

2) Cloud relay/broker (desktop and mobile both connect out)

Both apps connect to a cloud service that relays messages and syncs thread state. Desktop remains the executor/FS owner; the cloud is only a transport/state mirror.

• How it works:
   • A minimal backend (could be serverless) stores thread metadata, or at least acts as a message bus.
   • Desktop posts updates (new messages, summaries) and receives commands from iOS.
   • iOS subscribes to updates and sends commands via the same service.
• Pros:
   • Works across networks, NAT, and mobile data.
   • Better for push notifications and background updates.
• Cons:
   • Requires running a backend (cost/complexity).
   • Privacy concerns unless you encrypt end-to-end or store only minimal metadata.
• Good for:
   • Users on different networks, remote control scenarios, multi-device sync.

Implementation notes:
• Start with a simple REST + WebSocket (or Server-Sent Events) service.
• Consider end-to-end encryption for message bodies if you don’t want to trust the server.
• Use APNs for push to wake the iOS app for new desktop events.

3) Peer-to-peer via Multipeer Connectivity (Apple framework)

Direct device-to-device communication over Wi‑Fi, peer-to-peer Wi‑Fi, or Bluetooth, without a central server.

• How it works:
   • Use MultipeerConnectivity on both macOS and iOS to discover and connect.
   • Exchange messages and files directly.
• Pros:
   • No server required.
   • Works even without a shared Wi‑Fi (via peer-to-peer).
• Cons:
   • Less control over reliability and topology.
   • Limited observability/diagnostics compared to a proper server.
• Good for:
   • Ad-hoc, proximity-based control.

Implementation notes:
• Design a small protocol (e.g., JSON frames: {type, payload, messageId}) to handle commands, acks, and updates.
• Ensure backpressure and chunking for larger payloads.

4) Shared storage via iCloud Drive / CloudKit

Use iCloud Drive (document syncing) or CloudKit (structured data) to reflect thread state. Desktop writes the Codex outputs to a shared container; iOS reads it and writes commands via a command queue.

• How it works:
   • Desktop writes JSONL summaries and a “command inbox” file in an iCloud folder or CloudKit records.
   • iOS monitors changes (file coordination or CloudKit subscriptions), updates UI, and appends commands.
• Pros:
   • No custom server.
   • Works across networks.
• Cons:
   • Latency and eventual consistency; not great for real-time control.
   • CloudKit has a learning curve and quotas.
   • iCloud Drive file coordination can be finicky for near-real-time UX.
• Good for:
   • Async sync of threads; less for interactive command/control.

5) SSH/Reverse tunnel to desktop

iOS sends commands over an SSH-like channel to a desktop agent. Typically not feasible directly from iOS (no native SSH client APIs), but could be done via a custom relay or if the desktop exposes a secure reverse tunnel.

• Pros:
   • Powerful, can script CLI directly.
• Cons:
   • Complex; user experience hurdles on iOS; security implications.
• Good for:
   • Power users or internal tooling; not recommended for general app users.

6) Local-only simulator/test mode on iOS

For development/testing, keep iOS sandboxed and load sample data or env-provided paths (as you already conditionally support). Not a cross-device solution, but helpful for UI iteration.

• Pros:
   • Keeps iOS code clean and independent.
• Cons:
   • Doesn’t solve real connectivity.

Data model and protocol considerations

Regardless of transport, define a clean, platform-agnostic protocol:

• Entities:
   • Threads, messages, summaries, statuses (mirroring your ThreadSummary and JSONL format).
• Operations:
   • Query threads (pagination + updated_at filter).
   • Get thread detail (messages).
   • Post message (from iOS to desktop).
   • Desktop events: new message, plan state updates, error notifications.
• Delivery:
   • REST for fetch; WebSocket/SSE for push.
   • Include a since parameter to fetch changes since a timestamp to avoid full sync.
• Auth:
   • Pairing flow: iOS scans a QR code from desktop containing a token and host info.
   • Persist token in Keychain; rotate if needed.
• Versioning:
   • Include API version in all requests and a TXT record (if using Bonjour).

Recommended path for your use case

Given you want the desktop to remain the filesystem/CLI owner and you want a real-time, “same thread view” experience:

• Start with Option 1: Desktop-local HTTP + WebSocket API, advertised via Bonjour.
   • Fast to implement, no external infra, great for dev and LAN usage.
   • Implement token-based auth and a pairing flow.
   • Define a minimal schema to mirror your summaries/messages.
• If you need remote access later, layer Option 2 (cloud relay).
   • Keep the same API contract; the desktop can connect to a relay and the iOS app talks to the relay when not on LAN.
   • Optionally support both: prefer LAN when available, fall back to relay.

Handling the iOS filesystem constraint

• Keep iOS file access minimal. Treat iOS as a viewer/controller:
   • Cache the latest fetched threads/messages locally in the app’s sandbox (e.g., Application Support) for offline view.
   • All authoritative data and writes go through the desktop API.
• Your current guard around homeDirectoryForCurrentUser is correct for preventing iOS from probing desktop paths.

Next steps (when you’re ready)

• I can draft a small protocol and skeleton endpoints for the desktop server, plus a Swift client wrapper for iOS (URLSession + WebSocket).
• If you prefer Multipeer Connectivity instead, I can outline the advertiser/browser setup and message framing.
• If cloud is appealing, I can sketch a minimal relay with auth and end-to-end encryption approach.

Tell me which option you want to pursue first, and I’ll propose concrete API shapes and code scaffolding (no changes made yet).
