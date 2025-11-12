 Plan: Port Tinyvex to OpenAgents Tauri App

     Summary

     Port the production-ready tinyvex implementation from oaprewipe to the current openagents Tauri app, replacing the broken Tauri event-based approach with a WebSocket sync
     engine. This enables multi-client support (desktop + mobile) with store-first persistence.

     Phase 1: Comment on Issue 1475 and Port Core Tinyvex

     1.1 Add Comment to Issue 1475

     File: GitHub issue #1475
     - Explain why Tauri events don't work for multi-client scenarios (events only flow app→webview, can't reach external clients)
     - Reference the tinyvex architecture as the proper approach
     - Link to this implementation plan

     1.2 Port Tinyvex Core Database Layer

     Source: /Users/christopherdavid/code/oaprewipe/crates/tinyvex/src/lib.rs
     Destination: /Users/christopherdavid/code/openagents/tauri/src-tauri/crates/tinyvex/src/lib.rs

     Copy entire module including:
     - SQLite schema (threads, messages, acp_events, acp_tool_calls, acp_plan, acp_state)
     - All CRUD operations with duplicate detection
     - Unit tests

     1.3 Port Writer with ACP Integration

     Source: /Users/christopherdavid/code/oaprewipe/crates/tinyvex/src/writer.rs
     Destination: /Users/christopherdavid/code/openagents/tauri/src-tauri/crates/tinyvex/src/writer.rs

     Copy streaming logic:
     - Stream tracking and accumulation
     - ACP SessionUpdate mirroring
     - Finalization with duplicate detection
     - WriterNotification enum for UI updates
     - Unit tests

     1.4 Add Dependencies to Tauri Cargo.toml

     File: /Users/christopherdavid/code/openagents/tauri/src-tauri/Cargo.toml

     Add:
     axum = "0.8"
     tokio-tungstenite = "0.26"
     rusqlite = { version = "0.36", features = ["bundled", "column_decltype"] }
     bytes = "1.5"
     json-canon = "0.1"
     ts-rs = { version = "11.1", features = ["serde-compat"] }

     [dependencies.tinyvex]
     path = "./crates/tinyvex"

     Phase 2: Port WebSocket Server and Subscription System

     2.1 Port WebSocket Server

     Source: /Users/christopherdavid/code/oaprewipe/crates/oa-bridge/src/ws.rs
     Destination: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/tinyvex_ws.rs

     Adapt for Tauri:
     - WebSocket endpoint setup with axum
     - Broadcast channel for real-time updates
     - History replay buffer
     - Subscription system (tvx.subscribe)
     - Query handlers (threads.list, messages.list, messages.tailMany, etc.)
     - Control command routing

     2.2 Port Control Commands

     Source: /Users/christopherdavid/code/oaprewipe/crates/oa-bridge/src/controls.rs
     Destination: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/tinyvex_controls.rs

     Copy control command enum and parser

     2.3 Port State Management

     Source: /Users/christopherdavid/code/oaprewipe/crates/oa-bridge/src/state.rs
     Destination: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/tinyvex_state.rs

     Adapt AppState for Tauri context

     2.4 Port Tinyvex Write Adapters

     Source: /Users/christopherdavid/code/oaprewipe/crates/oa-bridge/src/tinyvex_write.rs
     Destination: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/tinyvex_adapters.rs

     Copy WriterNotification → WebSocket broadcast logic

     Phase 3: Integrate with Existing ACP Session Manager

     3.1 Wire ACP Session Manager to Tinyvex Writer

     File: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/oa_acp/session_manager.rs

     Modify handle_notification():
     - Remove Tauri event emission (app.emit() calls)
     - Replace with tinyvex writer calls:
       - writer.mirror_acp_update_to_tinyvex(session_id, update)
       - This persists to SQLite and broadcasts via WebSocket

     3.2 Start WebSocket Server in Tauri Setup

     File: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/lib.rs or main.rs

     Add in run():
     let tinyvex_state = AppState::new();
     let ws_server = tokio::spawn(async move {
         tinyvex_ws::serve(tinyvex_state, "127.0.0.1:9099").await
     });

     Phase 4: React Client Hooks

     4.1 Create WebSocket Client Hook

     File: /Users/christopherdavid/code/openagents/tauri/src/lib/useTinyvexWebSocket.ts

     export function useTinyvexWebSocket(url: string = "ws://localhost:9099") {
       const [socket, setSocket] = useState<WebSocket | null>(null);
       const [connected, setConnected] = useState(false);

       useEffect(() => {
         const ws = new WebSocket(url);
         ws.onopen = () => setConnected(true);
         ws.onclose = () => setConnected(false);
         setSocket(ws);
         return () => ws.close();
       }, [url]);

       return { socket, connected };
     }

     4.2 Create Subscription Hook

     File: /Users/christopherdavid/code/openagents/tauri/src/lib/useTinyvexSubscription.ts

     export function useTinyvexSubscription<T>(
       queryName: string,
       params: any
     ): T | null {
       const [data, setData] = useState<T | null>(null);
       const { socket } = useTinyvexWebSocket();

       useEffect(() => {
         if (!socket) return;

         const subId = `${queryName}:${JSON.stringify(params)}`;

         // Send subscription request
         socket.send(JSON.stringify({
           jsonrpc: "2.0",
           method: "tvx.subscribe",
           params: { queryName, params, subId }
         }));

         // Listen for updates
         const handler = (event: MessageEvent) => {
           const msg = JSON.parse(event.data);
           if (msg.method === "tinyvex/data" && msg.params.subId === subId) {
             setData(msg.params.value);
           }
         };

         socket.addEventListener("message", handler);

         // Cleanup
         return () => {
           socket.removeEventListener("message", handler);
           socket.send(JSON.stringify({
             jsonrpc: "2.0",
             method: "tvx.unsubscribe",
             params: { subId }
           }));
         };
       }, [socket, queryName, JSON.stringify(params)]);

       return data;
     }

     4.3 Create Session Updates Hook

     File: /Users/christopherdavid/code/openagents/tauri/src/lib/useAcpSessionUpdates.ts

     export function useAcpSessionUpdates(sessionId: string) {
       const messages = useTinyvexSubscription<Message[]>(
         "messages.list",
         { threadId: sessionId }
       );

       const toolCalls = useTinyvexSubscription<ToolCall[]>(
         "toolCalls.list",
         { threadId: sessionId }
       );

       return { messages, toolCalls };
     }

     4.4 Remove Broken acp-store.ts

     File: /Users/christopherdavid/code/openagents/tauri/src/lib/acp-store.ts

     Delete this file and remove all imports/usages (replaced by WebSocket hooks)

     Phase 5: Update React Components

     5.1 Update App.tsx

     File: /Users/christopherdavid/code/openagents/tauri/src/App.tsx

     Replace ACP adapter polling logic with WebSocket subscription:
     - Remove Zustand store usage
     - Use useAcpSessionUpdates() hook
     - Stream updates naturally via React state

     5.2 Update Thread Components

     Files: /Users/christopherdavid/code/openagents/tauri/src/components/assistant-ui/*.tsx

     Use tinyvex hooks for data fetching instead of manual Tauri commands

     Phase 6: TypeScript Type Generation

     6.1 Generate TypeScript Types from Rust

     File: /Users/christopherdavid/code/openagents/tauri/src-tauri/src/tinyvex_types.rs

     Create type export module:
     use ts_rs::TS;

     #[derive(Serialize, Deserialize, TS)]
     #[ts(export)]
     pub struct ThreadSummary {
         pub thread_id: String,
         pub message_count: u32,
         pub last_message_at: Option<i64>,
     }

     // Export all types used in WebSocket protocol

     Run cargo test to generate TypeScript definitions in bindings/

     6.2 Import Generated Types in React

     File: /Users/christopherdavid/code/openagents/tauri/src/types/tinyvex.ts

     export type { ThreadSummary, Message, ToolCall } from "../bindings/";

     Phase 7: Testing and Documentation

     7.1 Test WebSocket Connection

     - Start Tauri app
     - Verify WebSocket server listening on localhost:9099
     - Check browser console for successful connection

     7.2 Test ACP Event Flow

     - Create new session
     - Send prompt
     - Verify events appear in SQLite database
     - Verify WebSocket broadcasts updates
     - Verify React UI updates in real-time

     7.3 Test Multi-Client

     - Open desktop app
     - Connect from mobile browser to ws://[desktop-ip]:9099
     - Verify both clients receive updates

     7.4 Update Documentation

     File: /Users/christopherdavid/code/openagents/docs/assistant-ui/acp-integration-plan.md

     Update Phase 2 section with actual implementation details

     Phase 8: Cleanup and Optimization

     8.1 Remove Dead Code

     - Remove Tauri event emission code from session_manager.rs
     - Remove acp-store.ts and related Zustand store
     - Remove unused Tauri commands if any

     8.2 Add Configuration

     File: /Users/christopherdavid/code/openagents/tauri/src-tauri/tauri.conf.json

     Configure WebSocket server port (or use env var):
     {
       "plugins": {
         "tinyvex": {
           "wsPort": 9099,
           "dbPath": "~/.openagents/tinyvex/data.sqlite3"
         }
       }
     }

     8.3 Add Error Handling

     Add proper error boundaries in React components and error recovery in Rust WebSocket server

     Success Criteria

     - ✅ WebSocket server running on localhost:9099
     - ✅ ACP events persisted to SQLite before broadcast
     - ✅ React hooks receiving real-time updates via WebSocket
     - ✅ Desktop app shows live streaming text
     - ✅ Mobile browser can connect and receive same updates
     - ✅ No more Tauri event listener code
     - ✅ TypeScript types auto-generated from Rust structs
     - ✅ Issue 1475 closed with explanation comment

     Notes

     - Keep Ollama integration using direct streaming (no changes needed)
     - Focus on ACP event streaming via tinyvex WebSocket
     - Use store-first pattern: SQLite → WebSocket → UI
     - Support both localhost (desktop) and LAN (mobile) connections
     - No Bonjour discovery needed (manual IP entry or localhost for now)
