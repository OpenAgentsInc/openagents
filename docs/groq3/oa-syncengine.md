Let’s flesh out your plan for the OpenAgents sync engine, incorporating ideas from Linear and Figma’s architectures while tailoring them to your tech stack (Rust, Axum, Postgres on the backend; React, TypeScript, Tailwind, and Zustand on the frontend). The goal is a pragmatic, maintainable real-time sync solution that supports your useAgentSync() hook, balancing performance, simplicity, and eventual consistency for a high volume of updates (AI agent chats, payments, reasoning, code changes, etc.). Here’s a detailed design:
High-Level Architecture Overview
Backend (Rust + Axum + Postgres):
HTTP Endpoints: Handle initial bootstrap sync and client-initiated mutations (e.g., creating a message or updating an agent’s state).
WebSocket Server: Push real-time updates to connected clients using a persistent connection.
Postgres: Store authoritative data and a sync event log for ordering changes and enabling delta syncs.
Sync Events: Every mutation (insert, update, delete) generates a SyncEvent with a unique, monotonically increasing ID, stored in Postgres and broadcast via WebSocket.
Frontend (React + TypeScript + Zustand):
useAgentSync() Hook: A React hook that manages local state (via Zustand), communicates with HTTP endpoints for bootstrapping/mutations, and listens to WebSocket updates.
Local State: A lightweight in-memory store (Zustand) for fast UI updates, with optional persistence (e.g., IndexedDB) later if offline support is needed.
Selective Updates: Components only re-render for the data they care about, using Zustand’s subscription capabilities.
Sync Protocol:
Inspired by Linear’s incremental SyncAction stream and Figma’s centralized operation ordering, but simplified for your use case.
Clients bootstrap with an HTTP call, then receive incremental updates over WebSocket, catching up via delta sync if disconnected.
Backend Design (Rust + Axum + Postgres)
Database Schema
Core Tables: Reflect your domain (e.g., agents, messages, payments, code_changes). Each has a primary key (e.g., id) and an updated_at timestamp.
Example: messages (id: uuid, agent_id: uuid, content: text, created_at: timestamp, updated_at: timestamp).
Sync Log Table: sync_events
Columns:
id: bigserial (auto-incrementing, unique event ID)
scope: text (e.g., "org:123" or "agent:uuid", for filtering updates to relevant clients)
model: text (e.g., "Message", "Payment")
model_id: uuid (the affected row’s primary key)
action: text (e.g., "insert", "update", "delete")
data: jsonb (new/changed fields for insert/update, null for delete)
created_at: timestamp
Indexes: id (primary key), scope (for filtering), and created_at (for cleanup).
Server Structure
HTTP API (Axum):
/sync/bootstrap: Returns initial state for a client (e.g., all relevant agents, messages, etc., scoped to a user or org).
Query param: scope (e.g., "org:123").
Response: JSON with current state and latest sync_event.id (e.g., { models: { Messages: [...], Agents: [...] }, lastSyncId: 1000 }).
/sync/delta: Fetches missed events for catch-up.
Query params: lastSyncId (e.g., 1000), scope.
Response: { events: [SyncEvent], latestSyncId: 1050 }.
Mutation endpoints (e.g., /api/message):
POST to create a message, updates DB, generates a SyncEvent, and returns the new record.
Within a transaction: Update messages and insert into sync_events.
WebSocket Server (Axum + tokio-tungstenite):
Endpoint: wss://api.openagents.com/sync.
On connect, client sends { type: "subscribe", scope: "org:123", lastSyncId: 1000 }.
Server responds with delta events (if lastSyncId provided) or a bootstrap payload (if lastSyncId: 0).
Maintains a HashMap<Scope, Vec<WebSocket>> in memory to track connected clients by scope (e.g., "org:123").
Broadcasts new SyncEvents to relevant scopes after DB writes.
Sync Event Generation:
Every DB write (via HTTP mutation) triggers:
Update the core table (e.g., INSERT INTO messages ...).
Insert into sync_events (e.g., INSERT INTO sync_events (scope, model, model_id, action, data) VALUES ('org:123', 'Message', new_uuid, 'insert', new_message_json)).
Broadcast the event to WebSocket clients in the same scope.
Rust Implementation Sketch
rust
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

async fn bootstrap_handler(scope: String, state: AppState) -> Json<BootstrapResponse> {
    // Fetch current state from DB, return with latest sync ID
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
        .route("/sync/bootstrap", get(bootstrap_handler))
        .route("/sync", get(ws_handler))
        .with_state(AppState { ... });
    axum::Server::bind("0.0.0.0:3000").serve(app).await;
}
Performance Notes
Postgres: Use transactions for consistency (core table + sync log). Index sync_events.scope for fast filtering.
Scalability: Scope events by org/agent to limit broadcast scope. Partition sync_events by scope if volume grows.
Cleanup: Periodically prune old sync_events (e.g., after 30 days, assuming clients sync regularly).
Frontend Design (React + TypeScript + Zustand)
useAgentSync() Hook
Defined in frontend/sync/useAgentSync.ts.
Manages a WebSocket connection, HTTP bootstrap/mutations, and local state via Zustand.
Returns reactive state and methods for mutations.
Zustand Store
A single store for all syncable data, keyed by model (e.g., messages, agents).
Supports selective subscriptions so components only re-render for relevant changes.
Implementation Sketch
typescript
// frontend/sync/useAgentSync.ts
import { useEffect } from 'react';
import create from 'zustand';

