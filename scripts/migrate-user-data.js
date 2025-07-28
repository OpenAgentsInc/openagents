#!/usr/bin/env node

/**
 * Migration Script for Claude Sessions
 * 
 * This script helps you migrate your existing Claude sessions and messages
 * to be properly associated with your user account.
 * 
 * Usage:
 *   node scripts/migrate-user-data.js status
 *   node scripts/migrate-user-data.js preview  
 *   node scripts/migrate-user-data.js migrate
 *   node scripts/migrate-user-data.js sessions
 */

const { ConvexHttpClient } = require("convex/browser");
const { api } = require("../packages/convex/convex/_generated/api");

// Configuration - read from packages/convex/.env.local
const fs = require('fs');
const path = require('path');

let CONVEX_URL = process.env.CONVEX_URL || process.env.VITE_CONVEX_URL;

// Try to read from packages/convex/.env.local if not in environment
if (!CONVEX_URL) {
  try {
    const envPath = path.join(__dirname, '../packages/convex/.env.local');
    const envContent = fs.readFileSync(envPath, 'utf8');
    const match = envContent.match(/CONVEX_URL=(.+)/);
    if (match) {
      CONVEX_URL = match[1].trim();
    }
  } catch (error) {
    // Ignore if file doesn't exist
  }
}
const GITHUB_ID = "14167547"; // Your GitHub ID

if (!CONVEX_URL) {
  console.error("❌ Error: CONVEX_URL environment variable not set");
  console.log("Please set CONVEX_URL in your .env file or environment");
  process.exit(1);
}

const client = new ConvexHttpClient(CONVEX_URL);

async function checkStatus() {
  console.log("🔍 Checking migration status...\n");
  
  try {
    const status = await client.query(api.migration.getMigrationStatus, {
      githubId: GITHUB_ID
    });
    
    if (!status.userFound) {
      console.log("❌ User not found!");
      console.log(`   GitHub ID: ${GITHUB_ID}`);
      console.log("   Make sure you've logged in to the app at least once.");
      return;
    }
    
    console.log("✅ User found!");
    console.log(`   Username: ${status.user.githubUsername}`);
    console.log(`   Email: ${status.user.email}`);
    console.log("");
    
    console.log("📊 Data Summary:");
    console.log(`   Sessions owned by you: ${status.sessions.owned}`);
    console.log(`   Orphaned sessions: ${status.sessions.orphaned}`);
    console.log(`   Total sessions: ${status.sessions.total}`);
    console.log("");
    console.log(`   Messages owned by you: ${status.messages.owned}`);
    console.log(`   Orphaned messages: ${status.messages.orphaned}`);
    console.log(`   Total messages: ${status.messages.total}`);
    console.log("");
    
    if (status.migrationNeeded) {
      console.log("🔄 Migration needed!");
      console.log("   Run 'node scripts/migrate-user-data.js preview' to see what would be migrated");
      console.log("   Run 'node scripts/migrate-user-data.js migrate' to perform the migration");
    } else {
      console.log("✅ No migration needed - all data is already associated with your account!");
    }
    
  } catch (error) {
    console.error("❌ Error checking status:", error.message);
  }
}

async function previewMigration() {
  console.log("🔍 Previewing migration (dry run)...\n");
  
  try {
    const result = await client.mutation(api.migration.migrateExistingDataToUser, {
      githubId: GITHUB_ID,
      dryRun: true
    });
    
    console.log("📋 Migration Preview:");
    console.log(`   Sessions to migrate: ${result.sessionsToMigrate}`);
    console.log(`   Messages to migrate: ${result.messagesToMigrate}`);
    console.log(`   Target user: ${result.userName}`);
    console.log("");
    
    if (result.sessionsToMigrate === 0 && result.messagesToMigrate === 0) {
      console.log("✅ Nothing to migrate - all data is already yours!");
    } else {
      console.log("🚀 Ready to migrate!");
      console.log("   Run 'node scripts/migrate-user-data.js migrate' to perform the actual migration");
    }
    
  } catch (error) {
    console.error("❌ Error previewing migration:", error.message);
  }
}

async function performMigration() {
  console.log("🚀 Starting migration...\n");
  
  try {
    const result = await client.mutation(api.migration.migrateExistingDataToUser, {
      githubId: GITHUB_ID,
      dryRun: false
    });
    
    console.log("🎉 Migration completed!");
    console.log(`   Sessions migrated: ${result.migratedSessions}`);
    console.log(`   Messages migrated: ${result.migratedMessages}`);
    console.log(`   All data now owned by: ${result.userName}`);
    console.log("");
    console.log("✅ Your Claude sessions should now appear in the app!");
    
  } catch (error) {
    console.error("❌ Error during migration:", error.message);
  }
}

async function listSessions() {
  console.log("📋 Your recent Claude sessions:\n");
  
  try {
    const sessions = await client.query(api.migration.getRecentSessions, {
      githubId: GITHUB_ID,
      limit: 15
    });
    
    if (sessions.length === 0) {
      console.log("   No sessions found for your account");
      console.log("   Try running migration first if you have older sessions");
      return;
    }
    
    sessions.forEach((session, index) => {
      console.log(`${index + 1}. ${session.title || 'Untitled Session'}`);
      console.log(`   ID: ${session.sessionId}`);
      console.log(`   Path: ${session.projectPath}`);
      console.log(`   Status: ${session.status}`);
      console.log(`   Created by: ${session.createdBy}`);
      console.log(`   Last activity: ${session.lastActivity}`);
      console.log("");
    });
    
  } catch (error) {
    console.error("❌ Error listing sessions:", error.message);
  }
}

// Main execution
async function main() {
  const command = process.argv[2];
  
  console.log("🤖 Claude Sessions Migration Tool");
  console.log(`   GitHub ID: ${GITHUB_ID}`);
  console.log(`   Convex URL: ${CONVEX_URL}`);
  console.log("");
  
  switch (command) {
    case 'status':
      await checkStatus();
      break;
      
    case 'preview':
      await previewMigration();
      break;
      
    case 'migrate':
      await performMigration();
      break;
      
    case 'sessions':
      await listSessions();
      break;
      
    default:
      console.log("Usage:");
      console.log("  node scripts/migrate-user-data.js status    - Check current migration status");
      console.log("  node scripts/migrate-user-data.js preview   - Preview what would be migrated");
      console.log("  node scripts/migrate-user-data.js migrate   - Perform the actual migration");
      console.log("  node scripts/migrate-user-data.js sessions  - List your recent sessions");
      break;
  }
}

main().catch(console.error);