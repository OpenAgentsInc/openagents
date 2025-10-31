## Tinyvex — Data Model (ER)

```mermaid
erDiagram
  threads {
    TEXT id PK
    TEXT threadId
    TEXT title
    TEXT projectId
    TEXT resumeId
    TEXT rolloutPath
    TEXT source
    INTEGER createdAt
    INTEGER updatedAt
  }

  messages {
    INTEGER id PK
    TEXT threadId FK
    TEXT role
    TEXT kind
    TEXT text
    TEXT data
    TEXT itemId
    INTEGER partial
    INTEGER seq
    INTEGER ts
    INTEGER createdAt
    INTEGER updatedAt
  }

  acp_events {
    INTEGER id PK
    TEXT sessionId
    TEXT clientThreadDocId
    INTEGER ts
    INTEGER seq
    TEXT updateKind
    TEXT role
    TEXT text
    TEXT toolCallId
    TEXT status
    TEXT kind
    TEXT content_json
    TEXT locations_json
    TEXT raw_json
    INTEGER createdAt
    INTEGER updatedAt
  }

  acp_tool_calls {
    TEXT threadId FK
    TEXT toolCallId
    TEXT title
    TEXT kind
    TEXT status
    TEXT content_json
    TEXT locations_json
    TEXT content
    TEXT locations
    INTEGER createdAt
    INTEGER updatedAt
  }

  acp_plan {
    TEXT threadId PK
    TEXT entries_json
    TEXT entries
    INTEGER createdAt
    INTEGER updatedAt
  }

  acp_state {
    TEXT threadId PK
    TEXT currentModeId
    TEXT available_commands_json
    TEXT available_commands
    INTEGER createdAt
    INTEGER updatedAt
  }

  threads ||--o{ messages : id to threadId
  threads ||--o{ acp_tool_calls : id to threadId
  threads ||--|| acp_plan : id to threadId
  threads ||--|| acp_state : id to threadId
  acp_events }o--o{ threads : clientThreadDocId
```

Notes
- Indices and uniqueness are defined in SQL (see `crates/tinyvex/src/lib.rs`).
- Message rows for streamed items are upserted and finalized by `tinyvex_write`.
