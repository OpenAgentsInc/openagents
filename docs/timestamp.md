# Custom Timestamp Implementation

## Overview

This document explains why we implemented a custom `Timestamp` type in our Rust codebase instead of using existing timestamp types directly.

## Background

In our codebase, we deal with timestamps in several contexts:
1. Database storage and retrieval (PostgreSQL)
2. JSON serialization/deserialization (for APIs)
3. Business logic operations
4. Time-based comparisons and calculations

The main challenges we faced were:
- Inconsistency between different timestamp types (`chrono::DateTime`, `time::OffsetDateTime`, PostgreSQL timestamps)
- Serialization/deserialization complexity
- Type conversion overhead
- Potential precision loss during conversions
- Database compatibility issues

## Solution

We implemented a custom `Timestamp` type that wraps `time::OffsetDateTime` and provides:
1. Consistent behavior across the application
2. Safe conversions between different timestamp formats
3. Proper database integration
4. Clean serialization/deserialization

### Implementation Details

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timestamp(OffsetDateTime);
```

Key features:
1. **Internal Storage**: Uses `OffsetDateTime` from the `time` crate
2. **Database Integration**: Implements `Type<Postgres>`, `Encode`, and `Decode` traits
3. **Serialization**: Implements `Serialize` and `Deserialize`
4. **Conversions**: Implements `From` traits for common timestamp types

## Benefits

1. **Type Safety**
   - Prevents accidental mixing of different timestamp types
   - Ensures consistent timezone handling (always UTC)
   - Makes timestamp-related bugs more obvious at compile time

2. **Database Compatibility**
   - Direct mapping to PostgreSQL's timestamp type
   - No precision loss during database operations
   - Proper handling of NULL values

3. **API Consistency**
   - Uniform timestamp representation across the API
   - Predictable serialization format
   - Clear handling of optional timestamps

4. **Performance**
   - Minimizes conversions between different timestamp types
   - Reduces runtime overhead
   - Efficient database operations

## Common Use Cases

### 1. Creating Timestamps

```rust
// Current time
let now = Timestamp::now();

// From chrono::DateTime
let dt = chrono::Utc::now();
let ts: Timestamp = dt.into();
```

### 2. Database Operations

```rust
// In database queries
sqlx::query_as!(
    Message,
    "SELECT created_at FROM messages WHERE id = $1",
    id
)
```

### 3. JSON Serialization

```rust
#[derive(Serialize, Deserialize)]
struct Message {
    created_at: Option<Timestamp>,
    updated_at: Option<Timestamp>
}
```

## Implementation Challenges

1. **Orphan Rules**
   - Cannot implement `From` for `Option<DateTime>` directly
   - Solved by providing conversion methods instead

2. **PostgreSQL Integration**
   - Need to implement custom encode/decode logic
   - Careful handling of timezone information

3. **Precision Handling**
   - Ensuring no loss of precision during conversions
   - Maintaining nanosecond accuracy

## Future Improvements

1. **Additional Conversions**
   - Support for more timestamp formats
   - Better error handling for invalid timestamps

2. **Performance Optimizations**
   - Cached conversions for common operations
   - Reduced allocation overhead

3. **Enhanced Functionality**
   - Additional time manipulation methods
   - Better timezone support if needed

## Best Practices

1. Always use `Timestamp::now()` for current time
2. Use `Option<Timestamp>` for nullable timestamps
3. Prefer direct timestamp operations over conversions
4. Keep timestamps in UTC throughout the application

## Migration Guide

When working with timestamps in the codebase:

1. Replace direct usage of `chrono::DateTime` or `time::OffsetDateTime` with `Timestamp`
2. Update database queries to use the new type
3. Update API handlers to use `Timestamp` for consistency
4. Use the provided conversion methods when interfacing with external systems

## Conclusion

Our custom `Timestamp` type provides a robust solution for handling timestamps throughout the application. It ensures consistency, type safety, and proper integration with our database and API layers while minimizing the complexity of working with different timestamp formats.
