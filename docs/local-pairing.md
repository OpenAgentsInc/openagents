Yep — you can skip Tailscale for this. If the desktop and phone are on the **same Wi-Fi (same subnet)**, you can:

1. have the desktop bridge bind a local port,
2. show a **QR code** with its connect info, and
3. let the Expo app scan it and open a **WebSocket** to `ws://<lan-ip>:<port>`.

That’s a clean “QR = out-of-band signaling” flow and it works great for local pairing.

Here’s the practical blueprint, with the gotchas handled.

# What you need to make it reliable

* **Same subnet:** Phone and desktop must be on the same LAN/VLAN. Guest networks often block peer-to-peer.
* **Local network perms (mobile):**

  * iOS 14+: add `NSLocalNetworkUsageDescription` (and if you advertise Bonjour, add service types) to your app config.
  * Android: normal network perms (`INTERNET`).
* **Firewall:** Ensure the desktop’s OS firewall allows inbound on your chosen port.
* **IP churn:** LAN IPs can change. Put **multiple candidates** in the QR (all active non-loopback IPv4s + optional IPv6) and let the client try them in order.
* **Auth:** The QR should include a **short-lived pairing token** so that anyone else on the LAN can’t piggyback. Consider rotating it each run.
* **Security/TLS:** On LAN, `ws://` is okay if you do **app-level auth** (token, or even a Noise/PAKE handshake). Avoid self-signed `wss://` unless you plan to manage cert trust.

# Suggested QR payload

Keep it simple and explicit:

```json
{
  "v": 1,
  "protocol": "ws",
  "hosts": ["192.168.1.23:5173","10.0.0.21:5173"],
  "token": "pEoHk6J...short-lived...",
  "expires": 1730227200
}
```

(You can also use a URL scheme like
`tricoder://pair?v=1&hosts=192.168.1.23:5173,10.0.0.21:5173&token=...` if that’s easier to parse from a QR scanner.)

# Desktop (Rust) sketch

* Bind a WebSocket server (Axum or Tungstenite).
* Enumerate local interfaces (e.g., `get_if_addrs`) to collect candidate LAN IPs.
* Print a QR to the terminal (e.g., `qrcode` + `qrcodegen` or `qr2term`).
* Require the first client message to include the pairing `token`.

```rust
// Cargo.toml (minimal)
/// axum = "0.7"
/// axum-extra = { version = "0.9", features = ["ws"] }
/// tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
/// get_if_addrs = "0.5"
/// serde = { version = "1", features = ["derive"] }
/// serde_json = "1"
/// qrcode = "0.12"
/// image = "0.24"         # if you want to save PNG; for terminal use qr2term = "0.3"
/// qr2term = "0.3"
use axum::{routing::get, Router};
use axum::extract::ws::{WebSocketUpgrade, WebSocket, Message};
use axum::response::IntoResponse;
use get_if_addrs::get_if_addrs;
use serde::{Serialize, Deserialize};
use std::{net::SocketAddr, time::{SystemTime, Duration}};
use qr2term::print_qr;
use tokio::net::TcpListener;

#[derive(Serialize)]
struct QrPayload {
    v: u8,
    protocol: &'static str,
    hosts: Vec<String>,
    token: String,
    expires: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port = 5173;
    let token = nanoid::nanoid!(24); // or any random
    let expires = SystemTime::now()
        .checked_add(Duration::from_secs(5 * 60))
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)?.as_secs();

    let hosts = get_if_addrs()?
        .into_iter()
        .filter(|ifa| !ifa.is_loopback() && ifa.ip().is_ipv4())
        .map(|ifa| format!("{}:{}", ifa.ip(), port))
        .collect::<Vec<_>>();

    let payload = QrPayload { v:1, protocol:"ws", hosts, token: token.clone(), expires };
    let qr_text = serde_json::to_string(&payload)?;
    print_qr(qr_text.as_str())?; // prints QR to terminal

    // WS route with token check
    async fn ws_handler(
        ws: WebSocketUpgrade,
        token: String,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| handle_socket(socket, token))
    }

    let app = Router::new().route("/ws", get(|ws: WebSocketUpgrade| async move {
        ws.on_upgrade(|socket| async move { handle_socket(socket, std::env::var("PAIR_TOKEN").unwrap_or_default()).await })
    }));

    // Bind on 0.0.0.0 so all LAN IPs work
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    // Put the token somewhere the handler can reach; or use Extension/State in Axum
    std::env::set_var("PAIR_TOKEN", token);

    println!("Listening on ws://0.0.0.0:{port}/ws");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_socket(mut socket: WebSocket, token: String) {
    // Expect a first frame like: { "type": "hello", "token": "..." }
    if let Some(Ok(Message::Text(txt))) = socket.recv().await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            let ok = v.get("type") == Some(&"hello".into())
                && v.get("token").and_then(|t| t.as_str()) == Some(token.as_str());
            if !ok {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
        }
    } else {
        return;
    }

    // …proceed with your protocol…
    let _ = socket.send(Message::Text("{\"type\":\"ack\"}".into())).await;
}
```

