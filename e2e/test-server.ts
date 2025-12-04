/**
 * E2E Test Server for OpenAgents Desktop
 *
 * Serves the mainview in a browser-compatible mode with:
 * 1. HTTP server for mainview HTML at http://localhost:3333
 * 2. WebSocket server on port 4243 for HUD message injection
 * 3. API endpoint POST /api/inject-hud for sending messages from tests
 */

import type { HudMessage } from "../src/hud/protocol.js";
import { serializeHudMessage } from "../src/hud/protocol.js";
import { TEST_HTTP_PORT, TEST_WS_PORT } from "./constants.js";

// Store WebSocket clients for test message injection (keyed by clientId)
const wsClients = new Map<string, WebSocket>();

// Message queues for messages sent before target clients connect
const messageQueue: HudMessage[] = [];
const targetedMessageQueues = new Map<string, HudMessage[]>();

// Start WebSocket server for HUD message injection
const wsServer = Bun.serve<{ clientId: string }>({
  port: TEST_WS_PORT,
  fetch(req, server) {
    const url = new URL(req.url);
    const clientId =
      url.searchParams.get("clientId") ??
      (crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2));

    if (server.upgrade(req, { data: { clientId } })) {
      return; // WebSocket upgrade handled
    }
    return new Response("HUD Test WebSocket", { status: 200 });
  },
  websocket: {
    open(ws) {
      const clientId =
        ws.data?.clientId ??
        (crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2));
      wsClients.set(clientId, ws as unknown as WebSocket);
      console.log(`[WS] Client connected (${clientId}) (total: ${wsClients.size})`);

      // Send any queued messages for this client
      const queuedForClient = targetedMessageQueues.get(clientId);
      if (queuedForClient?.length) {
        for (const msg of queuedForClient) {
          ws.send(serializeHudMessage(msg));
        }
        targetedMessageQueues.delete(clientId);
      }

      // Send any queued broadcast messages
      for (const msg of messageQueue) {
        ws.send(serializeHudMessage(msg));
      }
      messageQueue.length = 0;
    },
    message(_ws, msg) {
      // Echo messages to all clients (for testing)
      const data = typeof msg === "string" ? msg : new TextDecoder().decode(msg);
      console.log(`[WS] Received: ${data.slice(0, 100)}...`);
    },
    close(ws) {
      const clientId = ws.data?.clientId;
      if (clientId) {
        wsClients.delete(clientId);
        targetedMessageQueues.delete(clientId);
      }
      console.log(`[WS] Client disconnected${clientId ? ` (${clientId})` : ""} (total: ${wsClients.size})`);
    },
  },
});

console.log(`[WS] WebSocket server listening on ws://localhost:${TEST_WS_PORT}`);

// Broadcast HUD message to all connected clients or a specific client
function broadcastHudMessage(message: HudMessage, targetClientId?: string): void {
  const serialized = serializeHudMessage(message);

  if (targetClientId) {
    const client = wsClients.get(targetClientId);
    if (!client) {
      const queue = targetedMessageQueues.get(targetClientId) ?? [];
      queue.push(message);
      targetedMessageQueues.set(targetClientId, queue);
      console.log(`[WS] No client ${targetClientId}, queued message: ${message.type}`);
      return;
    }
    (client as unknown as { send: (data: string) => void }).send(serialized);
    console.log(`[WS] Sent ${message.type} to client ${targetClientId}`);
    return;
  }

  if (wsClients.size === 0) {
    // Queue message if no clients connected
    messageQueue.push(message);
    console.log(`[WS] No clients, queued message: ${message.type}`);
    return;
  }
  for (const [, client] of wsClients) {
    (client as unknown as { send: (data: string) => void }).send(serialized);
  }
  console.log(`[WS] Broadcast ${message.type} to ${wsClients.size} clients`);
}

