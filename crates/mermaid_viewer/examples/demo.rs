use mermaid_viewer::render_mermaid_code;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Mermaid diagram copied from PR #1356 comment.
    const MERMAID: &str = r#"
sequenceDiagram
  participant Client as TinyvexProvider
  participant Bridge as WS Bridge
  participant DB as TinyVex DB

  Client->>Bridge: tvx.query "threadsAndTails.list" {limit, perThreadTail}
  activate Bridge
  Bridge->>DB: list_threads(limit)
  DB-->>Bridge: threads[]
  par per-thread tails (parallel conceptual)
    Bridge->>DB: list_messages(threadId, perThreadTail)
    DB-->>Bridge: messages[] for threadId
  end
  Bridge-->>Client: tinyvex.query_result {name:"threadsAndTails.list", threads, tails}
  deactivate Bridge

  Client->>Client: Replace threads state, populate messagesByThread, update providers

  Note over Bridge,Client: Later updates include inline thread rows
  Bridge->>Client: tinyvex.update {stream:"threads", row:{...}, updatedAt}
  Client->>Client: Merge/Upsert row into local threads state (no re-query)
"#;

    let viewer = render_mermaid_code(MERMAID)?;
    viewer.run()?;
    Ok(())
}