> Notes:
> • You’ll probably pass `token` via Axum state instead of env; this is a compact sketch.
> • If you want mDNS discovery later, add Bonjour/zeroconf, but QR is fine for pairing.

# Expo client flow (scan → connect)

1. Scan the QR (e.g., `expo-barcode-scanner` or `expo-camera + expo-qr-code-scanner`).
2. Parse `hosts[]`, `token`.
3. Try each host in order: `new WebSocket("ws://<host>/ws")`.
4. On `open`, immediately send `{"type":"hello","token":"..."}`.
5. Use exponential backoff across candidates.

```ts
// Example (TypeScript/Expo)
type QrPayload = {
  v: number;
  protocol: 'ws';
  hosts: string[];
  token: string;
  expires: number;
};

async function connectFromQr(raw: string) {
  const data: QrPayload = JSON.parse(raw);
  if (Date.now()/1000 > data.expires) throw new Error('Pairing code expired');

  const errors: any[] = [];
  for (const host of data.hosts) {
    try {
      const ws = new WebSocket(`${data.protocol}://${host}/ws`);
      await new Promise<void>((resolve, reject) => {
        let opened = false;
        ws.onopen = () => {
          opened = true;
          ws.send(JSON.stringify({ type: 'hello', token: data.token }));
        };
        ws.onmessage = (evt) => {
          // expect {"type":"ack"} or your first event
          resolve();
        };
        ws.onerror = (e) => !opened && reject(e);
        ws.onclose = () => !opened && reject(new Error('Closed before open'));
        setTimeout(() => !opened && reject(new Error('Timeout')), 3500);
      });
      return; // success on this host
    } catch (e) {
      errors.push({ host, e });
      // next host
    }
  }
  throw new Error('All candidates failed: ' + JSON.stringify(errors.slice(0,2)));
}
```

# Nice upgrades (optional but useful)

* **Multiple interfaces & priorities:** Prefer RFC1918 private ranges; skip link-locals unless you support IPv6.
* **mDNS fallback:** Advertise `_tricoder._tcp.local` and let the app query that (React Native mDNS modules exist), but iOS needs proper Bonjour entitlements.
* **Noise/PAKE:** Wrap the WS payloads with a session key derived from the token (e.g., SPAKE2 → symmetric key → encrypt frames).
* **Reconnect strategy:** If the IP changes mid-session (sleep/wake), re-scan or keep a tiny “directory” endpoint at `http://<host>/.well-known/bridge.json` returning current WS host.

---

**Bottom line:** You don’t need Tailscale for this local pairing. A QR that encodes **(hosts[], port, short-lived token)** + a small WS server in your Rust bridge + an Expo client that tries candidates in order is enough for a fast, robust local connection. If/when you want “works anywhere,” add Tailscale (or WebRTC with a public signaling server) later without changing your pairing UX.
