Short answer: **yes — with caveats**.

If your Expo/React Native app is running *on the same device that has the Tailscale client installed*, you can often discover Tailscale peers and their IPs — **but only if you can talk to Tailscale’s local API or the client exposes that info.** If your app is running on a different device (e.g., mobile app talking to your desktop over Tailscale), you can discover other devices’ Tailscale IPs by using the Tailscale admin/control API or by having devices advertise themselves over the tailnet.

Below I’ll summarize the practical options, platform limitations, and concrete next steps you can take.

# How discovery works (high-level)

* Tailscale assigns each node a stable Tailscale IP (and MagicDNS name) that other nodes on the same tailnet can use to reach it. So if you know a node’s Tailscale IP/name you can connect directly. ([Tailscale][1])
* Desktop/CLI Tailscale exposes `tailscale status --json` and a LocalAPI (`/localapi/v0/status`, etc.) that can list peers and addresses. That’s the easiest programmatic source of truth on a device that runs the full client/CLI. ([Tailscale][2])
* Mobile apps historically lack a shipped CLI; mobile behaviour varies by platform. Android has gained LocalAPI/intent support in places and has more flexibility; iOS is more locked down (the official client UI shows peer IPs, but programmatic access from a third-party app can be restricted). There are community wrappers/unofficial clients that hit the LocalAPI on-device. ([Tailscale][3])

# Practical approaches (choose one)

1. **If your Expo app runs on the *same mobile device* that has Tailscale installed**

   * Try the LocalAPI on `localhost` (common endpoint used historically is `http://localhost:41112/localapi/v0/status`). If available, that returns the current device’s status, and other endpoints (e.g., `whois`) let you map IP → device. This is the most direct approach. Example (React Native `fetch`):

     ```js
     // WARNING: may be unavailable/blocked on iOS or newer Tailscale versions
     const status = await fetch('http://localhost:41112/localapi/v0/status').then(r => r.json());
     console.log(status); // contains this device info and known peers
     ```

     Important: many mobile clients do not expose the LocalAPI to arbitrary apps for security reasons; behavior is platform- and versions-dependent. ([GitHub][4])

   * On Android there are community-reported intents / LocalAPI interactions that make this feasible; on iOS it's harder and often not possible for third-party apps without special entitlements. ([GitHub][5])

2. **If your app runs on a different device (or you need a robust production flow) — use the Admin/Control API**

   * Use the Tailscale admin REST API (requires API key from your Tailscale account) to list all devices in the tailnet and their Tailscale IPs and MagicDNS names. Your app (or a backend service) can call that API and then share device lists to your Expo app. This is the reliable, cross-platform method. ([Tailscale][6])
   * Security: an admin token grants access to device lists; protect it on a secure backend — don’t embed admin tokens directly in a mobile app.

3. **Peer self-advertising / discovery service (recommended for app-native discovery)**

   * Have each client open a small HTTP/UDP endpoint on a known port and register itself to a lightweight service (could be a tiny server or a peer-to-peer rendezvous over Tailscale). For example, on app start: POST `{name, tailscaleIP, port}` to your central registry (hosted on a machine on the tailnet). Other clients query that registry to discover peers. This avoids depending on LocalAPI availability and works across platforms because all devices can reach the registry over Tailscale IPs. (Design note: ensure auth — e.g., mutual TLS or API tokens — or rely on Tailscale ACLs.)
   * This also solves relay/DERP cases: even if two devices are behind relays, they still appear in the registry with their Tailscale IPs and can attempt connection. ([Tailscale][7])

# Things to watch out for

* **Platform differences:** LocalAPI availability and permissions differ (Android more flexible, iOS more restricted). Don’t count on being able to call `localhost:41112` from your Expo app on every phone. ([Tailscale][3])
* **ACLs & access control:** Tailscale ACLs can prevent visibility or connections between nodes; the admin API will still list devices your API key can see. ([Tailscale][8])
* **Relay vs direct:** connectivity may be relayed through DERP (relay) rather than direct P2P; that doesn’t prevent discovery of IPs, but it may affect latency and direct LAN scanning assumptions. ([Tailscale][7])
* **Privacy/security:** querying device lists or running a discovery mechanism should be secured. Don’t expose admin tokens in a packaged app.

# Quick recommended plan (practical)

1. If you control at least one always-on machine in your tailnet (server/desktop), **create a small registry service** on that machine (HTTP + simple auth). Have devices register their Tailscale IP / port on startup. Your Expo app queries the registry to discover peers. This is cross-platform and robust.
2. If you prefer to query device inventory directly from Tailscale, **set up a backend** that calls the Tailscale Admin API and returns a filtered device list to your mobile app. Secure the admin key on the backend.
3. If you want to experiment quickly on Android or a rooted/dev device, try reading the LocalAPI at `http://localhost:41112/localapi/v0/status` from your Expo app to see whether it’s exposed there. (Not recommended as a production strategy.) ([GitHub][9])

---

If you want, I can:

* give you a minimal Express/Node registry example (server) + Expo client code that registers itself and discovers peers, or
* produce sample RN code that tries the LocalAPI `status` endpoint and gracefully falls back to registry/admin API if not available.

Which of those would you like me to build now?

[1]: https://tailscale.com/kb/1452/connect-to-devices?utm_source=chatgpt.com "Connect to devices · Tailscale Docs"
[2]: https://tailscale.com/kb/1463/troubleshoot-connectivity?utm_source=chatgpt.com "Troubleshoot device connectivity"
[3]: https://tailscale.com/blog/android?utm_source=chatgpt.com "Using Tailscale for Android just got a whole lot better"
[4]: https://github.com/tailscale/tailscale/issues/6777?utm_source=chatgpt.com "Localapi no longer working on local port 41112 · Issue #6777"
[5]: https://github.com/tailscale/tailscale/issues/11683?utm_source=chatgpt.com "Android responding to an Intent for fetching Tailscale status ..."
[6]: https://tailscale.com/kb/1101/api?utm_source=chatgpt.com "Tailscale Docs - API"
[7]: https://tailscale.com/kb/1257/connection-types?utm_source=chatgpt.com "Connection types · Tailscale Docs"
[8]: https://tailscale.com/kb/1087/device-visibility?utm_source=chatgpt.com "What devices can connect to or know mine?"
[9]: https://github.com/tale/headplane/issues/65?utm_source=chatgpt.com "Implement local agent to query the Tailscale \"localapi\" #65"
