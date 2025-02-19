# OpenAgents Sync Engine Design

Let’s flesh out your plan for the OpenAgents sync engine, incorporating ideas from **Linear** and **Figma’s architectures** while tailoring them to your tech stack:

- **Backend:** Rust, Axum, Postgres
- **Frontend:** React, TypeScript, Tailwind, Zustand

The goal is a **pragmatic, maintainable real-time sync solution** that supports your `useAgentSync()` hook, balancing performance, simplicity, and eventual consistency for a high volume of updates (AI agent chats, payments, reasoning, code changes, etc.).

---

## 🏗 High-Level Architecture Overview

### **Backend (Rust + Axum + Postgres)**

- **HTTP Endpoints:** Handle initial bootstrap sync and client-initiated mutations (e.g., creating a message or updating an agent’s state).
- **WebSocket Server:** Push real-time updates to connected clients using a persistent connection.
- **Postgres:** Store authoritative data and a sync event log for ordering changes and enabling delta syncs.
- **Sync Events:**
  - Every mutation (**insert, update, delete**) generates a `SyncEvent` with a **unique, monotonically increasing ID**.
  - Stored in **Postgres** and **broadcast via WebSocket**.

### **Frontend (React + TypeScript + Zustand)**

- **`useAgentSync()` Hook:**
  - A React hook that manages local state (via **Zustand**),
  - Communicates with **HTTP endpoints** for bootstrap/mutations,
  - Listens to **WebSocket updates**.
- **Local State:**
  - Lightweight in-memory store (**Zustand**) for fast UI updates,
  - Optional **IndexedDB** persistence for offline support (later).
- **Selective Updates:**
  - Components re-render **only** for relevant data using **Zustand’s subscription capabilities**.

---

## 🔄 Sync Protocol

Inspired by **Linear’s incremental `SyncAction` stream** and **Figma’s centralized operation ordering**, but **simplified**:

1. Clients **bootstrap** with an **HTTP call**.
2. They **receive incremental updates** over **WebSocket**.
3. If disconnected, they **catch up via delta sync**.

---

## 🗄 Backend Design (Rust + Axum + Postgres)

### **📂 Database Schema**

#### **Core Tables:**
Reflect your domain (e.g., `agents`, `messages`, `payments`, `code_changes`).
Each has a **primary key** (`id`) and an `updated_at` timestamp.

##### Example: `messages` Table
```sql
CREATE TABLE messages (
    id UUID PRIMARY KEY,
    agent_id UUID REFERENCES agents(id),
    content TEXT,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

🔄 Sync Log Table: sync_events

Column	Type	Description
id	bigserial	Auto-incrementing, unique event ID
scope	text	e.g., "org:123" or "agent:uuid" for filtering updates
model	text	e.g., "Message", "Payment"
model_id	uuid	Affected row’s primary key
action	text	"insert", "update", "delete"
data	jsonb	New/changed fields for insert/update, null for delete
created_at	timestamp	Event timestamp

Indexes:
	•	id (Primary key)
	•	scope (For filtering)
	•	created_at (For cleanup)

🖥 Server Structure

🌐 HTTP API (Axum)
	•	/sync/bootstrap → Returns initial state for a client.
	•	Query param: scope (e.g., "org:123")
	•	Response:

{
  "models": { "Messages": [...], "Agents": [...] },
  "lastSyncId": 1000
}


	•	/sync/delta → Fetches missed events for catch-up.
	•	Query params: lastSyncId=1000, scope
	•	Response:

{ "events": [SyncEvent], "latestSyncId": 1050 }


	•	Mutation endpoints (e.g., /api/message):
	•	POST request creates a message, updates DB, generates a SyncEvent, and returns the new record.
	•	Uses transactions to ensure consistency.

🔗 WebSocket Server (Axum + tokio-tungstenite)
	•	Endpoint: wss://api.openagents.com/sync
	•	Client connection flow:
	1.	Client sends:

{ "type": "subscribe", "scope": "org:123", "lastSyncId": 1000 }


	2.	Server responds with:
	•	Delta events (if lastSyncId provided)
	•	Bootstrap payload (if lastSyncId: 0)

	•	Manages connected clients:
	•	Uses a HashMap<Scope, Vec<WebSocket>> in memory.
	•	Broadcasts new SyncEvents to relevant scopes after DB writes.

🚀 Rust Implementation Sketch

use axum::{routing::get, Router};
use tokio_tungstenite::WebSocketStream;
use serde::{Serialize, Deserialize};
use sqlx::PgPool;
use std::collections::HashMap;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize)]
struct SyncEvent {
    id: i64,
    scope: String,
    model: String,
    model_id: String,
    action: String,
    data: Option<serde_json::Value>,
}

struct AppState {
    db: PgPool,
    ws_clients: Mutex<HashMap<String, Vec<WebSocketStream>>>, // scope -> clients
}

async fn ws_handler(ws: WebSocketStream, state: AppState) {
    // Handle subscription, send initial delta/bootstrap, listen for messages
}

async fn broadcast_event(event: SyncEvent, state: &AppState) {
    let clients = state.ws_clients.lock().await;
    if let Some(sockets) = clients.get(&event.scope) {
        for socket in sockets {
            socket.send(event.clone()).await; // Pseudocode
        }
    }
}

fn main() {
    let app = Router::new()
        .route("/sync/bootstrap", get(ws_handler))
        .with_state(AppState { ... });

    axum::Server::bind("0.0.0.0:3000").serve(app).await;
}

🎨 Frontend Design (React + TypeScript + Zustand)

useAgentSync() Hook

Defined in frontend/sync/useAgentSync.ts.
	•	Manages:
	•	WebSocket connection
	•	HTTP bootstrap/mutations
	•	Zustand local state
	•	Returns:
	•	Reactive state
	•	Mutation methods

🗄 Zustand Store

import create from 'zustand';

interface SyncState {
  models: { [model: string]: { [id: string]: any } };
  lastSyncId: number;
  setModelData: (model: string, id: string, data: any) => void;
  deleteModelData: (model: string, id: string) => void;
}

const useSyncStore = create<SyncState>((set) => ({
  models: {},
  lastSyncId: 0,
  setModelData: (model, id, data) =>
    set((state) => ({
      models: { ...state.models, [model]: { ...state.models[model], [id]: data } },
      lastSyncId: Math.max(state.lastSyncId, data.syncId || 0),
    })),
  deleteModelData: (model, id) =>
    set((state) => {
      const newModels = { ...state.models };
      delete newModels[model][id];
      return { models: newModels };
    }),
}));

🔥 Performance & Scalability
	•	Postgres: Transactions ensure consistency; indexed queries for fast lookups.
	•	WebSocket Optimizations: Scope-based filtering reduces unnecessary broadcasts.
	•	Frontend Rendering: Zustand’s selective updates prevent re-renders.

This design gives you a robust, real-time sync engine tailored to OpenAgents, leveraging your stack and inspired by Linear & Figma. 🚀 Start with this MVP, then iterate (e.g., offline mode, npm package) as needed!
