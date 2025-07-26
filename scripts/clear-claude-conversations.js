#!/usr/bin/env node

/**
 * Script to delete all Claude conversations and related data from Convex
 * This will clear all sessions, messages, and sync status entries
 */

const { spawn } = require('child_process');
const path = require('path');

// Add the cleanup mutation to convex/claude.ts
const cleanupMutation = `
// Cleanup function for development/testing
export const clearAllClaudeData = mutation({
  args: {},
  handler: async (ctx) => {
    console.log('🧹 [CLEANUP] Starting to clear all Claude data...');
    
    let deletedCount = 0;
    
    // Delete all Claude messages
    const messages = await ctx.db.query("claudeMessages").collect();
    console.log(\`📧 [CLEANUP] Found \${messages.length} messages to delete\`);
    for (const message of messages) {
      await ctx.db.delete(message._id);
      deletedCount++;
    }
    
    // Delete all sync status entries
    const syncStatuses = await ctx.db.query("syncStatus").collect();
    console.log(\`🔄 [CLEANUP] Found \${syncStatuses.length} sync status entries to delete\`);
    for (const status of syncStatuses) {
      await ctx.db.delete(status._id);
      deletedCount++;
    }
    
    // Delete all Claude sessions
    const sessions = await ctx.db.query("claudeSessions").collect();
    console.log(\`💬 [CLEANUP] Found \${sessions.length} sessions to delete\`);
    for (const session of sessions) {
      await ctx.db.delete(session._id);
      deletedCount++;
    }
    
    console.log(\`✅ [CLEANUP] Successfully deleted \${deletedCount} total records\`);
    return { 
      success: true, 
      deletedRecords: deletedCount,
      breakdown: {
        messages: messages.length,
        syncStatuses: syncStatuses.length,
        sessions: sessions.length
      }
    };
  },
});`;

async function addCleanupMutation() {
  const fs = require('fs').promises;
  const claudeFilePath = path.join(__dirname, '../packages/convex/convex/claude.ts');
  
  try {
    // Read the current file
    const content = await fs.readFile(claudeFilePath, 'utf8');
    
    // Check if cleanup mutation already exists
    if (content.includes('clearAllClaudeData')) {
      console.log('🔍 Cleanup mutation already exists, skipping addition');
      return true;
    }
    
    // Add the cleanup mutation at the end
    const updatedContent = content + '\n' + cleanupMutation;
    await fs.writeFile(claudeFilePath, updatedContent);
    
    console.log('✅ Added cleanup mutation to claude.ts');
    return true;
  } catch (error) {
    console.error('❌ Failed to add cleanup mutation:', error.message);
    return false;
  }
}

async function deployConvexChanges() {
  return new Promise((resolve, reject) => {
    console.log('📦 Deploying Convex changes...');
    
    const deployProcess = spawn('bun', ['x', 'convex', 'dev', '--once'], {
      cwd: path.join(__dirname, '../packages/convex'),
      stdio: 'pipe'
    });
    
    let output = '';
    let errorOutput = '';
    
    deployProcess.stdout.on('data', (data) => {
      const text = data.toString();
      output += text;
      // Only log important lines to reduce noise
      if (text.includes('✓') || text.includes('×') || text.includes('Error') || text.includes('success')) {
        console.log(text.trim());
      }
    });
    
    deployProcess.stderr.on('data', (data) => {
      const text = data.toString();
      errorOutput += text;
      console.error(text.trim());
    });
    
    deployProcess.on('close', (code) => {
      if (code === 0) {
        console.log('✅ Convex deployment completed');
        resolve({ success: true, output });
      } else {
        console.error(`❌ Convex deployment failed with exit code ${code}`);
        reject(new Error(`Deploy failed: ${errorOutput || output}`));
      }
    });
    
    deployProcess.on('error', (error) => {
      console.error('❌ Failed to start deploy process:', error.message);
      reject(error);
    });
  });
}

