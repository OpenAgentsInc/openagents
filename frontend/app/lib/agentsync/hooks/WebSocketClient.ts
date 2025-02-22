interface WebSocketMessage {
  type: string;
  [key: string]: any;
}

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private messageHandlers: Set<(msg: WebSocketMessage) => void> = new Set();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectTimeout: NodeJS.Timeout | null = null;
  private connectionTimeout: NodeJS.Timeout | null = null;
  private messageQueue: string[] = [];
  private isConnecting = false;
  private isClosed = false;
  private sentMessages: Set<string> = new Set();

  constructor(private url: string) { }

  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  connect(): Promise<void> {
    if (this.ws?.readyState === WebSocket.OPEN) return Promise.resolve();
    if (this.isConnecting) return Promise.resolve();
    if (this.isClosed) return Promise.reject(new Error("Client is closed"));

    return new Promise((resolve, reject) => {
      this.isConnecting = true;

      // Clear any existing timeouts
      if (this.connectionTimeout) {
        clearTimeout(this.connectionTimeout);
      }
      if (this.reconnectTimeout) {
        clearTimeout(this.reconnectTimeout);
      }

      // Set connection timeout
      this.connectionTimeout = setTimeout(() => {
        if (this.ws?.readyState !== WebSocket.OPEN) {
          console.debug("WebSocket connection timeout");
          this.ws?.close();
          this.isConnecting = false;
          reject(new Error("WebSocket connection timeout"));
        }
      }, 5000);

      try {
        // Create WebSocket connection
        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
          if (this.connectionTimeout) {
            clearTimeout(this.connectionTimeout);
          }
          this.isConnecting = false;
          this.reconnectAttempts = 0;
          console.debug("WebSocket connected");

          // Process queued messages
          this.processQueue();
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const msg = JSON.parse(event.data);
            console.debug("WebSocket received:", msg);
            this.messageHandlers.forEach((handler) => handler(msg));
          } catch (e) {
            console.error("Failed to parse WebSocket message:", e);
          }
        };

        this.ws.onclose = (event) => {
          if (this.connectionTimeout) {
            clearTimeout(this.connectionTimeout);
          }
          this.isConnecting = false;
          console.debug("WebSocket closed:", event);

          if (!this.isClosed && this.reconnectAttempts < this.maxReconnectAttempts) {
            const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
            console.debug(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1})`);

            this.reconnectTimeout = setTimeout(
              () => {
                this.reconnectAttempts++;
                this.connect().catch(e => {
                  console.error("Reconnection failed:", e);
                });
              },
              delay
            );
          }
        };

        this.ws.onerror = (error) => {
          this.isConnecting = false;
          console.error("WebSocket error:", error);
          reject(error);
        };
      } catch (error) {
        this.isConnecting = false;
        console.error("Failed to create WebSocket:", error);
        reject(error);
      }
    });
  }

  disconnect() {
    this.isClosed = true;

    if (this.connectionTimeout) {
      clearTimeout(this.connectionTimeout);
    }
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }

    if (this.ws) {
      // Only close if not already closing/closed
      if (this.ws.readyState === WebSocket.OPEN) {
        this.ws.close();
      }
      this.ws = null;
    }

    this.messageHandlers.clear();
    this.messageQueue = [];
    this.isConnecting = false;
    this.reconnectAttempts = 0;
    this.sentMessages.clear();
    console.debug("WebSocket client disconnected");
  }

  clearHandlers() {
    console.debug("Clearing message handlers");
    this.messageHandlers.clear();
  }

  send(msg: any) {
    if (this.isClosed) {
      console.debug("Cannot send message - client is closed");
      return;
    }

    const msgStr = JSON.stringify(msg);

    // For subscription messages, check if we've already sent this subscription
    if (msg.type === "Subscribe") {
      const subscriptionKey = `subscription:${msg.scope}:${msg.conversation_id || 'default'}:${msg.connection_id}`;
      if (this.sentMessages.has(subscriptionKey)) {
        console.debug("Skipping duplicate subscription:", subscriptionKey);
        return;
      }
      this.sentMessages.add(subscriptionKey);
    }

    if (this.ws?.readyState === WebSocket.OPEN) {
      console.debug("WebSocket sending:", msg);
      this.ws.send(msgStr);
    } else {
      // Queue the message if not connected
      console.debug("WebSocket queuing message:", msg);
      this.messageQueue.push(msgStr);
      // Try to connect if not already connecting
      if (!this.isConnecting && !this.isClosed) {
        this.connect().catch(e => {
          console.error("Connection attempt failed:", e);
        });
      }
    }
  }

  private processQueue() {
    // Process any queued messages
    while (this.messageQueue.length > 0) {
      const msgStr = this.messageQueue.shift();
      if (msgStr && this.ws?.readyState === WebSocket.OPEN) {
        const msg = JSON.parse(msgStr);
        // Check for duplicate subscriptions in the queue
        if (msg.type === "Subscribe") {
          const subscriptionKey = `subscription:${msg.scope}:${msg.conversation_id || 'default'}:${msg.connection_id}`;
          if (this.sentMessages.has(subscriptionKey)) {
            console.debug("Skipping queued duplicate subscription:", subscriptionKey);
            continue;
          }
          this.sentMessages.add(subscriptionKey);
        }
        console.debug("WebSocket sending queued message:", msg);
        this.ws.send(msgStr);
      }
    }
  }

  onMessage(handler: (msg: WebSocketMessage) => void) {
    this.messageHandlers.add(handler);
    return () => {
      this.messageHandlers.delete(handler);
    };
  }
}
