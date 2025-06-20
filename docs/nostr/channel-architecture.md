# Channel Architecture: Why a Separate Channels Table?

## Overview

In NIP-28, channels are "just events" - specifically:
- **Kind 40**: Channel creation (contains initial metadata)
- **Kind 41**: Metadata updates (references channel via `e` tag)
- **Kind 42**: Messages (references channel via `e` tag)

So why does our relay implementation have a separate `channels` table? This document explains the deep rationale behind this architectural decision.

## The Performance Problem

Without a channels table, showing a channel list would require:

```sql
-- Find all channels (kind 40 events)
SELECT * FROM events WHERE kind = 40;

-- For EACH channel, find latest metadata (kind 41)
SELECT * FROM events 
WHERE kind = 41 
  AND JSON_CONTAINS(tags, '["e", "channel_id"]')
ORDER BY created_at DESC 
LIMIT 1;

-- For EACH channel, count messages
SELECT COUNT(*) FROM events 
WHERE kind = 42 
  AND JSON_CONTAINS(tags, '["e", "channel_id"]');

-- For EACH channel, find last message time
SELECT MAX(created_at) FROM events 
WHERE kind = 42 
  AND JSON_CONTAINS(tags, '["e", "channel_id"]');
```

This would be **O(n*m)** complexity where n = channels and m = messages per channel. For a relay with 100 channels and 10,000 messages per channel, this means 1,000,000+ row scans just to show a channel list!

## The Solution: Denormalized Read Cache

The channels table acts as a materialized view that pre-computes:
- Current metadata (merged from kind 40 + latest kind 41)
- Message count (incremented on each kind 42)
- Last activity timestamp
- Creator info

This turns complex queries into a simple: 
```sql
SELECT * FROM channels ORDER BY last_message_at DESC
```

## Key Benefits

### 1. Query Performance

A single indexed query replaces multiple complex JSON searches. Performance improvement: **100-1000x faster** for channel lists.

### 2. Index Efficiency

The channels table has specific indexes:
```sql
nameIdx: index("idx_name").on(table.name),
creatorIdx: index("idx_creator").on(table.creator_pubkey),
lastMessageIdx: index("idx_last_message").on(table.last_message_at),
messageCountIdx: index("idx_message_count").on(table.message_count)
```

These enable:
- Fast channel search by name
- Find channels by creator
- Sort by activity or popularity
- Filter active vs inactive channels

Doing this with just events would require complex JSON tag queries which can't be efficiently indexed in MySQL.

### 3. Write-Once, Read-Many Pattern

Channels follow a pattern where:
- Channel creation (kind 40) happens once
- Metadata updates (kind 41) are rare
- Messages (kind 42) are frequent but only increment counters
- Channel lists are viewed constantly

The separate table optimizes for the common read case at the cost of slightly more complex writes.

### 4. Atomic Updates

When a new message arrives, we can atomically:
```sql
UPDATE channels 
SET message_count = message_count + 1,
    last_message_at = NOW()
WHERE id = ?
```

Without this table, we'd need to recount all messages or maintain counts in application memory.

### 5. Relay-Specific Optimization

While Nostr events are the source of truth, relays need to serve data efficiently. The channels table is essentially a relay-specific index that doesn't change the protocol - it just makes the relay performant.

## Implementation Details

When processing events:

```typescript
if (event.kind === 40) {
  // Channel creation - insert into channels table
  await tx.insert(schema.channels).values({
    id: event.id,
    name: metadata.name,
    about: metadata.about,
    picture: metadata.picture,
    creator_pubkey: event.pubkey
  })
}

if (event.kind === 41) {
  // Metadata update - update channels table
  const channelId = event.tags.find(t => t[0] === 'e')?.[1]
  await tx.update(schema.channels)
    .set({ name: metadata.name, about: metadata.about })
    .where(eq(schema.channels.id, channelId))
}

if (event.kind === 42) {
  // New message - increment counter
  const channelId = event.tags.find(t => t[0] === 'e')?.[1]
  await tx.update(schema.channels)
    .set({
      message_count: sql`${schema.channels.message_count} + 1`,
      last_message_at: new Date()
    })
    .where(eq(schema.channels.id, channelId))
}
```

## Trade-offs

### Pros:
- 100-1000x faster channel list queries
- Efficient sorting and filtering
- Lower database load
- Better user experience
- Enables features like "trending channels"

### Cons:
- Data duplication (metadata stored twice)
- Complexity in keeping data synchronized
- More code to maintain
- Potential for inconsistencies if sync fails
- Additional storage requirements

## Alternatives Considered

### 1. Pure Event-Based Queries
- **Pros**: No denormalization, simpler code
- **Cons**: Prohibitively slow for any non-trivial number of channels

### 2. In-Memory Cache
- **Pros**: Even faster than database
- **Cons**: Lost on restart, doesn't scale across multiple relay instances

### 3. Redis/External Cache
- **Pros**: Fast, shared across instances
- **Cons**: Additional infrastructure, complexity

### 4. Database Views
- **Pros**: Automatic synchronization
- **Cons**: Still slow for complex aggregations, limited by MySQL capabilities

## Conclusion

The separate channels table is a **performance optimization**, not a protocol requirement. It's similar to how:
- Search engines maintain inverted indexes
- Databases use materialized views
- Caches store computed results

For a production relay serving many users, this denormalization is essential. It allows us to provide a responsive UI while maintaining compatibility with the Nostr protocol.

The events remain the source of truth - the channels table is simply a performance index that can be rebuilt from events at any time. This gives us the best of both worlds: protocol compliance and production-ready performance.