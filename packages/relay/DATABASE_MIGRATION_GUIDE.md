# Database Migration Guide for OpenAgents

## Overview

This guide explains how to handle database migrations in the OpenAgents monorepo. We use Drizzle ORM with PlanetScale MySQL for production.

## Key Principles

1. **Schema First**: Always define the desired schema in `packages/relay/src/schema.ts`
2. **Migration Scripts**: Use custom TypeScript migration scripts for complex changes
3. **Safe Migrations**: Always check existing tables before making changes
4. **Validation**: Test migrations locally before production

## Directory Structure

```
packages/relay/
├── src/schema.ts              # Single source of truth for database schema
├── drizzle.config.ts          # Drizzle configuration
├── drizzle/                   # Generated migration files
│   └── *.sql
└── scripts/
    └── run-migration.ts       # Custom migration script
```

## Environment Setup

Database credentials are stored in root `.env` file:

```bash
DATABASE_HOST=aws.connect.psdb.cloud
DATABASE_USERNAME=your_username
DATABASE_PASSWORD=your_password
DATABASE_NAME=openagents-com
```

## Migration Workflow

### 1. Update Schema Definition

Edit `packages/relay/src/schema.ts` to define the desired database structure:

```typescript
export const channels = mysqlTable("channels", {
  id: varchar("id", { length: 64 }).primaryKey(),
  name: varchar("name", { length: 255 }),
  creator_pubkey: varchar("creator_pubkey", { length: 64 }).notNull(),
  message_count: bigint("message_count", { mode: "number" }).default(0),
  created_at: timestamp("created_at").defaultNow().notNull(),
  updated_at: timestamp("updated_at").defaultNow().onUpdateNow().notNull()
}, (table) => ({
  creatorIdx: index("idx_creator").on(table.creator_pubkey),
}))
```

### 2. Generate Migration (Optional)

For reference, you can generate a full migration:

```bash
cd packages/relay
pnpm db:generate
```

This creates a `.sql` file in `drizzle/` directory.

### 3. Create Custom Migration Script

For production safety, create a custom migration script in `packages/relay/scripts/`:

```typescript
import { Client } from "@planetscale/database"
import 'dotenv/config'

const client = new Client({
  host: process.env.DATABASE_HOST!,
  username: process.env.DATABASE_USERNAME!,
  password: process.env.DATABASE_PASSWORD!,
  database: process.env.DATABASE_NAME!,
})

async function runMigration() {
  console.log('🚀 Starting database migration...')
  
  // Check if table exists
  const tablesResult = await client.execute('SHOW TABLES LIKE "table_name"')
  
  if (tablesResult.rows.length === 0) {
    // Create table from scratch
    await client.execute(`CREATE TABLE...`)
  } else {
    // Add missing columns
    const columnsResult = await client.execute('DESCRIBE table_name')
    const existingColumns = columnsResult.rows.map(row => row.Field)
    
    const requiredColumns = [
      { name: 'new_column', sql: 'ADD COLUMN new_column VARCHAR(64) NOT NULL DEFAULT ""' }
    ]
    
    for (const column of requiredColumns) {
      if (!existingColumns.includes(column.name)) {
        await client.execute(`ALTER TABLE table_name ${column.sql}`)
      }
    }
  }
}
```

### 4. Run Migration

Execute the migration script:

```bash
cd packages/relay
bun scripts/run-migration.ts
```

### 5. Rebuild and Test

```bash
pnpm --filter=@openagentsinc/relay build
```

Test that queries work:

```bash
curl http://localhost:3003/api/channels/list
```

## Common Migration Patterns

### Adding a New Column

```typescript
// In schema.ts
export const table = mysqlTable("table", {
  existing_column: varchar("existing", { length: 64 }),
  new_column: varchar("new_column", { length: 64 }).notNull(),
})

// In migration script
const requiredColumns = [
  { name: 'new_column', sql: 'ADD COLUMN new_column VARCHAR(64) NOT NULL DEFAULT ""' }
]
```

### Adding an Index

```typescript
// In schema.ts
}, (table) => ({
  newIdx: index("idx_new").on(table.new_column),
}))

// In migration script
await client.execute('CREATE INDEX IF NOT EXISTS idx_new ON table(new_column)')
```

### Modifying Column Type

```typescript
// Use ALTER TABLE to modify existing columns
await client.execute('ALTER TABLE table MODIFY COLUMN existing_column BIGINT DEFAULT 0')
```

## Troubleshooting

### Common Errors

1. **"Unknown column X in field list"**
   - Column missing from database but expected by schema
   - Solution: Add column via migration script

2. **"Table doesn't exist"**
   - Schema defines table that doesn't exist in database
   - Solution: Create table or modify schema

3. **"Index already exists"**
   - Index creation failing
   - Solution: Use `CREATE INDEX IF NOT EXISTS`

### Debugging Steps

1. Check current database structure:
   ```sql
   DESCRIBE table_name;
   SHOW INDEX FROM table_name;
   ```

2. Compare with schema definition in `schema.ts`

3. Create migration script to bridge the gap

4. Test locally before production

## Best Practices

1. **Always backup**: PlanetScale has automatic backups, but be careful
2. **Incremental changes**: Make small, reversible changes
3. **Test first**: Run migrations on development branch
4. **Document changes**: Update this guide for complex patterns
5. **Validate**: Always test API endpoints after migrations

## Integration with Code

After successful migration:

1. Update `packages/relay/src/database.ts` if needed
2. Rebuild relay package: `pnpm --filter=@openagentsinc/relay build`
3. Test all affected API endpoints
4. Commit schema and migration script changes

## Emergency Rollback

If migration fails:

1. Check PlanetScale dashboard for backup restore options
2. Revert schema changes in code
3. Deploy previous version
4. Investigate and fix migration script

---

## Example: Adding NIP-28 Channels Support

This shows the complete process used to add channels table:

1. **Schema Definition** (`packages/relay/src/schema.ts`):
   ```typescript
   export const channels = mysqlTable("channels", {
     id: varchar("id", { length: 64 }).primaryKey(),
     name: varchar("name", { length: 255 }),
     about: text("about"),
     picture: varchar("picture", { length: 500 }),
     creator_pubkey: varchar("creator_pubkey", { length: 64 }).notNull(),
     message_count: bigint("message_count", { mode: "number" }).default(0),
     last_message_at: timestamp("last_message_at"),
     created_at: timestamp("created_at").defaultNow().notNull(),
     updated_at: timestamp("updated_at").defaultNow().onUpdateNow().notNull()
   }, (table) => ({
     nameIdx: index("idx_name").on(table.name),
     creatorIdx: index("idx_creator").on(table.creator_pubkey),
     lastMessageIdx: index("idx_last_message").on(table.last_message_at),
     messageCountIdx: index("idx_message_count").on(table.message_count)
   }))
   ```

2. **Migration Script** (`packages/relay/scripts/run-migration.ts`):
   - Checked if table exists
   - Added missing columns incrementally
   - Created indexes safely
   - Validated final structure

3. **Testing**:
   ```bash
   curl http://localhost:3003/api/channels/list
   # Returns: {"channels":[]}
   ```

This pattern should be followed for all future schema changes.