// Broadcast raw data (for malformed message testing)
function broadcastRawMessage(data: string, targetClientId?: string): void {
  if (targetClientId) {
    const client = wsClients.get(targetClientId);
    if (!client) {
      console.log(`[WS] No client ${targetClientId}, cannot send raw message`);
      return;
    }
    (client as unknown as { send: (data: string) => void }).send(data);
    console.log(`[WS] Sent raw data to client ${targetClientId}`);
    return;
  }

  if (wsClients.size === 0) {
    console.log(`[WS] No clients, cannot send raw message`);
    return;
  }
  for (const [, client] of wsClients) {
    (client as unknown as { send: (data: string) => void }).send(data);
  }
  console.log(`[WS] Broadcast raw data to ${wsClients.size} clients`);
}

// Inline test mainview HTML with mock Electrobun and test WebSocket
const testMainviewHtml = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>OpenAgents Flow HUD - E2E Test</title>
    <link rel="stylesheet" href="/mainview.css" />
    <style>
      /* Additional test styles */
      #test-indicator {
        position: fixed;
        top: 10px;
        right: 10px;
        background: #22c55e;
        color: white;
        padding: 4px 8px;
        border-radius: 4px;
        font-size: 12px;
        z-index: 9999;
      }
      #ws-status {
        position: fixed;
        top: 40px;
        right: 10px;
        padding: 4px 8px;
        border-radius: 4px;
        font-size: 12px;
        z-index: 9999;
      }
      .ws-connected { background: #22c55e; color: white; }
      .ws-disconnected { background: #ef4444; color: white; }
      #error-indicator {
        position: fixed;
        top: 70px;
        right: 10px;
        padding: 8px 12px;
        border-radius: 4px;
        font-size: 12px;
        z-index: 9999;
        background: #ef4444;
        color: white;
        display: none;
        max-width: 300px;
      }
      #error-indicator.visible { display: block; }
      #controls {
        position: fixed;
        bottom: 20px;
        right: 20px;
        z-index: 9999;
        display: flex;
        gap: 10px;
        align-items: center;
      }
      #controls button {
        background: #3b82f6;
        color: white;
        border: none;
        padding: 8px 16px;
        border-radius: 4px;
        cursor: pointer;
      }
      #controls button:hover {
        background: #2563eb;
      }
      #zoom-level {
        color: white;
        font-size: 14px;
      }
      #flow-container {
        width: 100vw;
        height: 100vh;
        overflow: hidden;
        background: #0a0a0f;
      }
      #flow-container.dragging {
        cursor: grabbing;
      }
      #flow-svg {
        width: 100%;
        height: 100%;
      }
      /* Make grid not block pointer events */
      #flow-svg > rect {
        pointer-events: none;
      }
    </style>
  </head>
  <body>
    <div id="test-indicator">E2E TEST MODE</div>
    <div id="ws-status" class="ws-disconnected">WS: Disconnected</div>
    <div id="error-indicator"></div>

    <div id="flow-container">
      <svg id="flow-svg" width="100%" height="100%">
        <defs>
          <pattern
            id="grid"
            width="20"
            height="20"
            patternUnits="userSpaceOnUse"
          >
            <circle cx="1" cy="1" r="1" fill="#1e1e2e" opacity="0.3" />
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#grid)" />
        <g class="flow-canvas" id="flow-canvas">
          <g class="flow-content" id="flow-content"></g>
        </g>
      </svg>
    </div>

    <div id="controls">
      <button id="reset-btn">Reset View</button>
      <span id="zoom-level">100%</span>
    </div>

    <script type="module">
      // Mock Electrobun for browser context
      window.Electrobun = {
        BrowserView: { current: { send: () => {} } },
      };

      // Test WebSocket connection
      const TEST_WS_PORT = ${TEST_WS_PORT};
      let ws = null;
      let reconnectTimer = null;

      const clientId = (crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2));
      window.__clientId = clientId;

      function connectWs() {
        const wsUrl = new URL(\`ws://localhost:\${TEST_WS_PORT}\`);
        wsUrl.searchParams.set('clientId', clientId);
        ws = new WebSocket(wsUrl.toString());

        ws.onopen = () => {
          console.log('[Test WS] Connected');
          document.getElementById('ws-status').className = 'ws-connected';
          document.getElementById('ws-status').textContent = 'WS: Connected';
        };

        ws.onmessage = (event) => {
          try {
            const msg = JSON.parse(event.data);
            console.log('[Test WS] Received:', msg.type);
            window.dispatchEvent(new CustomEvent('hud-message', { detail: msg }));
          } catch (e) {
            console.error('[Test WS] Parse error:', e);
          }
        };

        ws.onclose = () => {
          console.log('[Test WS] Disconnected');
          document.getElementById('ws-status').className = 'ws-disconnected';
          document.getElementById('ws-status').textContent = 'WS: Disconnected';
          // Reconnect after 1 second
          reconnectTimer = setTimeout(connectWs, 1000);
        };

        ws.onerror = (e) => {
          console.error('[Test WS] Error:', e);
        };
      }

      connectWs();

      // Expose for tests
      window.__testWs = () => ws;
      window.__hudMessages = [];

      window.addEventListener('hud-message', (e) => {
        window.__hudMessages.push(e.detail);
      });
    </script>

    <!-- Load the actual mainview bundle (will need to be built) -->
    <script type="module" src="/mainview.js"></script>
  </body>
</html>
`;

// Read the actual mainview CSS
const mainviewCssPath = new URL(
  "../src/mainview/index.css",
  import.meta.url
).pathname;
const mainviewCss = await Bun.file(mainviewCssPath).text().catch(() => "/* CSS not found */");

// Start HTTP server for mainview
const httpServer = Bun.serve({
  port: TEST_HTTP_PORT,
  async fetch(req) {
    const url = new URL(req.url);

    // API endpoint for injecting HUD messages
    if (url.pathname === "/api/inject-hud" && req.method === "POST") {
      try {
        const message = (await req.json()) as HudMessage;
        const targetClientId = req.headers.get("x-client-id") ?? url.searchParams.get("clientId") ?? undefined;
        broadcastHudMessage(message, targetClientId ?? undefined);
        return new Response(JSON.stringify({ ok: true, type: message.type }), {
          headers: { "Content-Type": "application/json" },
        });
      } catch (e) {
        return new Response(
          JSON.stringify({ ok: false, error: String(e) }),
          { status: 400, headers: { "Content-Type": "application/json" } }
        );
      }
    }

    // API endpoint for injecting raw/malformed data
    if (url.pathname === "/api/inject-raw" && req.method === "POST") {
      try {
        const rawData = await req.text();
        const targetClientId = req.headers.get("x-client-id") ?? url.searchParams.get("clientId") ?? undefined;
        broadcastRawMessage(rawData, targetClientId ?? undefined);
        return new Response(JSON.stringify({ ok: true, raw: true }), {
          headers: { "Content-Type": "application/json" },
        });
      } catch (e) {
        return new Response(
          JSON.stringify({ ok: false, error: String(e) }),
          { status: 400, headers: { "Content-Type": "application/json" } }
        );
      }
    }

    // API endpoint to disconnect all WebSocket clients (for testing)
    if (url.pathname === "/api/disconnect-ws" && req.method === "POST") {
      const clientCount = wsClients.size;
      for (const [, client] of wsClients) {
        try {
          (client as unknown as { close: () => void }).close();
        } catch {
          // Ignore close errors
        }
      }
      wsClients.clear();
      targetedMessageQueues.clear();
      return new Response(JSON.stringify({ ok: true, disconnected: clientCount }), {
        headers: { "Content-Type": "application/json" },
      });
    }

    // Serve mainview CSS
    if (url.pathname === "/mainview.css") {
      return new Response(mainviewCss, {
        headers: { "Content-Type": "text/css" },
      });
    }

    // Serve mainview JS (placeholder - will need bundling)
    if (url.pathname === "/mainview.js") {
      // For now, return a minimal script that sets up the flow
      const minimalJs = `
        console.log('[Mainview] Loading in E2E test mode');

        // Listen for HUD messages and update UI
        window.addEventListener('hud-message', (e) => {
          const msg = e.detail;
          console.log('[Mainview] Processing:', msg.type);

          // Update UI based on message type
          const content = document.getElementById('flow-content');
          if (!content) return;

          if (msg.type === 'session_start') {
            content.innerHTML = '<text x="50" y="50" fill="#fff">Session: ' + msg.sessionId + '</text>';
          }

          if (msg.type === 'task_selected') {
            content.innerHTML += '<g class="flow-node-group" data-node-id="' + msg.task.id + '"><rect x="100" y="100" width="200" height="80" fill="#1a1a2e" rx="8"/><text x="200" y="140" fill="#fff" text-anchor="middle">' + msg.task.title + '</text></g>';
          }

          if (msg.type === 'apm_update') {
            let apmWidget = document.querySelector('.apm-widget');
            if (!apmWidget) {
              apmWidget = document.createElementNS('http://www.w3.org/2000/svg', 'g');
              apmWidget.setAttribute('class', 'apm-widget');
              apmWidget.setAttribute('transform', 'translate(20, 20)');
              content.appendChild(apmWidget);
            }
            // Handle NaN/Infinity gracefully for APM display
            const apmValue = Number.isFinite(msg.sessionAPM) ? msg.sessionAPM.toFixed(1) : '0.0';
            const actionsValue = Number.isFinite(msg.totalActions) && msg.totalActions >= 0 ? msg.totalActions : 0;
            apmWidget.innerHTML = '<rect width="180" height="100" fill="#1a1a2e" rx="8"/><text x="10" y="30" fill="#fff" class="apm-value">APM: ' + apmValue + '</text><text x="10" y="50" fill="#888">Actions: ' + actionsValue + '</text>';
          }

          if (msg.type === 'error') {
            const errorIndicator = document.getElementById('error-indicator');
            if (errorIndicator) {
              errorIndicator.textContent = 'Error: ' + msg.error;
              errorIndicator.className = 'visible';
              // Store error for recovery testing
              window.__lastError = msg;
              window.__errorCount = (window.__errorCount || 0) + 1;
            }
          }
        });

        // Canvas pan/zoom state
        let scale = 1;
        let panX = 0;
        let panY = 0;
        let isDragging = false;
        let dragStartX = 0;
        let dragStartY = 0;
        let dragStartPanX = 0;
        let dragStartPanY = 0;

        const container = document.getElementById('flow-container');
        const canvas = document.getElementById('flow-canvas');
        const zoomLevel = document.getElementById('zoom-level');
        const resetBtn = document.getElementById('reset-btn');

        function updateTransform() {
          canvas.setAttribute('transform', 'translate(' + panX + ',' + panY + ') scale(' + scale + ')');
          zoomLevel.textContent = Math.round(scale * 100) + '%';
        }

        container.addEventListener('mousedown', (e) => {
          isDragging = true;
          dragStartX = e.clientX;
          dragStartY = e.clientY;
          dragStartPanX = panX;
          dragStartPanY = panY;
          container.classList.add('dragging');
        });

        document.addEventListener('mousemove', (e) => {
          if (!isDragging) return;
          panX = dragStartPanX + (e.clientX - dragStartX);
          panY = dragStartPanY + (e.clientY - dragStartY);
          updateTransform();
        });

        document.addEventListener('mouseup', () => {
          isDragging = false;
          container.classList.remove('dragging');
        });

        container.addEventListener('wheel', (e) => {
          e.preventDefault();
          const delta = e.deltaY > 0 ? 0.9 : 1.1;
          const newScale = Math.max(0.1, Math.min(4, scale * delta));

          // Zoom towards cursor
          const rect = container.getBoundingClientRect();
          const x = e.clientX - rect.left;
          const y = e.clientY - rect.top;

          panX = x - (x - panX) * (newScale / scale);
          panY = y - (y - panY) * (newScale / scale);
          scale = newScale;

          updateTransform();
        });

        resetBtn.addEventListener('click', () => {
          scale = 1;
          panX = 0;
          panY = 0;
          updateTransform();
        });

        // Initial transform
        updateTransform();
      `;
      return new Response(minimalJs, {
        headers: { "Content-Type": "application/javascript" },
      });
    }

    // Serve test mainview HTML
    if (url.pathname === "/" || url.pathname === "/index.html") {
      return new Response(testMainviewHtml, {
        headers: { "Content-Type": "text/html" },
      });
    }

    return new Response("Not Found", { status: 404 });
  },
});

console.log(`[HTTP] Test server listening on http://localhost:${TEST_HTTP_PORT}`);

// Export for programmatic use
export { httpServer, wsServer, broadcastHudMessage };
