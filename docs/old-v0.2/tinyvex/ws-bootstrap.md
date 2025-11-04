## Tinyvex — WS Bootstrap and Live Updates

```mermaid
sequenceDiagram
  participant App as RN App (TinyvexProvider)
  participant Bridge as WS Bridge
  participant DB as Tinyvex DB (SQLite)

  App->>Bridge: tvx.subscribe {stream:"threads"}
  App->>Bridge: tvx.query name:"threads.list" {limit:50}
  Bridge->>DB: list_threads(limit)
  DB-->>Bridge: rows: ThreadRow[]
  Bridge-->>App: tinyvex.query_result {name:"threads.list", rows}

  Note over App: Prefetch recent message tails per top threads
  loop for each top thread
    App->>Bridge: tvx.subscribe {stream:"messages", threadId}
    App->>Bridge: tvx.query name:"messages.list" {threadId, limit:200}
    Bridge->>DB: list_messages(threadId, limit)
    DB-->>Bridge: rows: MessageRow[]
    Bridge-->>App: tinyvex.query_result {name:"messages.list", threadId, rows}
  end

  Note over Bridge,App: Streaming writes send lightweight updates
  Bridge-->>App: tinyvex.update {stream:"messages", op, threadId, itemId, ...}
  App->>Bridge: (throttled) tvx.query name:"messages.list" {threadId, limit}
```

Notes
- Control verbs come over the same WebSocket as the live Codex stream.
- Provider logic debounces `threads.list` refreshes and throttles per-thread `messages.list` during streaming.