async function runConvexFunction() {
  return new Promise((resolve, reject) => {
    console.log('🚀 Running Convex cleanup function...');
    
    const convexProcess = spawn('bun', ['x', 'convex', 'run', 'claude:clearAllClaudeData'], {
      cwd: path.join(__dirname, '../packages/convex'),
      stdio: 'pipe'
    });
    
    let output = '';
    let errorOutput = '';
    
    convexProcess.stdout.on('data', (data) => {
      const text = data.toString();
      output += text;
      console.log(text.trim());
    });
    
    convexProcess.stderr.on('data', (data) => {
      const text = data.toString();
      errorOutput += text;
      console.error(text.trim());
    });
    
    convexProcess.on('close', (code) => {
      if (code === 0) {
        console.log('✅ Cleanup completed successfully');
        resolve({ success: true, output });
      } else {
        console.error(`❌ Cleanup failed with exit code ${code}`);
        reject(new Error(`Process failed: ${errorOutput || output}`));
      }
    });
    
    convexProcess.on('error', (error) => {
      console.error('❌ Failed to start cleanup process:', error.message);
      reject(error);
    });
  });
}

async function removeCleanupMutation() {
  const fs = require('fs').promises;
  const claudeFilePath = path.join(__dirname, '../packages/convex/convex/claude.ts');
  
  try {
    // Read the current file
    const content = await fs.readFile(claudeFilePath, 'utf8');
    
    // Remove the cleanup mutation
    const lines = content.split('\n');
    const cleanupStartIndex = lines.findIndex(line => line.includes('clearAllClaudeData'));
    
    if (cleanupStartIndex === -1) {
      console.log('🔍 No cleanup mutation found to remove');
      return true;
    }
    
    // Find the end of the mutation (look for the closing });)
    let cleanupEndIndex = cleanupStartIndex;
    let braceCount = 0;
    let foundStart = false;
    
    for (let i = cleanupStartIndex; i < lines.length; i++) {
      const line = lines[i];
      
      if (line.includes('{')) {
        foundStart = true;
        braceCount += (line.match(/{/g) || []).length;
      }
      
      if (foundStart) {
        braceCount -= (line.match(/}/g) || []).length;
        
        if (braceCount === 0 && line.includes('});')) {
          cleanupEndIndex = i;
          break;
        }
      }
    }
    
    // Remove the mutation and any preceding comment lines
    let startRemoveIndex = cleanupStartIndex;
    while (startRemoveIndex > 0 && (lines[startRemoveIndex - 1].trim().startsWith('//') || lines[startRemoveIndex - 1].trim() === '')) {
      startRemoveIndex--;  
    }
    
    const updatedLines = [
      ...lines.slice(0, startRemoveIndex),
      ...lines.slice(cleanupEndIndex + 1)
    ];
    
    await fs.writeFile(claudeFilePath, updatedLines.join('\n'));
    console.log('✅ Removed cleanup mutation from claude.ts');
    return true;
  } catch (error) {
    console.error('❌ Failed to remove cleanup mutation:', error.message);
    return false;
  }
}

async function main() {
  console.log('🧹 Claude Conversations Cleanup Script');
  console.log('=====================================');
  
  try {
    // Step 1: Add cleanup mutation
    const mutationAdded = await addCleanupMutation();
    if (!mutationAdded) {
      process.exit(1);
    }
    
    // Step 2: Deploy the changes to Convex
    await deployConvexChanges();
    
    // Step 3: Run the cleanup
    await runConvexFunction();
    
    // Step 4: Remove the temporary mutation
    await removeCleanupMutation();
    
    // Step 5: Deploy again to remove the mutation from Convex
    console.log('📦 Removing cleanup mutation from deployment...');
    await deployConvexChanges();
    
    console.log('');
    console.log('🎉 All Claude conversations have been cleared!');
    console.log('✅ Temporary cleanup mutation has been removed');
    
  } catch (error) {
    console.error('');
    console.error('💥 Cleanup failed:', error.message);
    
    // Try to clean up the mutation even if the process failed
    console.log('🧹 Attempting to clean up temporary mutation...');
    await removeCleanupMutation();
    
    process.exit(1);
  }
}

// Run the script
if (require.main === module) {
  main();
}

module.exports = { main };