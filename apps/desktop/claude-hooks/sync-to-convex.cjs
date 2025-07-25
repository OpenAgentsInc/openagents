#!/usr/bin/env node

/**
 * Claude Code Hook Script for Convex Sync
 * 
 * This script receives Claude Code hook data via stdin and syncs it to Convex backend.
 * Triggered by Claude Code hooks for real-time session synchronization.
 */

const https = require('https');
const fs = require('fs');
const path = require('path');

// Configuration
const CONVEX_DEPLOYMENT_URL = process.env.VITE_CONVEX_URL || process.env.CONVEX_URL;
const DEBUG = process.env.CLAUDE_HOOK_DEBUG === 'true';

function debug(message, data = null) {
  if (DEBUG) {
    console.error(`[Claude Hook] ${message}`);
    if (data) {
      console.error(JSON.stringify(data, null, 2));
    }
  }
}

function logToFile(message, data = null) {
  const logDir = path.join(process.env.HOME || '/tmp', '.claude', 'hook-logs');
  try {
    if (!fs.existsSync(logDir)) {
      fs.mkdirSync(logDir, { recursive: true });
    }
    const logFile = path.join(logDir, `convex-sync-${new Date().toISOString().split('T')[0]}.log`);
    const timestamp = new Date().toISOString();
    const logEntry = `${timestamp} - ${message}${data ? '\n' + JSON.stringify(data, null, 2) : ''}\n`;
    fs.appendFileSync(logFile, logEntry);
  } catch (error) {
    console.error('Failed to write to log file:', error.message);
  }
}

async function makeConvexRequest(functionPath, args) {
  return new Promise((resolve, reject) => {
    if (!CONVEX_DEPLOYMENT_URL) {
      reject(new Error('Convex deployment URL not configured. Set VITE_CONVEX_URL or CONVEX_URL environment variable.'));
      return;
    }
    
    // Validate URL format
    let baseUrl;
    try {
      baseUrl = new URL(CONVEX_DEPLOYMENT_URL);
    } catch (error) {
      reject(new Error(`Invalid Convex deployment URL: ${CONVEX_DEPLOYMENT_URL}`));
      return;
    }
    
    // Simplify URL construction logic
    const isMutation = functionPath.startsWith('mutation:');
    const endpoint = isMutation ? '/api/mutation' : '/api/query';
    const url = new URL(endpoint, baseUrl);
    
    if (isMutation) {
      functionPath = functionPath.replace('mutation:', '');
    }
    
    const postData = JSON.stringify({
      path: functionPath,
      args: args,
      format: 'json',
    });
    
    const options = {
      hostname: url.hostname,
      port: url.port || 443,
      path: url.pathname + url.search,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(postData),
      },
    };
    
    debug(`Making Convex request to ${functionPath}`, args);
    
    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', (chunk) => {
        data += chunk;
      });
      
      res.on('end', () => {
        try {
          const result = JSON.parse(data);
          if (res.statusCode >= 200 && res.statusCode < 300) {
            resolve(result);
          } else {
            reject(new Error(`HTTP ${res.statusCode}: ${data}`));
          }
        } catch (error) {
          reject(new Error(`Failed to parse response: ${error.message}`));
        }
      });
    });
    
    req.on('error', (error) => {
      reject(error);
    });
    
    req.write(postData);
    req.end();
  });
}

async function syncSessionToConvex(hookData) {
  try {
    debug('Syncing session to Convex', hookData);
    
    // Extract session information
    const sessionId = hookData.session?.id || hookData.sessionId;
    const projectPath = hookData.session?.project_path || hookData.projectPath || process.cwd();
    
    if (!sessionId) {
      throw new Error('No session ID found in hook data');
    }
    
    // Create or update session in Convex
    await makeConvexRequest('mutation:claude.createClaudeSession', {
      sessionId: sessionId,
      projectPath: projectPath,
      createdBy: 'desktop',
      title: `Desktop Session - ${path.basename(projectPath)}`,
      metadata: {
        workingDirectory: process.cwd(),
        model: hookData.session?.model,
        systemPrompt: hookData.session?.system_prompt,
      },
    });
    
    // Sync messages if available
    if (hookData.messages && Array.isArray(hookData.messages)) {
      // Validate message structure and filter out invalid messages
      const validMessages = hookData.messages.filter((msg, index) => {
        if (!msg || typeof msg !== 'object') {
          debug(`Skipping invalid message at index ${index}: not an object`);
          return false;
        }
        if (!msg.message_type && !msg.messageType) {
          debug(`Skipping message at index ${index}: missing message_type`);
          return false;
        }
        if (msg.content === undefined || msg.content === null) {
          debug(`Skipping message at index ${index}: missing content`);
          return false;
        }
        return true;
      });

      const messages = validMessages.map((msg, index) => ({
        messageId: msg.id || `${msg.message_type || msg.messageType}-${Date.now()}-${Math.random().toString(36).substring(2)}`,
        messageType: msg.message_type || msg.messageType,
        content: String(msg.content),
        timestamp: msg.timestamp || new Date().toISOString(),
        toolInfo: msg.tool_info ? {
          toolName: msg.tool_info.tool_name,
          toolUseId: msg.tool_info.tool_use_id,
          input: msg.tool_info.input,
          output: msg.tool_info.output,
        } : undefined,
        metadata: {
          hookEvent: hookData.event,
          hookTimestamp: hookData.timestamp,
        },
      }));
      
      if (messages.length > 0) {
        await makeConvexRequest('mutation:claude.batchAddMessages', {
          sessionId: sessionId,
          messages: messages,
        });
      }
    }
    
    // Update sync status
    await makeConvexRequest('mutation:claude.updateSyncStatus', {
      sessionId: sessionId,
      desktopLastSeen: Date.now(),
    });
    
    debug('Successfully synced to Convex');
    logToFile('Sync successful', { sessionId, messageCount: hookData.messages?.length || 0 });
    
    return { success: true };
    
  } catch (error) {
    debug('Failed to sync to Convex', error);
    logToFile('Sync failed', { error: error.message, hookData });
    throw error;
  }
}

async function main() {
  try {
    // Read hook data from stdin
    let inputData = '';
    
    // Set up stdin reading
    process.stdin.setEncoding('utf8');
    
    for await (const chunk of process.stdin) {
      inputData += chunk;
    }
    
    if (!inputData.trim()) {
      debug('No input data received');
      process.exit(0);
    }
    
    debug('Received hook data:', inputData);
    
    let hookData;
    try {
      hookData = JSON.parse(inputData);
    } catch (error) {
      console.error('Failed to parse input JSON:', error.message);
      logToFile('Failed to parse input JSON', { error: error.message, input: inputData });
      process.exit(1);
    }
    
    // Sync to Convex
    await syncSessionToConvex(hookData);
    
    // Success
    process.exit(0);
    
  } catch (error) {
    console.error('Hook execution failed:', error.message);
    logToFile('Hook execution failed', { error: error.message });
    process.exit(2); // Exit code 2 indicates blocking error
  }
}

// Handle unhandled errors
process.on('uncaughtException', (error) => {
  console.error('Uncaught exception:', error.message);
  logToFile('Uncaught exception', { error: error.message, stack: error.stack });
  process.exit(2);
});

process.on('unhandledRejection', (error) => {
  console.error('Unhandled rejection:', error.message);
  logToFile('Unhandled rejection', { error: error.message });
  process.exit(2);
});

// Run the script
main();