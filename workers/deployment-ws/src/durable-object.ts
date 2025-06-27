import { DurableObject } from "cloudflare:workers"
import type { ConnectionInfo, DeploymentStatus, Env, WebSocketMessage } from "./types"

export class DeploymentSession extends DurableObject {
  private connections: Map<WebSocket, ConnectionInfo> = new Map()
  private deploymentStatus: DeploymentStatus
  private deploymentId: string = ""
  private envConfig: Env

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env)
    this.envConfig = env

    // Initialize deployment status
    this.deploymentStatus = {
      id: "",
      projectId: "",
      status: "pending",
      progress: 0,
      stage: "Initializing",
      timestamp: Date.now(),
      logs: []
    }
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

    // Handle internal API updates
    if (url.pathname === "/internal/update" && request.method === "POST") {
      return this.handleInternalUpdate(request)
    }

    // Handle WebSocket upgrade
    const upgradeHeader = request.headers.get("Upgrade")
    if (!upgradeHeader || upgradeHeader !== "websocket") {
      return new Response("Expected WebSocket", { status: 426 })
    }

    // Extract deployment ID
    const deploymentId = url.searchParams.get("deploymentId")
    if (!deploymentId) {
      return new Response("Missing deploymentId", { status: 400 })
    }

    this.deploymentId = deploymentId

    // Initialize deployment status if needed
    if (!this.deploymentStatus.id) {
      this.deploymentStatus.id = deploymentId
      this.deploymentStatus.projectId = deploymentId // Could be different in production
    }

    // Create WebSocket pair
    const pair = new WebSocketPair()
    const [client, server] = Object.values(pair)

    // Accept the WebSocket connection
    await this.handleWebSocketSession(server)

    // Return client WebSocket
    return new Response(null, {
      status: 101,
      webSocket: client
    })
  }

  private async handleWebSocketSession(ws: WebSocket) {
    // Accept the WebSocket connection
    this.ctx.acceptWebSocket(ws)

    // Create connection info
    const connectionInfo: ConnectionInfo = {
      id: crypto.randomUUID(),
      connectedAt: Date.now(),
      deploymentId: this.deploymentId
    }

    // Store connection
    this.connections.set(ws, connectionInfo)
    console.log(`WebSocket connected: ${connectionInfo.id} for deployment ${this.deploymentId}`)

    // Send initial status
    this.sendToWebSocket(ws, {
      type: "deployment_update",
      data: this.deploymentStatus
    })

    // Set up event handlers
    ws.addEventListener("message", (event) => {
      this.handleMessage(ws, event)
    })

    ws.addEventListener("close", () => {
      console.log(`WebSocket disconnected: ${connectionInfo.id}`)
      this.connections.delete(ws)

      // If no more connections and deployment is complete, schedule cleanup
      if (
        this.connections.size === 0 &&
        (this.deploymentStatus.status === "success" || this.deploymentStatus.status === "error")
      ) {
        // Keep DO alive for 5 minutes for late connections
        this.ctx.waitUntil(
          new Promise((resolve) => setTimeout(resolve, 5 * 60 * 1000))
        )
      }
    })

    ws.addEventListener("error", (error) => {
      console.error(`WebSocket error for ${connectionInfo.id}:`, error)
      this.connections.delete(ws)
    })
  }

  private handleMessage(ws: WebSocket, event: MessageEvent) {
    try {
      const message: WebSocketMessage = JSON.parse(event.data as string)

      switch (message.type) {
        case "ping":
          this.sendToWebSocket(ws, { type: "pong" })
          break

        default:
          console.log("Unknown message type:", message.type)
      }
    } catch (error) {
      console.error("Failed to handle message:", error)
    }
  }

  private async handleInternalUpdate(request: Request): Promise<Response> {
    try {
      // In production, verify internal API key
      const authHeader = request.headers.get("Authorization")
      if (this.envConfig.INTERNAL_API_KEY && authHeader !== `Bearer ${this.envConfig.INTERNAL_API_KEY}`) {
        return new Response("Unauthorized", { status: 401 })
      }

      const body = await request.json() as { status: DeploymentStatus }

      if (!body || typeof body !== "object" || !("status" in body)) {
        return new Response("Missing status", { status: 400 })
      }

      // Update deployment status
      this.updateDeploymentStatus(body.status)

      return new Response("OK", { status: 200 })
    } catch (error) {
      console.error("Failed to handle internal update:", error)
      return new Response("Internal Server Error", { status: 500 })
    }
  }

  private updateDeploymentStatus(status: Partial<DeploymentStatus>) {
    this.deploymentStatus = {
      ...this.deploymentStatus,
      ...status,
      timestamp: Date.now()
    }

    // Add log entry if message provided
    if (status.message && this.deploymentStatus.logs) {
      const logEntry = `[${new Date().toISOString()}] ${status.message}`
      this.deploymentStatus.logs.push(logEntry)

      // Keep only last 100 log entries
      if (this.deploymentStatus.logs.length > 100) {
        this.deploymentStatus.logs = this.deploymentStatus.logs.slice(-100)
      }
    }

    // Broadcast to all connected clients
    this.broadcast({
      type: "deployment_update",
      data: this.deploymentStatus
    })

    console.log(
      `Deployment ${this.deploymentId} updated: ${this.deploymentStatus.status} - ${this.deploymentStatus.progress}%`
    )
  }

  private sendToWebSocket(ws: WebSocket, message: WebSocketMessage) {
    try {
      ws.send(JSON.stringify(message))
    } catch (error) {
      console.error("Failed to send to WebSocket:", error)
      this.connections.delete(ws)
    }
  }

  private broadcast(message: WebSocketMessage) {
    const data = JSON.stringify(message)

    for (const [ws, info] of this.connections) {
      try {
        ws.send(data)
      } catch (error) {
        console.error(`Failed to send to connection ${info.id}:`, error)
        this.connections.delete(ws)
      }
    }

    console.log(`Broadcast to ${this.connections.size} connections`)
  }
}