// Sync state shape
interface SyncState {
  models: {
    [model: string]: { [id: string]: any }; // e.g., messages: { uuid: {...} }
  };
  lastSyncId: number;
  setModelData: (model: string, id: string, data: any) => void;
  deleteModelData: (model: string, id: string) => void;
}

// Zustand store
const useSyncStore = create<SyncState>((set) => ({
  models: {},
  lastSyncId: 0,
  setModelData: (model, id, data) =>
    set((state) => ({
      models: {
        ...state.models,
        [model]: { ...state.models[model], [id]: data },
      },
      lastSyncId: Math.max(state.lastSyncId, data.syncId || 0),
    })),
  deleteModelData: (model, id) =>
    set((state) => {
      const newModels = { ...state.models };
      delete newModels[model][id];
      return { models: newModels };
    }),
}));

// WebSocket client
class SyncClient {
  ws: WebSocket;
  constructor(scope: string, lastSyncId: number) {
    this.ws = new WebSocket('wss://api.openagents.com/sync');
    this.ws.onopen = () => {
      this.ws.send(JSON.stringify({ type: 'subscribe', scope, lastSyncId }));
    };
    this.ws.onmessage = (msg) => {
      const { events, latestSyncId } = JSON.parse(msg.data);
      events.forEach((event: SyncEvent) => {
        const store = useSyncStore.getState();
        if (event.action === 'delete') {
          store.deleteModelData(event.model, event.model_id);
        } else {
          store.setModelData(event.model, event.model_id, event.data);
        }
      });
    };
  }

  sendMutation(model: string, action: string, data: any) {
    fetch(`/api/${model}`, { method: 'POST', body: JSON.stringify(data) });
    // Optimistic update applied locally via store
  }
}

// Hook
export function useAgentSync(scope: string) {
  const { models, lastSyncId } = useSyncStore();
  useEffect(() => {
    const client = new SyncClient(scope, lastSyncId);
    return () => client.ws.close();
  }, [scope, lastSyncId]);

  const mutate = (model: string, action: string, data: any) => {
    const client = new SyncClient(scope, lastSyncId); // Simplified, reuse instance in practice
    client.sendMutation(model, action, data);
  };

  return { data: models, mutate };
}

// Usage in a component
function MessageList({ agentId }) {
  const { data } = useAgentSync(`agent:${agentId}`);
  return (
    <ul>
      {Object.values(data.messages || {}).map((msg) => (
        <li key={msg.id}>{msg.content}</li>
      ))}
    </ul>
  );
}
Key Features
Bootstrap: On mount, fetches initial state via /sync/bootstrap if lastSyncId is 0.
Real-Time Updates: WebSocket listener applies SyncEvents to Zustand store.
Optimistic Updates: mutate updates local state immediately, reconciled by server events.
Selective Rendering: Components subscribe to specific models (e.g., messages) via Zustand’s selector API.
Future Extraction
Move SyncClient and store logic to an npm package (e.g., @openagents/sync).
Expose useAgentSync and raw client for third-party frontends.
Sync Protocol
Bootstrap: { type: "bootstrap", models: {...}, lastSyncId: 1000 }.
Delta Sync: { type: "sync", events: [SyncEvent], latestSyncId: 1050 }.
Real-Time Update: { type: "sync", events: [{ id: 1051, scope: "org:123", model: "Message", model_id: "uuid", action: "insert", data: {...} }] }.
Consistency & Conflict Resolution
Centralized Ordering: Postgres sync_events.id ensures all clients apply events in the same order.
Last-Writer-Wins: Concurrent edits to the same field resolve to the latest SyncEvent. Rare conflicts are acceptable (e.g., two users editing a message simultaneously).
Scope Filtering: Events broadcast only to clients in the same scope (e.g., "org:123"), reducing noise.
Performance & Scalability
Backend: Rust’s async runtime and Axum handle high WebSocket throughput. Postgres scales with indexing and partitioning.
Frontend: Zustand’s lightweight state avoids React re-renders for unrelated data. Batch WebSocket updates (multiple events per message).
Network: Small SyncEvent payloads minimize bandwidth.
Trade-offs
Offline Support: Not prioritized initially (requires local persistence like IndexedDB). Add later with delta sync.
Complexity: Simpler than CRDTs/OT, but requires maintaining a sync log and WebSocket server.
Conflicts: Rare overwrites are possible but manageable given real-time feedback.
This design gives you a robust, real-time sync engine tailored to OpenAgents, leveraging your stack and drawing from Linear’s local-first pragmatism and Figma’s centralized simplicity. Start with this MVP, then iterate (e.g., offline mode, npm package) as needed!
