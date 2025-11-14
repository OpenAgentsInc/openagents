#!/usr/bin/env bun
/**
 * ACP-Supabase Bridge
 *
 * Standalone script that:
 * - Listens to Supabase Realtime for new user messages
 * - Manages Claude Code and Codex ACP sessions
 * - Streams agent responses back to Supabase
 *
 * Usage:
 *   bun scripts/acp-supabase-bridge.ts
 *
 * Environment variables:
 *   SUPABASE_URL - Your Supabase project URL
 *   SUPABASE_ANON_KEY - Your Supabase anon key
 *   WORKING_DIR - Default working directory for sessions (optional)
 */

import { createClient, RealtimeChannel } from '@supabase/supabase-js';
import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';
import * as readline from 'readline';

// Types based on @agentclientprotocol/sdk
interface ACPSessionId {
  sessionId: string;
}

interface ACPPromptRequest {
  session_id: ACPSessionId;
  prompt: Array<{ type: 'text'; text: string }>;
  meta?: Record<string, unknown>;
}

interface ACPPromptResponse {
  request_id: string;
}

interface ACPSessionNewRequest {
  cwd: string;
}

interface ACPSessionNewResponse extends ACPSessionId {}

interface ACPInitializeRequest {
  protocol_version: string;
  capabilities: Record<string, unknown>;
}

interface ACPInitializeResponse {
  protocol_version: string;
  capabilities: Record<string, unknown>;
}

interface ACPSessionUpdate {
  session_id: ACPSessionId;
  update: {
    type: string;
    [key: string]: unknown;
  };
}

interface JSONRPCRequest {
  jsonrpc: '2.0';
  id: number | string;
  method: string;
  params: unknown;
}

interface JSONRPCResponse {
  jsonrpc: '2.0';
  id: number | string;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

interface JSONRPCNotification {
  jsonrpc: '2.0';
  method: string;
  params: unknown;
}

// Database types
interface Session {
  id: string;
  agent_type: 'claude-code' | 'codex';
  acp_session_id: string;
  cwd: string;
  status: 'active' | 'completed' | 'error';
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

interface Message {
  id: string;
  session_id: string;
  role: 'user' | 'assistant';
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

// Configuration
const SUPABASE_URL = process.env.SUPABASE_URL;
const SUPABASE_ANON_KEY = process.env.SUPABASE_ANON_KEY;
const DEFAULT_WORKING_DIR = process.env.WORKING_DIR || process.cwd();

if (!SUPABASE_URL || !SUPABASE_ANON_KEY) {
  console.error('Error: SUPABASE_URL and SUPABASE_ANON_KEY must be set');
  process.exit(1);
}

// Initialize Supabase client
const supabase = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
  realtime: {
    params: {
      eventsPerSecond: 10,
    },
  },
});

/**
 * ACP Client - manages communication with an ACP agent process
 */
class ACPClient {
  private process: ChildProcess;
  private requestId = 0;
  private pendingRequests = new Map<number, (result: unknown) => void>();
  private updateHandlers: Array<(update: ACPSessionUpdate) => void> = [];
  private rl: readline.Interface;

  constructor(
    command: string,
    args: string[],
    env: Record<string, string> = {}
  ) {
    console.log(`[ACPClient] Spawning: ${command} ${args.join(' ')}`);

    this.process = spawn(command, args, {
      stdio: ['pipe', 'pipe', 'inherit'],
      env: { ...process.env, ...env },
    });

    // Set up readline for line-by-line JSON-RPC parsing
    this.rl = readline.createInterface({
      input: this.process.stdout!,
      crlfDelay: Infinity,
    });

    this.rl.on('line', (line) => {
      try {
        const msg = JSON.parse(line);
        this.handleMessage(msg);
      } catch (err) {
        console.error('[ACPClient] Failed to parse JSON-RPC message:', line, err);
      }
    });

    this.process.on('exit', (code, signal) => {
      console.log(`[ACPClient] Process exited: code=${code}, signal=${signal}`);
    });

    this.process.on('error', (err) => {
      console.error('[ACPClient] Process error:', err);
    });
  }

