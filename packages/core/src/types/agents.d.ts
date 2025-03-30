// Type definitions for Cloudflare Agents SDK

declare module 'agents/client' {
  import { PartySocket, PartySocketOptions } from 'partysocket';

  /**
   * Options for creating an AgentClient
   */
  export type AgentClientOptions<State = unknown> = Omit<
    PartySocketOptions,
    'party' | 'room'
  > & {
    /** Name of the agent to connect to */
    agent: string;
    /** Name of the specific Agent instance */
    name?: string;
    /** Called when the Agent's state is updated */
    onStateUpdate?: (state: State, source: "server" | "client") => void;
    /** Host URL for the agent server */
    host?: string;
    /** Headers for authentication */
    headers?: Record<string, string>;
  };

  /**
   * Options for streaming RPC calls
   */
  export type StreamOptions = {
    /** Called when a chunk of data is received */
    onChunk?: (chunk: unknown) => void;
    /** Called when the stream ends */
    onDone?: (finalChunk: unknown) => void;
    /** Called when an error occurs */
    onError?: (error: string) => void;
  };

  /**
   * WebSocket client for connecting to an Agent
   */
  export class AgentClient<State = unknown> extends PartySocket {
    agent: string;
    name: string;
    constructor(options: AgentClientOptions<State>);
    setState(state: State): void;
    
    /**
     * Call a method on the Agent
     * @param method Name of the method to call
     * @param args Arguments to pass to the method
     * @param streamOptions Options for handling streaming responses
     * @returns Promise that resolves with the method's return value
     */
    call<T = unknown>(
      method: string,
      args?: unknown[],
      streamOptions?: StreamOptions
    ): Promise<T>;
    
    /**
     * Close the connection
     */
    close(): void;
  }
}

// Add additional modules as needed
declare module 'agents/react' {
  import { PartySocket } from "partysocket";
  import { AgentClient } from "agents/client";
  
  /**
   * Options for the useAgent hook
   * @template State Type of the Agent's state
   */
  export type UseAgentOptions<State = unknown> = {
    /** Name of the agent to connect to */
    agent: string;
    /** Name of the specific Agent instance */
    name?: string;
    /** Called when the Agent's state is updated */
    onStateUpdate?: (state: State, source: "server" | "client") => void;
    /** URL for the agent */
    host?: string;
  };
  
  /**
   * React hook for connecting to an Agent
   * @template State Type of the Agent's state
   * @param options Connection options
   * @returns WebSocket connection with setState and call methods
   */
  export function useAgent<State = unknown>(
    options: UseAgentOptions<State>
  ): PartySocket & {
    agent: string;
    name: string;
    setState: (state: State) => void;
    call: <T = unknown>(
      method: string,
      args?: unknown[],
      streamOptions?: any
    ) => Promise<T>;
  };
}

// Add a module declaration for partysocket to satisfy the dependency
declare module 'partysocket' {
  export class PartySocket extends WebSocket {
    constructor(options: PartySocketOptions);
  }
  
  export interface PartySocketOptions {
    party?: string;
    room?: string;
    host?: string;
    retry?: boolean;
    debug?: boolean;
    headers?: Record<string, string>;
    [key: string]: any;
  }
}