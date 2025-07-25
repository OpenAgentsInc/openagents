#!/usr/bin/env node

/**
 * Claude Code Session Inspector
 * 
 * This script inspects Claude Code session data to understand the exact message structure
 * for debugging and development purposes.
 */

const fs = require('fs');
const path = require('path');

function logStructure(obj, depth = 0, maxDepth = 3) {
  const indent = '  '.repeat(depth);
  
  if (depth > maxDepth) {
    console.log(`${indent}[Max depth reached]`);
    return;
  }
  
  if (obj === null) {
    console.log(`${indent}null`);
    return;
  }
  
  if (typeof obj !== 'object') {
    console.log(`${indent}${typeof obj}: ${JSON.stringify(obj)}`);
    return;
  }
  
  if (Array.isArray(obj)) {
    console.log(`${indent}Array[${obj.length}]:`);
    obj.slice(0, 3).forEach((item, index) => {
      console.log(`${indent}  [${index}]:`);
      logStructure(item, depth + 2, maxDepth);
    });
    if (obj.length > 3) {
      console.log(`${indent}  ... and ${obj.length - 3} more items`);
    }
  } else {
    console.log(`${indent}Object:`);
    Object.keys(obj).forEach(key => {
      console.log(`${indent}  ${key}:`);
      logStructure(obj[key], depth + 2, maxDepth);
    });
  }
}

function analyzeHookData(hookData) {
  console.log('=== Claude Code Hook Data Analysis ===\n');
  
  console.log('📋 Overview:');
  console.log(`Event: ${hookData.event || 'unknown'}`);
  console.log(`Timestamp: ${hookData.timestamp || 'unknown'}`);
  console.log(`Session ID: ${hookData.session?.id || hookData.sessionId || 'unknown'}`);
  console.log(`Project Path: ${hookData.session?.project_path || hookData.projectPath || 'unknown'}`);
  console.log('');
  
  console.log('🏗️ Full Data Structure:');
  logStructure(hookData);
  console.log('');
  
  if (hookData.session) {
    console.log('📱 Session Details:');
    logStructure(hookData.session, 0, 2);
    console.log('');
  }
  
  if (hookData.messages && Array.isArray(hookData.messages)) {
    console.log(`💬 Messages (${hookData.messages.length} total):`);
    hookData.messages.forEach((msg, index) => {
      console.log(`\n  Message ${index + 1}:`);
      console.log(`    ID: ${msg.id || 'unknown'}`);
      console.log(`    Type: ${msg.message_type || 'unknown'}`);
      console.log(`    Content: ${msg.content ? msg.content.substring(0, 100) + '...' : 'empty'}`);
      console.log(`    Timestamp: ${msg.timestamp || 'unknown'}`);
      
      if (msg.tool_info) {
        console.log(`    Tool Info:`);
        console.log(`      Name: ${msg.tool_info.tool_name || 'unknown'}`);
        console.log(`      Use ID: ${msg.tool_info.tool_use_id || 'unknown'}`);
        console.log(`      Input: ${JSON.stringify(msg.tool_info.input || {}).substring(0, 100)}...`);
        console.log(`      Output: ${msg.tool_info.output ? msg.tool_info.output.substring(0, 100) + '...' : 'none'}`);
      }
    });
    console.log('');
  }
  
  if (hookData.tool) {
    console.log('🔧 Tool Information:');
    logStructure(hookData.tool, 0, 2);
    console.log('');
  }
  
  console.log('📝 Convex Sync Mapping:');
  console.log('Session mapping:');
  console.log(`  sessionId: ${hookData.session?.id || hookData.sessionId || 'MISSING'}`);
  console.log(`  projectPath: ${hookData.session?.project_path || hookData.projectPath || 'MISSING'}`);
  console.log(`  createdBy: "desktop"`);
  console.log(`  status: "active"`);
  console.log('');
  
  if (hookData.messages) {
    console.log('Message mapping:');
    hookData.messages.forEach((msg, index) => {
      console.log(`  Message ${index + 1}:`);
      console.log(`    messageId: ${msg.id || `generated-${Date.now()}-${index}`}`);
      console.log(`    messageType: ${msg.message_type || 'MISSING'}`);
      console.log(`    content: ${msg.content ? '[HAS CONTENT]' : 'MISSING'}`);
      console.log(`    timestamp: ${msg.timestamp || 'MISSING'}`);
      console.log(`    toolInfo: ${msg.tool_info ? '[HAS TOOL INFO]' : 'none'}`);
    });
  }
}

async function main() {
  try {
    let inputData = '';
    
    if (process.argv.length > 2) {
      // Read from file if provided
      const filePath = process.argv[2];
      inputData = fs.readFileSync(filePath, 'utf8');
    } else {
      // Read from stdin
      process.stdin.setEncoding('utf8');
      
      for await (const chunk of process.stdin) {
        inputData += chunk;
      }
    }
    
    if (!inputData.trim()) {
      console.log('Usage:');
      console.log('  # From stdin:');
      console.log('  echo \'{"session":{"id":"test"}}\' | node inspect-session.js');
      console.log('');
      console.log('  # From file:');
      console.log('  node inspect-session.js hook-data.json');
      console.log('');
      console.log('  # Create sample data file:');
      console.log('  echo \'{"event":"UserPromptSubmit","session":{"id":"test-123","project_path":"/test"},"messages":[{"id":"msg-1","message_type":"user","content":"Hello","timestamp":"2024-01-01T00:00:00Z"}]}\' > sample-hook-data.json');
      process.exit(0);
    }
    
    let hookData;
    try {
      hookData = JSON.parse(inputData);
    } catch (error) {
      console.error('❌ Failed to parse JSON input:', error.message);
      console.error('Input data:', inputData.substring(0, 200) + '...');
      process.exit(1);
    }
    
    analyzeHookData(hookData);
    
  } catch (error) {
    console.error('❌ Error:', error.message);
    process.exit(1);
  }
}

main();