  private handleMessage(msg: JSONRPCResponse | JSONRPCNotification) {
    if ('id' in msg) {
      // Response to a request
      const handler = this.pendingRequests.get(msg.id as number);
      if (handler) {
        this.pendingRequests.delete(msg.id as number);
        if (msg.error) {
          console.error('[ACPClient] RPC error:', msg.error);
          handler(null);
        } else {
          handler(msg.result);
        }
      }
    } else if (msg.method === 'session/update') {
      // Notification
      const update = msg.params as ACPSessionUpdate;
      this.updateHandlers.forEach((h) => h(update));
    }
  }

  private sendRequest<T>(method: string, params: unknown): Promise<T> {
    return new Promise((resolve) => {
      const id = ++this.requestId;
      const req: JSONRPCRequest = {
        jsonrpc: '2.0',
        id,
        method,
        params,
      };

      this.pendingRequests.set(id, resolve as (result: unknown) => void);
      this.process.stdin!.write(JSON.stringify(req) + '\n');
    });
  }

  async initialize(): Promise<ACPInitializeResponse> {
    const req: ACPInitializeRequest = {
      protocol_version: '2024-11-05',
      capabilities: {},
    };
    return this.sendRequest<ACPInitializeResponse>('initialize', req);
  }

  async newSession(cwd: string): Promise<ACPSessionId> {
    const req: ACPSessionNewRequest = { cwd };
    return this.sendRequest<ACPSessionNewResponse>('session/new', req);
  }

  async prompt(sessionId: ACPSessionId, text: string): Promise<ACPPromptResponse> {
    const req: ACPPromptRequest = {
      session_id: sessionId,
      prompt: [{ type: 'text', text }],
      meta: {},
    };
    return this.sendRequest<ACPPromptResponse>('session/prompt', req);
  }

  onUpdate(handler: (update: ACPSessionUpdate) => void) {
    this.updateHandlers.push(handler);
  }

  destroy() {
    this.process.kill();
    this.rl.close();
  }
}

/**
 * Session Manager - tracks active ACP sessions
 */
class SessionManager {
  private clients = new Map<string, ACPClient>();

  async getOrCreateSession(
    dbSessionId: string,
    agentType: 'claude-code' | 'codex',
    cwd: string
  ): Promise<{ client: ACPClient; acpSessionId: ACPSessionId }> {
    // Check if we already have an active client for this session
    let client = this.clients.get(dbSessionId);

    if (!client) {
      // Spawn new ACP client
      const { command, args } = this.resolveAgentCommand(agentType);
      client = new ACPClient(command, args);

      // Initialize ACP protocol
      await client.initialize();

      // Create new ACP session
      const acpSessionId = await client.newSession(cwd);

      // Store client
      this.clients.set(dbSessionId, client);

      // Set up update handler
      client.onUpdate(async (update) => {
        await this.handleSessionUpdate(dbSessionId, update);
      });

      // Update database with ACP session ID
      await supabase
        .from('sessions')
        .update({ acp_session_id: acpSessionId.sessionId })
        .eq('id', dbSessionId);

      console.log(`[SessionManager] Created session: db=${dbSessionId}, acp=${acpSessionId.sessionId}`);

      return { client, acpSessionId };
    }

    // Get existing ACP session ID from database
    const { data: session } = await supabase
      .from('sessions')
      .select('acp_session_id')
      .eq('id', dbSessionId)
      .single();

    const acpSessionId: ACPSessionId = {
      sessionId: session?.acp_session_id || '',
    };

    return { client, acpSessionId };
  }

  private resolveAgentCommand(agentType: 'claude-code' | 'codex'): { command: string; args: string[] } {
    if (agentType === 'claude-code') {
      // Assume claude-code-acp is available via npm package or local path
      // Adjust path as needed for your setup
      const claudeCodePath = path.resolve(__dirname, '../packages/claude-code-acp/index.ts');
      return {
        command: 'tsx',
        args: [claudeCodePath],
      };
    } else {
      // Codex binary - adjust path as needed
      return {
        command: 'codex-acp',
        args: [],
      };
    }
  }

