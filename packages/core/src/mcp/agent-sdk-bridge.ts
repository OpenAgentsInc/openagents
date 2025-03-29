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
  /** Path pattern for WebSocket endpoint (default: api/agent) */
  pathPattern?: string;
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
  private pendingMessages: Array<{ id: string, data: string }> = [];
  private connected = false;
  private connecting = false;
  private reconnectAttempts = 0;
  private messageId = 0;
  private connectionPromise: Promise<void> | null = null;
  private currentPatternIndex = 0; // Track which pattern we're currently trying
  
  agent: string;
  name: string;
  
  constructor(private options: AgentClientOptions) {
    this.agent = options.agent;
    this.name = options.name || 'default';
    
    // Initialize the connection immediately
    this.connectionPromise = this.connect();
  }
  
  /**
   * Establishes WebSocket connection to the agent server
   */
  private connect(): Promise<void> {
    if (this.connecting) {
      return this.connectionPromise || Promise.reject(new Error('Connection already in progress'));
    }
    
    this.connecting = true;
    const host = this.options.host || 'https://agents.openagents.com';
    
    // Prepare URLs to try
    let allPossibleUrls: string[] = [];
    
    try {
      // Handle URLs with or without protocol
      const hostWithProtocol = host.startsWith('http') ? host : `https://${host}`;
      const url = new URL(hostWithProtocol);
      const wsProtocol = url.protocol === 'https:' ? 'wss' : 'ws';
      
      // Define all possible patterns to try - THE CORRECT PATTERN IS THE THIRD ONE (/agents)
      // Cloudflare Agents SDK follows this pattern: wss://{hostname}/{namespace}/{id}
      // As documented in the SDK, the correct pattern is: /agents/{agent}/{instance}
      const allPatterns = [
        'agents',     // Primary pattern (namespace) - THIS IS THE CORRECT ONE 
        '',           // Direct path - in case there's no namespace
        'api/agents', // With api prefix - alternative pattern
        'api/agent',  // Original pattern attempt
        'ws',         // WebSocket-specific
        'worker',     // Worker-specific endpoint
        'agent'       // Direct agent endpoint
      ];
      
      // If a pattern is provided, try it first, then fall back to others on reconnect attempts
      const providedPattern = this.options.pathPattern;
      let possiblePatterns: string[];
      
      if (providedPattern) {
        // On first attempt, only try the provided pattern
        if (this.reconnectAttempts === 0) {
          possiblePatterns = [providedPattern];
        } else {
          // On reconnection attempts, try all patterns except the one that failed
          possiblePatterns = [
            ...allPatterns.filter(p => p !== providedPattern), 
            providedPattern // Put the provided pattern last since it failed before
          ];
        }
      } else {
        // No pattern provided, try all patterns
        possiblePatterns = allPatterns;
      }
      
      // Agent and instance names should be lowercase
      const agentName = this.agent.toLowerCase();
      const instanceName = this.name.toLowerCase();
      
      // Log warnings for uppercase names
      if (this.agent !== agentName) {
        console.warn(`Agent names should be lowercase. Converting ${this.agent} to ${agentName}.`);
      }
      
      if (this.name !== instanceName) {
        console.warn(`Instance names should be lowercase. Converting ${this.name} to ${instanceName}.`);
      }
      
      // Create all possible URLs
      allPossibleUrls = possiblePatterns.map(pattern => {
        const path = pattern ? `${pattern}/` : '';
        return `${wsProtocol}://${url.host}/${path}${agentName}/${instanceName}`;
      });
    } catch (e) {
      // Fallback for invalid URLs
      const wsProtocol = host.startsWith('https') ? 'wss' : 'ws';
      // Use lowercase agent and instance names in the fallback URL too
      const agentName = this.agent.toLowerCase();
      const instanceName = this.name.toLowerCase();
      
      // Add some fallback patterns
      allPossibleUrls = [
        `${wsProtocol}://${host.replace(/^https?:\/\//, '')}/api/agent/${agentName}/${instanceName}`,
        `${wsProtocol}://${host.replace(/^https?:\/\//, '')}/api/agents/${agentName}/${instanceName}`,
        `${wsProtocol}://${host.replace(/^https?:\/\//, '')}/agents/${agentName}/${instanceName}`,
        `${wsProtocol}://${host.replace(/^https?:\/\//, '')}/${agentName}/${instanceName}`
      ];
    }
    
    // Return a promise that resolves when any of the URLs connects successfully
    return new Promise<void>((finalResolve, finalReject) => {
      // Reset pattern index
      this.currentPatternIndex = 0;
      
      console.log(`Starting connection attempts with ${allPossibleUrls.length} possible URL patterns`);
      
      // Try all URLs one by one until one works
      const tryNextUrl = (index: number) => {
        if (index >= allPossibleUrls.length) {
          // We've tried all URLs and none worked
          this.connecting = false;
          finalReject(new Error(
            `Failed to connect to agent ${this.agent}/${this.name} after trying all URL patterns:\n` +
            allPossibleUrls.join('\n')
          ));
          return;
        }
        
        // Store which pattern we're currently trying
        this.currentPatternIndex = index;
        
        const currentUrl = allPossibleUrls[index];
        console.log(`Connecting to agent at ${currentUrl} (attempt ${index + 1}/${allPossibleUrls.length})`);
        
        try {
          // Try to connect with this URL
          console.log(`🔌 Initiating WebSocket connection to ${currentUrl}`);
          this.socket = new WebSocket(currentUrl);
          
          // Set a timeout for this connection attempt
          const connectionTimeout = setTimeout(() => {
            if (!this.socket) return;
            
            if (this.socket.readyState !== WebSocket.OPEN) {
              console.log(`Connection attempt ${index + 1} timed out, trying next URL pattern...`);
              this.socket.close();
              // Try the next URL
              tryNextUrl(index + 1);
            }
          }, 5000); // Shorter timeout for each individual attempt
          
          // Handle successful connection
          this.socket.onopen = () => {
            console.log(`✅ Connected to agent ${this.agent}/${this.name} using pattern ${index + 1}`);
            clearTimeout(connectionTimeout);
            this.connected = true;
            this.connecting = false;
            this.reconnectAttempts = 0;
            
            // Store the successful URL pattern for future reconnects
            const matchedPattern = currentUrl.match(/wss?:\/\/[^\/]+\/([^\/]*)/)?.[1] || '';
            this.options.pathPattern = matchedPattern;
            console.log(`Using path pattern '${matchedPattern}' for future connections`);
            
            // Notify connection is ready via an initial handshake
            try {
              this.socket?.send(JSON.stringify({
                type: 'handshake',
                agent: this.agent,
                name: this.name,
                version: '1.0.0'
              }));
              
              // Send any pending messages
              this.sendPendingMessages();
              
              // Resolve the final promise
              finalResolve();
            } catch (error) {
              console.warn('Failed to send handshake message:', error);
              finalReject(error);
            }
          };
          
          // Handle errors and retries
          this.socket.onerror = (event) => {
            const errorInfo = {
              type: 'websocket_error',
              message: 'WebSocket connection error',
              url: currentUrl,
              readyState: this.socket ? ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'][this.socket.readyState] : 'UNKNOWN',
              timestamp: new Date().toISOString(),
              attempt: index + 1,
              totalAttempts: allPossibleUrls.length
            };
            
            console.log(`❌ WebSocket error on attempt ${index + 1}: ${errorInfo.readyState}`);
            
            // Don't reject yet - wait for close event or timeout to try next URL
          };
          
          // Close handler - try next URL if this one fails
          this.socket.onclose = (event) => {
            // Clear the timeout to avoid duplicate calls to tryNextUrl
            clearTimeout(connectionTimeout);
            
            // Only process if we're still attempting this URL (might have moved on already)
            if (this.socket && this.socket.url === currentUrl) {
              // Log the close with code info
              let closeMessage = 'Unknown';
              
              // Translate common close codes
              switch (event.code) {
                case 1000: closeMessage = 'Normal closure'; break;
                case 1001: closeMessage = 'Going away'; break;
                case 1002: closeMessage = 'Protocol error'; break;
                case 1003: closeMessage = 'Unsupported data'; break;
                case 1005: closeMessage = 'No status received'; break;
                case 1006: closeMessage = 'Abnormal closure'; break;
                case 1007: closeMessage = 'Invalid frame payload data'; break;
                case 1008: closeMessage = 'Policy violation'; break;
                case 1009: closeMessage = 'Message too big'; break;
                case 1010: closeMessage = 'Mandatory extension'; break;
                case 1011: closeMessage = 'Internal server error'; break;
                case 1012: closeMessage = 'Service restart'; break;
                case 1013: closeMessage = 'Try again later'; break;
                case 1014: closeMessage = 'Bad gateway'; break;
                case 1015: closeMessage = 'TLS handshake'; break;
              }
              
              console.log(`Connection to ${currentUrl} closed with code ${event.code} (${closeMessage})`);
              
              // If this wasn't a normal closure, try the next URL
              if (event.code !== 1000 && event.code !== 1001) {
                console.log(`URL pattern ${index + 1} failed, trying next pattern...`);
                // Try the next URL pattern
                tryNextUrl(index + 1);
              }
            }
          };
          
          this.socket.onmessage = (event) => {
            try {
              // Log receipt of message
              console.log(`⬅️ Received message from agent ${this.agent}/${this.name} (pattern ${index + 1})`);
              
              // Try to parse the message as JSON
              let data;
              try {
                data = JSON.parse(event.data);
              } catch (parseError) {
                console.error('Failed to parse message as JSON:', event.data);
                console.error('Parse error:', parseError);
                return;
              }
              
              // Validate the message structure
              if (!data || typeof data !== 'object') {
                console.error('Invalid message format: message is not an object', data);
                return;
              }
              
              if (!data.type) {
                console.error('Invalid message format: missing message type', data);
                return;
              }
              
              // Handle state updates
              if (data.type === 'state') {
                if (data.state === undefined) {
                  console.warn('Received state update message with undefined state:', data);
                } else {
                  console.log(`Received state update from agent ${this.agent}/${this.name}`);
                  this.options.onStateUpdate?.(data.state, 'server');
                }
                return;
              }
              
              // Handle RPC responses
              if (data.type === 'response') {
                // Validate response format
                if (!data.id) {
                  console.error('Invalid response: missing response ID', data);
                  return;
                }
                
                if (!this.messageQueue.has(data.id)) {
                  console.warn(`Received response for unknown request ID: ${data.id}`, data);
                  return;
                }
                
                const { resolve, reject } = this.messageQueue.get(data.id)!;
                
                if (data.error) {
                  // Enhanced error handling for server-side errors
                  const serverError = new Error(
                    typeof data.error === 'string' 
                      ? data.error 
                      : JSON.stringify(data.error)
                  );
                  
                  // Add server error details if available
                  if (typeof data.error === 'object' && data.error !== null) {
                    Object.assign(serverError, {
                      serverError: data.error,
                      code: 'SERVER_ERROR',
                      context: {
                        agent: this.agent,
                        name: this.name,
                        requestId: data.id,
                        timestamp: new Date().toISOString()
                      }
                    });
                  } else {
                    Object.assign(serverError, {
                      code: 'SERVER_ERROR',
                      context: {
                        agent: this.agent,
                        name: this.name,
                        requestId: data.id,
                        timestamp: new Date().toISOString()
                      }
                    });
                  }
                  
                  console.error(`Server returned error for request ${data.id}:`, serverError);
                  reject(serverError);
                } else {
                  // Log successful response
                  console.log(`Received successful response for request ${data.id}`);
                  resolve(data.result);
                }
                
                this.messageQueue.delete(data.id);
                return;
              }
              
              // Handle unknown message types
              console.warn(`Received unknown message type: ${data.type}`, data);
            } catch (error) {
              // General error handling for message processing
              console.error('Error processing message:', error);
              
              // Log the message data that caused the error if available
              try {
                console.error('Message data:', typeof event.data === 'string' 
                  ? (event.data.length > 1000 ? event.data.substring(0, 1000) + '...' : event.data)
                  : '[Non-string message]');
              } catch (logError) {
                console.error('Failed to log message data');
              }
            }
          };
        } catch (setupError) {
          console.error('Error setting up WebSocket connection:', setupError);
          // Try the next URL pattern
          tryNextUrl(index + 1);
        }
      };
      
      // Start the connection process by trying the first URL
      tryNextUrl(0);
    });
  }
  
  /**
   * Send any pending messages that were queued before the connection was established
   */
  private sendPendingMessages(): void {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return;
    }
    
    console.log(`Sending ${this.pendingMessages.length} pending messages`);
    
    while (this.pendingMessages.length > 0) {
      const message = this.pendingMessages.shift();
      if (message) {
        try {
          this.socket.send(message.data);
        } catch (error) {
          console.error(`Failed to send pending message ${message.id}:`, error);
          // Find and reject the corresponding promise if it exists
          if (this.messageQueue.has(message.id)) {
            const { reject } = this.messageQueue.get(message.id)!;
            reject(error);
            this.messageQueue.delete(message.id);
          }
        }
      }
    }
  }
  
  /**
   * Calls a method on the agent
   */
  async call<T = unknown>(method: string, args: unknown[] = []): Promise<T> {
    // If we're still connecting, wait for the connection to complete
    if (this.connecting && this.connectionPromise) {
      try {
        console.log(`Waiting for connection to complete before calling ${method}...`);
        await this.connectionPromise;
      } catch (error) {
        // Create a more descriptive error object
        const connectionError = new Error(
          `Failed to connect to agent before calling method '${method}': ${error instanceof Error ? error.message : String(error)}`
        );
        // Copy any properties from the original error
        if (error instanceof Error) {
          Object.assign(connectionError, error);
        }
        // Add context about the method call
        Object.assign(connectionError, { 
          context: {
            method,
            args,
            agent: this.agent,
            name: this.name
          }
        });
        throw connectionError;
      }
    }
    
    // More robust check for connection status with detailed errors
    if (!this.socket) {
      const error = new Error(`Cannot call method '${method}': socket not initialized`);
      Object.assign(error, {
        code: 'SOCKET_NOT_INITIALIZED',
        context: {
          method,
          args,
          agent: this.agent,
          name: this.name
        }
      });
      throw error;
    }

    if (!this.connected) {
      const error = new Error(`Cannot call method '${method}': connection not established`);
      Object.assign(error, {
        code: 'CONNECTION_NOT_ESTABLISHED',
        context: {
          method,
          args,
          agent: this.agent,
          name: this.name
        }
      });
      throw error;
    }

    if (this.socket.readyState !== WebSocket.OPEN) {
      // Check specific socket state and provide more detailed error
      const states = ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'];
      const error = new Error(
        `Cannot call method '${method}': socket in ${states[this.socket.readyState]} state`
      );
      Object.assign(error, {
        code: 'INVALID_SOCKET_STATE',
        socketState: states[this.socket.readyState],
        context: {
          method,
          args,
          agent: this.agent,
          name: this.name
        }
      });
      throw error;
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
          
          // Create a detailed timeout error
          const timeoutError = new Error(
            `Request timed out after 30s when calling method '${method}' on agent ${this.agent}/${this.name}`
          );
          Object.assign(timeoutError, {
            code: 'REQUEST_TIMEOUT',
            context: {
              method,
              args,
              agent: this.agent,
              name: this.name,
              requestId: id,
              timestamp: new Date().toISOString()
            }
          });
          reject(timeoutError);
        }
      }, 30000);
      
      // Send the message
      try {
        if (this.socket && this.socket.readyState === WebSocket.OPEN) {
          this.socket.send(JSON.stringify(message));
          // Log outgoing message for debugging
          console.log(`➡️ Sent message to agent ${this.agent}/${this.name}: ${method}`, 
            args.length > 0 ? `with ${args.length} arguments` : 'with no arguments');
        } else {
          // If socket isn't open yet, queue the message to send when the connection is established
          if (this.connecting && this.connectionPromise) {
            console.log(`Queueing message for ${method} until connection is established`);
            this.pendingMessages.push({ id, data: JSON.stringify(message) });
          } else {
            // Create detailed error for socket state issues
            const socketError = new Error(
              `Cannot send message for method '${method}': socket not available or in wrong state ` +
              `(${this.socket ? ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'][this.socket.readyState] : 'null'})`
            );
            Object.assign(socketError, {
              code: 'SOCKET_STATE_ERROR',
              socketState: this.socket ? this.socket.readyState : -1,
              context: {
                method,
                args,
                agent: this.agent,
                name: this.name,
                requestId: id,
                connecting: this.connecting,
                hasConnectionPromise: !!this.connectionPromise,
                timestamp: new Date().toISOString()
              }
            });
            throw socketError;
          }
        }
      } catch (error) {
        clearTimeout(timeout);
        this.messageQueue.delete(id);
        
        // Create a detailed send error
        const sendError = new Error(
          `Failed to send message for method '${method}': ${error instanceof Error ? error.message : String(error)}`
        );
        
        // Copy any properties from the original error
        if (error instanceof Error) {
          Object.assign(sendError, error);
        }
        
        // Add context about the method call
        Object.assign(sendError, {
          code: 'MESSAGE_SEND_ERROR',
          context: {
            method,
            args,
            agent: this.agent,
            name: this.name,
            requestId: id,
            timestamp: new Date().toISOString()
          }
        });
        
        reject(sendError);
      }
    });
  }
  
  /**
   * Updates the agent's state
   */
  async setState(state: any): Promise<void> {
    // If we're still connecting, wait for the connection to complete
    if (this.connecting && this.connectionPromise) {
      try {
        console.log(`Waiting for connection to complete before setting state...`);
        await this.connectionPromise;
      } catch (error) {
        console.warn(`Cannot set state: connection failed: ${error instanceof Error ? error.message : String(error)}`);
        return;
      }
    }
    
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
      } else if (this.connecting && this.connectionPromise) {
        // Queue the state update to be sent when the connection is established
        console.log(`Queueing state update until connection is established`);
        const messageId = `state-${Date.now()}`;
        this.pendingMessages.push({ 
          id: messageId, 
          data: JSON.stringify({
            type: 'setState',
            state
          })
        });
        
        // Still notify of the state change locally
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