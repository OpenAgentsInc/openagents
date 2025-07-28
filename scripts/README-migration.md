# Claude Sessions Migration Scripts

These scripts help you migrate your existing Claude sessions and messages to be properly associated with your user account after the authentication fixes.

## 🎯 What This Does

After the recent security fixes, all Claude sessions now require user authentication and proper ownership. Any existing sessions/messages from before this fix are "orphaned" (not linked to any user account). These scripts help you claim those orphaned sessions so they appear in your app.

## 🚀 Quick Start

### Option 1: Interactive Shell Script (Recommended)
```bash
# Run the interactive migration script
./scripts/migrate-my-chats.sh
```

This script will:
1. Check your current status
2. Preview what would be migrated
3. Ask for confirmation
4. Perform the migration
5. Show your sessions after migration

### Option 2: Individual Commands

Check your current status:
```bash
./scripts/check-migration.sh
```

List your sessions:
```bash
./scripts/list-my-sessions.sh
```

### Option 3: Advanced Node.js Script

For more control, use the Node.js script directly:

```bash
# Check migration status
node scripts/migrate-user-data.js status

# Preview migration (safe - no changes made)
node scripts/migrate-user-data.js preview

# Perform actual migration
node scripts/migrate-user-data.js migrate

# List your sessions
node scripts/migrate-user-data.js sessions
```

### Option 4: Web Dashboard

Open the HTML dashboard in your browser:
```bash
open scripts/migration-dashboard.html
```

(Note: The web interface currently shows instructions to use the command line - browser-based migration not yet implemented)

## 📋 What You'll See

### Migration Status Output
```
✅ User found!
   Username: your-github-username
   Email: your-email@example.com

📊 Data Summary:
   Sessions owned by you: 5
   Orphaned sessions: 12
   Total sessions: 17

   Messages owned by you: 150
   Orphaned messages: 340
   Total messages: 490

🔄 Migration needed!
```

### After Migration
```
🎉 Migration completed!
   Sessions migrated: 12
   Messages migrated: 340
   All data now owned by: your-github-username

✅ Your Claude sessions should now appear in the app!
```

## 🔧 Configuration

The scripts are configured with your GitHub ID: **14167547**

If you need to change this, edit the `GITHUB_ID` constant in:
- `scripts/migrate-user-data.js`
- `scripts/migration-dashboard.html`

## 🛡️ Safety Features

- **Dry Run Mode**: Preview exactly what would be migrated before making changes
- **User Validation**: Ensures you're authenticated and the user exists
- **Orphan-Only**: Only migrates sessions/messages that aren't already owned by someone
- **Confirmation**: Interactive script asks for confirmation before migrating
- **Rollback Info**: All changes are logged for debugging if needed

## 📚 Technical Details

### What Gets Migrated
- **Sessions**: Claude conversation sessions without a `userId`
- **Messages**: Individual messages within sessions without a `userId`
- **Sync Status**: Related sync tracking data

### What Doesn't Get Migrated
- Sessions already owned by any user (including you)
- Messages already owned by any user
- System data or metadata

### Database Changes
The migration updates two fields:
- `claudeSessions.userId` - Links sessions to your user account
- `claudeMessages.userId` - Links messages to your user account

## 🆘 Troubleshooting

### "User not found" Error
Make sure you've logged into the app at least once with GitHub authentication. The migration script needs your user record to exist in the database.

### "CONVEX_URL environment variable not set"
The scripts need to know how to connect to your Convex database. Set the `CONVEX_URL` or `VITE_CONVEX_URL` environment variable, or check your `.env` files.

### No Sessions Found
If you see "No sessions found" after migration, it means:
1. All your sessions were already properly linked, or
2. There were no orphaned sessions to migrate

### Permission Errors
If you get authentication errors, make sure:
1. You're logged into the app
2. Your authentication token is valid
3. The Convex deployment is accessible

## 🔍 Verification

After migration, you can verify everything worked by:

1. **Check the app**: Your old sessions should now appear
2. **Run status check**: Should show 0 orphaned sessions/messages
3. **List sessions**: Should show all your migrated sessions

```bash
# Verify migration worked
./scripts/check-migration.sh
./scripts/list-my-sessions.sh
```

## 📞 Need Help?

If you run into issues:
1. Check the troubleshooting section above
2. Look at the console output for specific error messages
3. Try running the status check first to verify your setup
4. Use the preview mode to see what would be migrated before making changes

The migration is designed to be safe and reversible, so don't hesitate to try the preview mode to understand what it would do.