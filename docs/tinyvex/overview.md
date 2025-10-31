## Tinyvex — High‑Level Overview

```mermaid
flowchart LR
  subgraph Device
    title Developer Device
    App[Expo React Native App<br/>TinyvexProvider]
  end

  subgraph Bridge
    title oa-bridge (Rust)
    WS[WebSocket /ws]
    Writer[tinyvex_write.rs<br/>(JSONL -> ACP -> SQLite)]
  end

  subgraph Storage
    title Local Storage
    DB[(SQLite file<br/>crates/tinyvex)]
  end

  App --|connect ws://host:8787/ws|--> WS
  WS --|broadcast JSONL + tinyvex.update|--> App
  Writer --|upserts/queries|--> DB
  App --|tvx.query / tvx.subscribe|--> WS
  WS --|query|--> DB

  style App fill:#08090a,stroke:#23252a,color:#f7f8f8
  style WS fill:#0e0e12,stroke:#23252a,color:#f7f8f8
  style Writer fill:#0e0e12,stroke:#23252a,color:#f7f8f8
  style DB fill:#08090a,stroke:#62666d,color:#f7f8f8
```

Key points
- Tinyvex is a lightweight SQLite-backed store (crate `crates/tinyvex`).
- The Rust bridge (`crates/oa-bridge`) writes to Tinyvex as Codex events stream in, and exposes a WebSocket for the mobile app.
- The app subscribes to Tinyvex streams and issues queries via WebSocket control messages; updates are pushed as `tinyvex.update`.