  private async handleSessionUpdate(dbSessionId: string, update: ACPSessionUpdate) {
    const updateType = update.update.type;
    console.log(`[SessionManager] Session update: ${dbSessionId} - ${updateType}`);

    // Map ACP update to message content
    let content = '';
    let metadata: Record<string, unknown> = { type: updateType, ...update.update };

    switch (updateType) {
      case 'AgentMessageChunk':
        content = (update.update as any).text || '';
        break;
      case 'AgentThoughtChunk':
        content = (update.update as any).thought || '';
        break;
      case 'ToolCall':
        content = `[Tool: ${(update.update as any).tool_name}]`;
        break;
      case 'ToolResult':
        content = `[Result: ${JSON.stringify((update.update as any).result)}]`;
        break;
      default:
        content = JSON.stringify(update.update);
    }

    // Write to Supabase
    if (content || Object.keys(metadata).length > 0) {
      await supabase.from('messages').insert({
        session_id: dbSessionId,
        role: 'assistant',
        content,
        metadata,
      });
    }
  }

  async sendPrompt(dbSessionId: string, text: string) {
    const { data: session } = await supabase
      .from('sessions')
      .select('*')
      .eq('id', dbSessionId)
      .single();

    if (!session) {
      throw new Error(`Session not found: ${dbSessionId}`);
    }

    const { client, acpSessionId } = await this.getOrCreateSession(
      dbSessionId,
      session.agent_type,
      session.cwd
    );

    await client.prompt(acpSessionId, text);
  }

  destroy() {
    for (const client of this.clients.values()) {
      client.destroy();
    }
    this.clients.clear();
  }
}

/**
 * Main application
 */
class ACPSupabaseBridge {
  private sessionManager = new SessionManager();
  private channel: RealtimeChannel | null = null;

  async start() {
    console.log('[Bridge] Starting ACP-Supabase Bridge...');

    // Subscribe to messages table for real-time updates
    this.channel = supabase
      .channel('messages')
      .on(
        'postgres_changes',
        {
          event: 'INSERT',
          schema: 'public',
          table: 'messages',
          filter: 'role=eq.user',
        },
        async (payload) => {
          await this.handleNewUserMessage(payload.new as Message);
        }
      )
      .subscribe((status) => {
        console.log(`[Bridge] Realtime subscription status: ${status}`);
      });

    console.log('[Bridge] Listening for new messages...');
    console.log('[Bridge] Press Ctrl+C to stop');
  }

  private async handleNewUserMessage(message: Message) {
    console.log(`[Bridge] New user message: ${message.id} (session: ${message.session_id})`);

    try {
      // Get or create session
      const { data: session } = await supabase
        .from('sessions')
        .select('*')
        .eq('id', message.session_id)
        .single();

      if (!session) {
        console.error(`[Bridge] Session not found: ${message.session_id}`);
        return;
      }

      // Send prompt to ACP agent
      await this.sessionManager.sendPrompt(message.session_id, message.content);
    } catch (err) {
      console.error('[Bridge] Error handling message:', err);

      // Update session status to error
      await supabase
        .from('sessions')
        .update({
          status: 'error',
          metadata: { error: String(err) },
        })
        .eq('id', message.session_id);
    }
  }

  async stop() {
    console.log('[Bridge] Stopping...');

    if (this.channel) {
      await supabase.removeChannel(this.channel);
    }

    this.sessionManager.destroy();
    console.log('[Bridge] Stopped');
  }
}

// Run the bridge
const bridge = new ACPSupabaseBridge();

// Handle graceful shutdown
process.on('SIGINT', async () => {
  console.log('\n[Bridge] Received SIGINT, shutting down...');
  await bridge.stop();
  process.exit(0);
});

process.on('SIGTERM', async () => {
  console.log('\n[Bridge] Received SIGTERM, shutting down...');
  await bridge.stop();
  process.exit(0);
});

// Start the bridge
bridge.start().catch((err) => {
  console.error('[Bridge] Fatal error:', err);
  process.exit(1);
});
