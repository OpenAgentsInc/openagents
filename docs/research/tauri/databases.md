I'll research local database options that work well with Tauri applications, considering performance, integration patterns, and desktop-specific requirements.Based on my research, here's a comprehensive overview of local database options that work well with Tauri:

## SQLite - The Clear Winner for Most Use Cases

**SQLite is the most popular and recommended choice** for Tauri applications. Here's why:

### Integration Options:
1. **Official Tauri SQL Plugin** - The easiest approach using `@tauri-apps/plugin-sql`
2. **Direct Rust Integration** - Using crates like `sqlx`, `diesel`, or `rusqlite`

### Performance Characteristics:
- **Rusqlite**: Fastest performance (7-70x faster than sqlx for some operations)
- **SQLx**: Good async support but slower due to async overhead
- **Diesel**: Fast performance with strong type safety, but limited async support for SQLite

### Bundle Size Impact:
- **SQLite adds only ~1.9MB** to your app bundle
- Minimal footprint compared to alternatives

### Code Example (SQLx approach):
```rust
// Cargo.toml
[dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }

// Database setup
async fn setup_database(app_handle: &AppHandle) -> Result<SqlitePool, sqlx::Error> {
    let app_data_dir = app_handle.path().app_data_dir()
        .expect("Failed to get app data directory");

    let db_path = app_data_dir.join("database.sqlite");

    let connection_options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connection_options)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
```

## SurrealDB - The Feature-Rich Alternative

**SurrealDB offers advanced capabilities** but comes with trade-offs:

### Advantages:
- Multi-model database (document, graph, key-value)
- Built-in full-text search and vector embeddings
- Advanced graph queries and relationships
- Real-time subscriptions
- Strong TypeScript support

### Disadvantages:
- **Significantly larger bundle size**: Adds ~16.1MB (vs SQLite's 1.9MB)
- More complex setup
- Overkill for simple applications

### When to Choose SurrealDB:
- You need graph database capabilities
- Complex relationships between data
- Real-time features are critical
- Full-text search is required

### Code Example:
```rust
// Cargo.toml
[dependencies]
surrealdb = { version = "1.0", features = ["kv-rocksdb"] }

// Setup
use surrealdb::{engine::local::RocksDb, Surreal};

async fn setup_surrealdb(app_handle: &AppHandle) -> Result<Surreal<RocksDb>, surrealdb::Error> {
    let app_data_dir = app_handle.path().app_data_dir()
        .expect("Failed to get app data directory");

    let db_path = app_data_dir.join("database");

    let db = Surreal::new::<RocksDb>(db_path).await?;
    db.use_ns("app").use_db("main").await?;

    Ok(db)
}
```

## Other Options to Consider

### BonsaiDB
- Document-based database in alpha
- Good async support
- Smaller community and ecosystem

### Traditional SQL Databases
- **PostgreSQL/MySQL**: Not practical for embedded use
- Require separate server processes
- Not suitable for desktop applications

## Performance Comparison Summary

| Database | Bundle Size | Performance | Async Support | Complexity |
|----------|-------------|-------------|---------------|------------|
| SQLite (rusqlite) | +1.9MB | Excellent | Limited | Low |
| SQLite (sqlx) | +1.9MB | Good | Excellent | Low |
| SQLite (diesel) | +1.9MB | Excellent | Limited | Medium |
| SurrealDB | +16.1MB | Good | Excellent | High |

## Recommendation

**For most Tauri applications, start with SQLite using one of these approaches:**

1. **Beginners**: Use the official Tauri SQL plugin
2. **Performance-critical**: Use `rusqlite` directly
3. **Async-heavy**: Use `sqlx` with connection pooling
4. **Type safety focus**: Use `diesel` (accept limited async)

**Consider SurrealDB only if you specifically need:**
- Graph database features
- Complex relationships
- Real-time subscriptions
- Advanced search capabilities

The bundle size difference is significant - SQLite keeps your app lean while SurrealDB adds substantial weight. For desktop applications where users care about download size and startup time, SQLite is usually the better choice unless you absolutely need SurrealDB's advanced features.
