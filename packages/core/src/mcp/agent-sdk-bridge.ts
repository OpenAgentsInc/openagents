/**
 * Agent SDK Browser Bridge
 * 
 * This file provides a browser-compatible bridge to the Cloudflare Agents SDK.
 * It uses native WebSocket communication to interact with agent servers without
 * depending on Node.js-specific APIs.
 * 
 * Implementation details:
 * - Uses the browser's native WebSocket implementation for cross-platform compatibility
 * - Supports the same interface as the official Cloudflare Agents SDK
 * - Handles connection management, reconnection, and message routing
 * - Provides RPC-style method calling with proper error handling
 * - Supports state synchronization between client and server
 * 
 * This bridge allows OpenAgents to connect to Cloudflare Workers running the Agents SDK
 * directly from browser environments without requiring server-side proxying.
 */

/**
 * Base agent client interface that matches the Cloudflare Agents SDK
 */
export interface BaseAgentClient {
  agent: string;
  name: string;
  call<T = unknown>(method: string, args?: unknown[]): Promise<T>;
  setState(state: any): void;
  close(): void;
}

/**
 * Options for creating an agent client connection
 */
export interface AgentClientOptions {
  /** Name of the agent to connect to */
  agent: string;
  /** Name of the specific Agent instance */
  name?: string;
  /** Host URL for the agent server */
  host?: string;
  /** Headers for authentication */
  headers?: Record<string, string>;
  /** Called when the Agent's state is updated */
  onStateUpdate?: (state: any, source: "server" | "client") => void;
}

/**
 * Websocket-based implementation of the Agent client
 */
export class AgentClient implements BaseAgentClient {
  private socket: WebSocket | null = null;
  private messageQueue: Map<string, { resolve: Function, reject: Function }> = new Map();
  private connected = false;
  private reconnectAttempts = 0;
  private messageId = 0;
  
  agent: string;
  name: string;
  
  constructor(private options: AgentClientOptions) {
    this.agent = options.agent;
    this.name = options.name || 'default';
    
    // Initialize the connection immediately
    this.connect();
  }
  
  /**
   * Establishes WebSocket connection to the agent server
   */
  private connect() {
    const host = this.options.host || 'https://agents.openagents.com';
    
    // Properly construct WebSocket URL from HTTP URL
    let wsUrl = '';
    try {
      // Handle URLs with or without protocol
      const hostWithProtocol = host.startsWith('http') ? host : `https://${host}`;
      const url = new URL(hostWithProtocol);
      const wsProtocol = url.protocol === 'https:' ? 'wss' : 'ws';
      
      // This must match the exact route pattern defined in the Cloudflare Worker
      wsUrl = `${wsProtocol}://${url.host}/api/agent/${this.agent}/${this.name}`;
    } catch (e) {
      // Fallback for invalid URLs
      const wsProtocol = host.startsWith('https') ? 'wss' : 'ws';
      wsUrl = `${wsProtocol}://${host.replace(/^https?:\/\//, '')}/api/agent/${this.agent}/${this.name}`;
    }
    
    console.log(`Connecting to agent at ${wsUrl}`);
    
    try {
      this.socket = new WebSocket(wsUrl);
      
      this.socket.onopen = () => {
        console.log(`Connected to agent ${this.agent}/${this.name}`);
        this.connected = true;
        this.reconnectAttempts = 0;
        
        // Notify connection is ready via an initial handshake
        try {
          this.socket?.send(JSON.stringify({
            type: 'handshake',
            agent: this.agent,
            name: this.name,
            version: '1.0.0'
          }));
        } catch (error) {
          console.warn('Failed to send handshake message:', error);
        }
      };
      
      this.socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          
          // Handle state updates
          if (data.type === 'state') {
            this.options.onStateUpdate?.(data.state, 'server');
            return;
          }
          
          // Handle RPC responses
          if (data.type === 'response' && data.id && this.messageQueue.has(data.id)) {
            const { resolve, reject } = this.messageQueue.get(data.id)!;
            
            if (data.error) {
              reject(new Error(data.error));
            } else {
              resolve(data.result);
            }
            
            this.messageQueue.delete(data.id);
          }
        } catch (error) {
          console.error('Error processing message:', error);
        }
      };
      
      this.socket.onerror = (error) => {
        console.error('WebSocket error:', error);
      };
      
      this.socket.onclose = () => {
        this.connected = false;
        console.log(`Disconnected from agent ${this.agent}/${this.name}`);
        
        // Reject all pending requests
        for (const [id, { reject }] of this.messageQueue.entries()) {
          reject(new Error('WebSocket connection closed'));
          this.messageQueue.delete(id);
        }
        
        // Attempt reconnection with exponential backoff
        if (this.reconnectAttempts < 5) {
          const delay = Math.min(1000 * (2 ** this.reconnectAttempts), 30000);
          this.reconnectAttempts++;
          
          console.log(`Attempting to reconnect in ${delay}ms...`);
          setTimeout(() => this.connect(), delay);
        }
      };
    } catch (error) {
      console.error('Failed to connect to agent:', error);
      throw error;
    }
  }
  
  /**
   * Calls a method on the agent
   */
  async call<T = unknown>(method: string, args: unknown[] = []): Promise<T> {
    // More robust check for connection status
    if (!this.socket) {
      throw new Error('Not connected to agent: socket not initialized');
    }

    if (!this.connected) {
      throw new Error('Not connected to agent: connection not established');
    }

    if (this.socket.readyState !== WebSocket.OPEN) {
      // Check specific socket state and provide more detailed error
      const states = ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'];
      throw new Error(`Not connected to agent: socket in ${states[this.socket.readyState]} state`);
    }
    
    return new Promise<T>((resolve, reject) => {
      const id = `${Date.now()}-${this.messageId++}`;
      const message = {
        id,
        type: 'call',
        method,
        args
      };
      
      // Store the callback in the queue
      this.messageQueue.set(id, { resolve, reject });
      
      // Set a timeout for the request
      const timeout = setTimeout(() => {
        if (this.messageQueue.has(id)) {
          this.messageQueue.delete(id);
          reject(new Error(`Request timed out after 30s: ${method}`));
        }
      }, 30000);
      
      // Send the message
      try {
        if (this.socket && this.socket.readyState === WebSocket.OPEN) {
          this.socket.send(JSON.stringify(message));
        } else {
          throw new Error('Socket not available or not in OPEN state');
        }
      } catch (error) {
        clearTimeout(timeout);
        this.messageQueue.delete(id);
        reject(error);
      }
    });
  }
  
  /**
   * Updates the agent's state
   */
  setState(state: any): void {
    if (!this.socket || !this.connected) {
      console.warn('Cannot set state: not connected to agent');
      return;
    }
    
    try {
      if (this.socket.readyState === WebSocket.OPEN) {
        this.socket.send(JSON.stringify({
          type: 'setState',
          state
        }));
        
        // Notify of the state change locally
        this.options.onStateUpdate?.(state, 'client');
      } else {
        console.warn('Cannot set state: socket not in OPEN state');
      }
    } catch (error) {
      console.error('Failed to set state:', error);
    }
  }
  
  /**
   * Closes the connection to the agent
   */
  close(): void {
    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }
  }
}

/**
 * Creates a new agent client instance
 */
export const createAgentClient = (options: AgentClientOptions): BaseAgentClient => {
  return new AgentClient(options);
};